//! FFmpeg-driven video export. Mirrors the desktop pipeline from
//! `fl-svs/lib/services/export_service.dart`:
//!
//! 1. Encode each slide+narration pair into its own MP4 segment.
//! 2. Either `concat`-copy them (when every transition is `None`) or
//!    glue them with `xfade` filters that preserve the full narration.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

use crate::models::Transition;

const XFADE_DURATION_SECS: f64 = 0.75;

pub struct SegmentInput {
    pub index: usize,
    pub image_path: PathBuf,
    pub audio_path: PathBuf,
    pub output_path: PathBuf,
    pub duration_seconds: f64,
    pub transition: Transition,
}

#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub preset: &'static str,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
            preset: "veryfast",
        }
    }
}

/// Locate an `ffmpeg` binary. GUI macOS apps lose `PATH`, but a CLI
/// inherits it; we still probe Homebrew prefixes as a fallback for users
/// invoking us from a stripped shell (cron, launchd).
pub async fn locate_ffmpeg() -> Result<PathBuf> {
    let candidates = [
        "ffmpeg",
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
        "/opt/local/bin/ffmpeg",
    ];
    for candidate in candidates {
        let ok = Command::new(candidate)
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success());
        if ok {
            return Ok(PathBuf::from(candidate));
        }
    }
    bail!(
        "ffmpeg is required for video export. Install it (e.g. `brew install ffmpeg` \
         or `apt install ffmpeg`) and try again."
    )
}

pub async fn encode_segment(ffmpeg: &Path, opts: EncodeOptions, seg: &SegmentInput) -> Result<()> {
    let video_filter = format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,\
         pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,format=yuv420p",
        w = opts.width,
        h = opts.height,
    );
    let duration = format!("{:.3}", seg.duration_seconds);
    let args = [
        "-y",
        "-loop",
        "1",
        "-i",
        seg.image_path.to_str().context("non-UTF8 image path")?,
        "-i",
        seg.audio_path.to_str().context("non-UTF8 audio path")?,
        "-t",
        &duration,
        "-vf",
        &video_filter,
        "-r",
        &opts.fps.to_string(),
        "-c:v",
        "libx264",
        "-preset",
        opts.preset,
        "-c:a",
        "aac",
        "-shortest",
        seg.output_path.to_str().context("non-UTF8 output path")?,
    ];
    run_ffmpeg(ffmpeg, &args).await
}

pub async fn assemble(
    ffmpeg: &Path,
    segments: &[SegmentInput],
    work_dir: &Path,
    output_path: &Path,
    opts: EncodeOptions,
) -> Result<()> {
    let has_transitions = segments
        .iter()
        .skip(1)
        .any(|s| s.transition != Transition::None);

    if segments.len() < 2 || !has_transitions {
        return concat_copy(ffmpeg, segments, work_dir, output_path).await;
    }
    xfade_assemble(ffmpeg, segments, output_path, opts).await
}

async fn concat_copy(
    ffmpeg: &Path,
    segments: &[SegmentInput],
    work_dir: &Path,
    output_path: &Path,
) -> Result<()> {
    let concat_file = work_dir.join("segments.txt");
    let mut body = String::new();
    for s in segments {
        use std::fmt::Write;
        writeln!(
            body,
            "file '{}'",
            s.output_path.to_string_lossy().replace('\'', r"'\''")
        )
        .unwrap();
    }
    tokio::fs::write(&concat_file, body)
        .await
        .context("writing ffmpeg concat list")?;

    let args = [
        "-y",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        concat_file.to_str().context("non-UTF8 concat path")?,
        "-c",
        "copy",
        output_path.to_str().context("non-UTF8 output path")?,
    ];
    run_ffmpeg(ffmpeg, &args).await
}

async fn xfade_assemble(
    ffmpeg: &Path,
    segments: &[SegmentInput],
    output_path: &Path,
    _opts: EncodeOptions,
) -> Result<()> {
    let mut args: Vec<String> = vec!["-y".to_string()];
    for seg in segments {
        args.push("-i".to_string());
        args.push(
            seg.output_path
                .to_str()
                .context("non-UTF8 segment path")?
                .to_string(),
        );
    }

    let mut filters: Vec<String> = Vec::new();
    for i in 0..segments.len() {
        filters.push(format!(
            "[{i}:v]settb=AVTB,setpts=PTS-STARTPTS,setsar=1[v{i}];\
             [{i}:a]asetpts=PTS-STARTPTS[a{i}]"
        ));
    }

    let mut video_label = "v0".to_string();
    let mut audio_label = "a0".to_string();
    let mut elapsed = segments[0].duration_seconds;

    #[allow(clippy::needless_range_loop)]
    for i in 1..segments.len() {
        let transition = segments[i].transition;
        let offset = elapsed.max(0.01);
        let padded_video_label = format!("vp{i}");
        let next_video_label = format!("vx{i}");
        let next_audio_label = format!("ax{i}");

        if transition == Transition::None {
            filters.push(format!(
                "[{video_label}][{audio_label}][v{i}][a{i}]\
                 concat=n=2:v=1:a=1[{next_video_label}][{next_audio_label}]"
            ));
        } else {
            filters.push(format!(
                "[{video_label}]tpad=stop_mode=clone:\
                 stop_duration={XFADE_DURATION_SECS}[{padded_video_label}]"
            ));
            filters.push(format!(
                "[{padded_video_label}][v{i}]xfade=transition={kind}:\
                 duration={XFADE_DURATION_SECS}:offset={offset:.3}[{next_video_label}]",
                kind = transition.xfade_name(),
            ));
            filters.push(format!(
                "[{audio_label}][a{i}]concat=n=2:v=0:a=1[{next_audio_label}]"
            ));
        }

        video_label = next_video_label;
        audio_label = next_audio_label;
        elapsed += segments[i].duration_seconds;
    }

    args.push("-filter_complex".to_string());
    args.push(filters.join(";"));
    args.push("-map".to_string());
    args.push(format!("[{video_label}]"));
    args.push("-map".to_string());
    args.push(format!("[{audio_label}]"));
    args.push("-c:v".to_string());
    args.push("libx264".to_string());
    args.push("-preset".to_string());
    args.push("veryfast".to_string());
    args.push("-pix_fmt".to_string());
    args.push("yuv420p".to_string());
    args.push("-c:a".to_string());
    args.push("aac".to_string());
    args.push(
        output_path
            .to_str()
            .context("non-UTF8 output path")?
            .to_string(),
    );

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ffmpeg(ffmpeg, &arg_refs).await
}

async fn run_ffmpeg(ffmpeg: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new(ffmpeg)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("spawning {}", ffmpeg.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffmpeg failed ({}): {stderr}", output.status);
    }
    Ok(())
}

// ─── Adapter trait impl ──────────────────────────────────────────────

/// Default FFmpeg adapter using the system `ffmpeg` binary.
#[derive(Debug, Clone, Copy)]
pub struct FfmpegService;

impl crate::adapters::FfmpegAdapter for FfmpegService {
    async fn locate(&self) -> Result<PathBuf> {
        locate_ffmpeg().await
    }

    async fn encode_segment(
        &self,
        ffmpeg: &Path,
        opts: EncodeOptions,
        seg: &SegmentInput,
    ) -> Result<()> {
        encode_segment(ffmpeg, opts, seg).await
    }

    async fn assemble(
        &self,
        ffmpeg: &Path,
        segments: &[SegmentInput],
        work_dir: &Path,
        output_path: &Path,
        opts: EncodeOptions,
    ) -> Result<()> {
        assemble(ffmpeg, segments, work_dir, output_path, opts).await
    }
}
