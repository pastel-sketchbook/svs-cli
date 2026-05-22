//! Adapter traits — testable boundaries for external dependencies.
//!
//! Mirrors the adapter-protocol pattern from swift-svs, scoped to what
//! a headless CLI actually needs: AI (notes + TTS), video encoding, and
//! PDF rasterisation.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::ffmpeg::{EncodeOptions, SegmentInput};
use crate::models::Voice;

// ─── Gemini (AI) ─────────────────────────────────────────────────────

/// Adapter for AI operations: note extraction and speech synthesis.
pub trait GeminiAdapter: Send + Sync {
    /// Extract presenter notes from a slide image.
    fn extract_notes(
        &self,
        image_jpeg: &[u8],
        is_first_slide: bool,
        notes_model: &str,
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Generate speech audio (raw PCM) from text.
    fn generate_speech(
        &self,
        text: &str,
        voice: Voice,
    ) -> impl std::future::Future<Output = Result<Vec<u8>>> + Send;
}

// ─── FFmpeg (Video) ──────────────────────────────────────────────────

/// Adapter for video encoding and assembly.
pub trait FfmpegAdapter: Send + Sync {
    /// Locate the ffmpeg binary.
    fn locate(&self) -> impl std::future::Future<Output = Result<PathBuf>> + Send;

    /// Encode a single slide+audio pair into an MP4 segment.
    fn encode_segment(
        &self,
        ffmpeg: &Path,
        opts: EncodeOptions,
        seg: &SegmentInput,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Assemble segments into a final MP4.
    fn assemble(
        &self,
        ffmpeg: &Path,
        segments: &[SegmentInput],
        work_dir: &Path,
        output_path: &Path,
        opts: EncodeOptions,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ─── PDF ─────────────────────────────────────────────────────────────

/// Adapter for PDF rasterisation and slide image discovery.
pub trait PdfAdapter: Send + Sync {
    /// Rasterise a PDF into per-page JPEG images.
    fn rasterise(
        &self,
        pdf_path: &Path,
        out_dir: &Path,
        dpi: u32,
        jpeg_quality: u32,
    ) -> impl std::future::Future<Output = Result<Vec<PathBuf>>> + Send;

    /// Discover existing slide images in a directory.
    fn discover_images(&self, dir: &Path) -> Result<Vec<PathBuf>>;
}
