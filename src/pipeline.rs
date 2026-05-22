//! End-to-end render pipeline: slides → Gemini notes → Gemini TTS → MP4.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;
use tracing::{info, warn};

use crate::adapters::{FfmpegAdapter, GeminiAdapter, PdfAdapter};
use crate::audio::{pcm_duration_ms, pcm_to_wav};
use crate::ffmpeg::{EncodeOptions, SegmentInput};
use crate::models::{Transition, Voice};

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub cache_dir: PathBuf,
    pub voice: Voice,
    pub transition: Transition,
    pub notes_model: String,
    pub gemini_concurrency: usize,
    pub encode_concurrency: usize,
    pub encode: EncodeOptions,
    pub pdf_dpi: u32,
    pub pdf_jpeg_quality: u32,
    pub keep_cache: bool,
    pub regenerate_notes: bool,
    pub regenerate_audio: bool,
}

/// Injected adapters for external dependencies.
pub struct Adapters<G, F, P> {
    pub gemini: Arc<G>,
    pub ffmpeg: Arc<F>,
    pub pdf: P,
}

pub async fn render<G, F, P>(opts: RenderOptions, adapters: Adapters<G, F, P>) -> Result<PathBuf>
where
    G: GeminiAdapter + 'static,
    F: FfmpegAdapter + Send + Sync + 'static,
    P: PdfAdapter,
{
    tokio::fs::create_dir_all(&opts.cache_dir)
        .await
        .with_context(|| format!("creating cache dir {}", opts.cache_dir.display()))?;
    let images_dir = opts.cache_dir.join("images");
    let notes_dir = opts.cache_dir.join("notes");
    let audio_dir = opts.cache_dir.join("audio");
    let segments_dir = opts.cache_dir.join("segments");
    for d in [&images_dir, &notes_dir, &audio_dir, &segments_dir] {
        tokio::fs::create_dir_all(d).await?;
    }

    // 1. Resolve slide images (PDF or directory).
    let slide_images = resolve_slides(&opts.input, &images_dir, &opts, &adapters.pdf).await?;
    info!(count = slide_images.len(), "resolved slides");

    // 2. Notes per slide (cached on disk as .txt sidecars).
    let notes_pb = make_pb(slide_images.len() as u64, "notes");
    let notes = generate_notes(
        &adapters.gemini,
        &slide_images,
        &notes_dir,
        &opts.notes_model,
        opts.gemini_concurrency,
        opts.regenerate_notes,
        &notes_pb,
    )
    .await?;
    notes_pb.finish_with_message("notes ready");

    // 3. TTS audio per slide (cached as .wav).
    let audio_pb = make_pb(slide_images.len() as u64, "audio");
    let audio = generate_audio(
        &adapters.gemini,
        &notes,
        &audio_dir,
        opts.voice,
        opts.gemini_concurrency,
        opts.regenerate_audio,
        &audio_pb,
    )
    .await?;
    audio_pb.finish_with_message("audio ready");

    // 4. Encode per-slide MP4 segments in parallel.
    let ffmpeg_bin = adapters.ffmpeg.locate().await?;
    let segments: Vec<SegmentInput> = slide_images
        .iter()
        .enumerate()
        .map(|(i, img)| SegmentInput {
            index: i,
            image_path: img.clone(),
            audio_path: audio[i].path.clone(),
            output_path: segments_dir.join(format!("segment_{i:04}.mp4")),
            #[allow(clippy::cast_precision_loss)]
            duration_seconds: (audio[i].duration_ms as f64) / 1000.0,
            transition: opts.transition,
        })
        .collect();

    let encode_pb = make_pb(segments.len() as u64, "encode");
    encode_all(
        &adapters.ffmpeg,
        &ffmpeg_bin,
        &segments,
        opts.encode,
        opts.encode_concurrency,
        &encode_pb,
    )
    .await?;
    encode_pb.finish_with_message("segments encoded");

    // 5. Assemble into a final MP4.
    info!(output = %opts.output.display(), "assembling final video");
    if let Some(parent) = opts.output.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    adapters
        .ffmpeg
        .assemble(
            &ffmpeg_bin,
            &segments,
            &segments_dir,
            &opts.output,
            opts.encode,
        )
        .await?;

    if !opts.keep_cache {
        // Keep notes + audio (cheap, user value) but drop the heavy
        // segment intermediates.
        let _ = tokio::fs::remove_dir_all(&segments_dir).await;
    }

    Ok(opts.output)
}

async fn resolve_slides(
    input: &Path,
    images_dir: &Path,
    opts: &RenderOptions,
    pdf: &impl PdfAdapter,
) -> Result<Vec<PathBuf>> {
    let meta = tokio::fs::metadata(input)
        .await
        .with_context(|| format!("stat {}", input.display()))?;

    if meta.is_dir() {
        return pdf.discover_images(input);
    }

    if meta.is_file() {
        let ext = input
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if ext == "pdf" {
            // If we already rasterised this PDF and the cache is intact,
            // reuse it.
            if let Ok(cached) = pdf.discover_images(images_dir) {
                info!(count = cached.len(), "reusing cached PDF rasterisation");
                return Ok(cached);
            }
            return pdf
                .rasterise(input, images_dir, opts.pdf_dpi, opts.pdf_jpeg_quality)
                .await;
        }
        bail!(
            "unsupported input file: {}. Pass a PDF or a directory of slide images.",
            input.display()
        );
    }

    bail!("input not found: {}", input.display())
}

struct AudioOutput {
    path: PathBuf,
    duration_ms: u64,
}

