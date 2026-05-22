use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::ffmpeg::{EncodeOptions, FfmpegService};
use crate::gemini::{GeminiClient, default_notes_model};
use crate::models::{Transition, Voice};
use crate::pdf::PdfService;
use crate::pipeline::{self, Adapters, RenderOptions};

/// Automated SlideVoice video production CLI.
///
/// Takes a PDF or a directory of slide images and produces a narrated
/// MP4: Gemini writes per-slide notes, Gemini TTS narrates them, and
/// FFmpeg composes the final video — no UI in the loop.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Render the full pipeline: slides → notes → narration → MP4.
    ///
    /// Usage: svs render <INPUT> [OPTIONS]
    ///
    /// Examples:
    ///   svs render slides.pdf
    ///   svs render ./slide-images/ --voice kore --transition fade
    Render(RenderArgs),
}

#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)]
struct RenderArgs {
    /// Input: a PDF file, or a directory of slide images (jpg/png/webp).
    input: PathBuf,

    /// Output MP4 path. Defaults to `<input-stem>.mp4` next to the input.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Cache/working directory for intermediate notes, audio, and segments.
    /// Defaults to `<input-stem>.svs-cache/` next to the input.
    #[arg(long)]
    cache_dir: Option<PathBuf>,

    /// Gemini API key. Falls back to the `GEMINI_API_KEY` env var.
    #[arg(long, env = "GEMINI_API_KEY", hide_env_values = true)]
    api_key: String,

    /// TTS voice.
    #[arg(long, value_enum, default_value_t = Voice::Zephyr)]
    voice: Voice,

    /// Per-slide entry transition.
    #[arg(long, value_enum, default_value_t = Transition::Slide)]
    transition: Transition,

    /// Gemini vision model used for note extraction.
    #[arg(long)]
    notes_model: Option<String>,

    /// Output video width in pixels.
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Output video height in pixels.
    #[arg(long, default_value_t = 1080)]
    height: u32,

    /// Output video frame rate.
    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// Max concurrent Gemini requests (notes + TTS).
    #[arg(long, default_value_t = 4)]
    gemini_concurrency: usize,

    /// Max concurrent FFmpeg segment encodes.
    #[arg(long, default_value_t = default_encode_concurrency())]
    encode_concurrency: usize,

    /// DPI for PDF rasterisation via pdftoppm.
    #[arg(long, default_value_t = 200)]
    pdf_dpi: u32,

    /// JPEG quality (1–100) for PDF rasterisation.
    #[arg(long, default_value_t = 85)]
    pdf_jpeg_quality: u32,

    /// Keep the per-segment MP4s in the cache after assembly.
    #[arg(long)]
    keep_cache: bool,

    /// Force regeneration of cached notes.
    #[arg(long)]
    regenerate_notes: bool,

    /// Force regeneration of cached audio.
    #[arg(long)]
    regenerate_audio: bool,

    /// Resume a previous incomplete render without prompting.
    #[arg(long, conflicts_with = "clear")]
    resume: bool,

    /// Clear existing cache and start fresh without prompting.
    #[arg(long, conflicts_with = "resume")]
    clear: bool,
}

fn default_encode_concurrency() -> usize {
    std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4))
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Render(args) => render(args).await,
        }
    }
}

async fn render(args: RenderArgs) -> Result<()> {
    let input = args
        .input
        .canonicalize()
        .with_context(|| format!("resolving input {}", args.input.display()))?;

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("slidevoice")
        .to_string();
    let parent = input.parent().unwrap_or_else(|| std::path::Path::new("."));

    let output = args
        .output
        .unwrap_or_else(|| parent.join(format!("{stem}.mp4")));
    let cache_dir = args
        .cache_dir
        .unwrap_or_else(|| parent.join(format!("{stem}.svs-cache")));

    // Handle resume/clear logic when cache already exists.
    if cache_dir.exists() {
        // Synchronous read_dir is acceptable here: it's a single non-blocking
        // metadata check before any heavy async work begins.
        let has_content = std::fs::read_dir(&cache_dir).is_ok_and(|mut d| d.next().is_some());

        if has_content {
            if args.clear {
                tracing::info!("clearing existing cache");
                tokio::fs::remove_dir_all(&cache_dir).await.ok();
            } else if !args.resume {
                // Interactive prompt.
                let choice = prompt_resume_or_clear(&cache_dir)?;
                if choice == CacheChoice::Clear {
                    tracing::info!("clearing existing cache");
                    tokio::fs::remove_dir_all(&cache_dir).await.ok();
                } else {
                    tracing::info!("resuming from existing cache");
                }
            } else {
                tracing::info!("resuming from existing cache");
            }
        }
    }

    let opts = RenderOptions {
        input,
        output,
        cache_dir,
        voice: args.voice,
        transition: args.transition,
        notes_model: args
            .notes_model
            .unwrap_or_else(|| default_notes_model().to_string()),
        gemini_concurrency: args.gemini_concurrency,
        encode_concurrency: args.encode_concurrency,
        encode: EncodeOptions {
            width: args.width,
            height: args.height,
            fps: args.fps,
            preset: "veryfast",
        },
        pdf_dpi: args.pdf_dpi,
        pdf_jpeg_quality: args.pdf_jpeg_quality,
        keep_cache: args.keep_cache,
        regenerate_notes: args.regenerate_notes,
        regenerate_audio: args.regenerate_audio,
    };

    let adapters = Adapters {
        gemini: Arc::new(GeminiClient::new(args.api_key)?),
        ffmpeg: Arc::new(FfmpegService),
        pdf: PdfService,
    };

    let path = pipeline::render(opts, adapters).await?;
    println!("✓ wrote {}", path.display());
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheChoice {
    Resume,
    Clear,
}

fn prompt_resume_or_clear(cache_dir: &std::path::Path) -> Result<CacheChoice> {
    eprintln!("\n  Existing cache found: {}\n", cache_dir.display());
    eprintln!("  [r] Resume previous production");
    eprintln!("  [c] Clear cache and start fresh");
    eprint!("\n  Choice [r/c] (default: r): ");
    std::io::stderr().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_ascii_lowercase();

    if trimmed == "c" || trimmed == "clear" {
        Ok(CacheChoice::Clear)
    } else {
        Ok(CacheChoice::Resume)
    }
}
