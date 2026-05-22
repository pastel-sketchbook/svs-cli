//! PDF → per-page JPEG rasterisation via the `pdftoppm` CLI (Poppler).
//!
//! This keeps the Rust binary free of native PDF dependencies. macOS users
//! install it with `brew install poppler`; Linux distros ship it as
//! `poppler-utils`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

/// Render every page of `pdf_path` into `out_dir` as JPEGs.
///
/// Returns the resulting image paths sorted by page order.
pub async fn rasterise_pdf(
    pdf_path: &Path,
    out_dir: &Path,
    dpi: u32,
    jpeg_quality: u32,
) -> Result<Vec<PathBuf>> {
    ensure_tool_available("pdftoppm", &["-v"]).await?;
    tokio::fs::create_dir_all(out_dir)
        .await
        .with_context(|| format!("creating {}", out_dir.display()))?;

    let prefix = out_dir.join("slide");
    let status = Command::new("pdftoppm")
        .arg("-jpeg")
        .arg("-jpegopt")
        .arg(format!("quality={jpeg_quality}"))
        .arg("-r")
        .arg(dpi.to_string())
        .arg(pdf_path)
        .arg(&prefix)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .context("spawning pdftoppm")?;

    if !status.success() {
        bail!("pdftoppm failed with exit status {status}");
    }

    let mut images = Vec::new();
    let mut entries = tokio::fs::read_dir(out_dir)
        .await
        .with_context(|| format!("reading {}", out_dir.display()))?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("jpg")
            && path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with("slide"))
        {
            images.push(path);
        }
    }
    images.sort();

    if images.is_empty() {
        bail!("pdftoppm produced no images for {}", pdf_path.display());
    }
    Ok(images)
}

/// Discover slide images (jpg/jpeg/png) in a directory, sorted naturally
/// by filename so `slide-001.jpg` < `slide-002.jpg`.
pub fn discover_slide_images(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut images = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if matches!(
            ext.to_ascii_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "webp"
        ) {
            images.push(path);
        }
    }
    images.sort();
    if images.is_empty() {
        bail!("no slide images found in {}", dir.display());
    }
    Ok(images)
}

async fn ensure_tool_available(name: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(name)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => bail!("`{name}` failed: {s}"),
        Err(e) => bail!(
            "`{name}` is required but was not found on PATH ({e}). \
             On macOS install with `brew install poppler`; on Debian/Ubuntu \
             with `apt install poppler-utils`."
        ),
    }
}