async fn generate_notes<G: GeminiAdapter + 'static>(
    gemini: &Arc<G>,
    slide_images: &[PathBuf],
    notes_dir: &Path,
    notes_model: &str,
    concurrency: usize,
    regenerate: bool,
    pb: &ProgressBar,
) -> Result<Vec<String>> {
    let mut notes = vec![String::new(); slide_images.len()];
    let sem = std::sync::Arc::new(Semaphore::new(concurrency.max(1)));
    let mut handles = Vec::with_capacity(slide_images.len());

    for (i, image) in slide_images.iter().enumerate() {
        let sem = sem.clone();
        let gemini = Arc::clone(gemini);
        let notes_dir = notes_dir.to_path_buf();
        let notes_model = notes_model.to_string();
        let image = image.clone();
        let pb = pb.clone();
        handles.push(tokio::spawn(async move {
            // Semaphore is Arc-held and never closed; acquire cannot fail.
            let _permit = sem.acquire_owned().await.unwrap();
            let cache_path = notes_dir.join(format!("slide_{i:04}.txt"));
            if !regenerate && let Ok(existing) = tokio::fs::read_to_string(&cache_path).await {
                pb.inc(1);
                return Result::<(usize, String)>::Ok((i, existing));
            }
            let bytes = tokio::fs::read(&image)
                .await
                .with_context(|| format!("read {}", image.display()))?;
            let text = gemini.extract_notes(&bytes, i == 0, &notes_model).await?;
            tokio::fs::write(&cache_path, &text).await?;
            pb.inc(1);
            Ok((i, text))
        }));
    }

    for h in handles {
        let (i, text) = h.await.context("notes task panicked")??;
        notes[i] = text;
    }
    Ok(notes)
}

async fn generate_audio<G: GeminiAdapter + 'static>(
    gemini: &Arc<G>,
    notes: &[String],
    audio_dir: &Path,
    voice: Voice,
    concurrency: usize,
    regenerate: bool,
    pb: &ProgressBar,
) -> Result<Vec<AudioOutput>> {
    let mut outputs: Vec<Option<AudioOutput>> = (0..notes.len()).map(|_| None).collect();
    let sem = std::sync::Arc::new(Semaphore::new(concurrency.max(1)));
    let mut handles = Vec::with_capacity(notes.len());

    for (i, note) in notes.iter().enumerate() {
        let sem = sem.clone();
        let gemini = Arc::clone(gemini);
        let audio_dir = audio_dir.to_path_buf();
        let note = note.clone();
        let pb = pb.clone();
        handles.push(tokio::spawn(async move {
            // Semaphore is Arc-held and never closed; acquire cannot fail.
            let _permit = sem.acquire_owned().await.unwrap();
            let wav_path = audio_dir.join(format!("slide_{i:04}.wav"));
            let pcm_path = audio_dir.join(format!("slide_{i:04}.pcm"));

            if !regenerate
                && let (Ok(wav_meta), Ok(pcm_bytes)) = (
                    tokio::fs::metadata(&wav_path).await,
                    tokio::fs::read(&pcm_path).await,
                )
                && wav_meta.len() > 44
            {
                let duration_ms = pcm_duration_ms(&pcm_bytes);
                pb.inc(1);
                return Result::<(usize, AudioOutput)>::Ok((
                    i,
                    AudioOutput {
                        path: wav_path,
                        duration_ms,
                    },
                ));
            }

            let pcm = gemini.generate_speech(&note, voice).await?;
            let duration_ms = pcm_duration_ms(&pcm);
            if duration_ms == 0 {
                warn!(slide = i, "Gemini returned empty audio");
            }
            tokio::fs::write(&pcm_path, &pcm).await?;
            tokio::fs::write(&wav_path, pcm_to_wav(&pcm)).await?;
            pb.inc(1);
            Ok((
                i,
                AudioOutput {
                    path: wav_path,
                    duration_ms,
                },
            ))
        }));
    }

    for h in handles {
        let (i, out) = h.await.context("audio task panicked")??;
        outputs[i] = Some(out);
    }
    Ok(outputs.into_iter().map(Option::unwrap).collect())
}

async fn encode_all<F: FfmpegAdapter + Send + Sync + 'static>(
    ffmpeg_adapter: &Arc<F>,
    ffmpeg_bin: &Path,
    segments: &[SegmentInput],
    opts: EncodeOptions,
    concurrency: usize,
    pb: &ProgressBar,
) -> Result<()> {
    let sem = std::sync::Arc::new(Semaphore::new(concurrency.max(1)));
    let mut handles = Vec::with_capacity(segments.len());
    for seg in segments {
        let sem = sem.clone();
        let ffmpeg_adapter = Arc::clone(ffmpeg_adapter);
        let ffmpeg_bin = ffmpeg_bin.to_path_buf();
        let pb = pb.clone();
        let seg = SegmentInput {
            index: seg.index,
            image_path: seg.image_path.clone(),
            audio_path: seg.audio_path.clone(),
            output_path: seg.output_path.clone(),
            duration_seconds: seg.duration_seconds,
            transition: seg.transition,
        };
        handles.push(tokio::spawn(async move {
            // Semaphore is Arc-held and never closed; acquire cannot fail.
            let _permit = sem.acquire_owned().await.unwrap();
            ffmpeg_adapter
                .encode_segment(&ffmpeg_bin, opts, &seg)
                .await?;
            pb.inc(1);
            Ok::<(), anyhow::Error>(())
        }));
    }
    for h in handles {
        h.await.context("encode task panicked")??;
    }
    Ok(())
}

fn make_pb(total: u64, label: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        // Template string is a compile-time constant; parsing cannot fail.
        ProgressStyle::with_template("{prefix:>8} [{bar:30.cyan/blue}] {pos:>3}/{len:3} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_prefix(label.to_string());
    pb
}
