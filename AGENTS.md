# SlideVoice Studio CLI (Rust) Agent Guide

## Role

You are a senior Rust engineer building **SlideVoice Studio CLI** — a
headless, scriptable video producer that mirrors the rendering pipeline
of the reference apps in `../fl-svs` (Flutter) and `../swift-svs` (Swift),
without any UI in the loop.

It takes a PDF (or a directory of slide images) and produces a narrated
MP4:

1. Rasterise the PDF into per-slide JPEGs (via `pdftoppm`).
2. Ask Gemini to write a presenter script for each slide.
3. Ask Gemini TTS to narrate each script.
4. Encode one MP4 segment per slide with FFmpeg.
5. Assemble the segments (with optional `xfade` transitions).

Build small, auditable changes. Preserve cache layout, model defaults,
voice/transition parity, and FFmpeg filter behavior unless the task
asks to change them.

## Reference Apps

Behavior parity targets:

- `../fl-svs/lib/services/export_service.dart` — FFmpeg segment +
  xfade pipeline.
- `../fl-svs/lib/services/gemini_service.dart` — notes and TTS prompts,
  models, request shapes.
- `../fl-svs/lib/services/audio_utils.dart` — PCM/WAV layout.
- `../fl-svs/lib/models/{voice_name,transition_type}.dart` — voice and
  transition enums.
- `../swift-svs/Sources/SlideVoiceStudio/Services/*.swift` — same
  pipeline in Swift; cross-check edge cases here.

Voices: `Zephyr`, `Puck`, `Charon`, `Kore`, `Fenrir`.
Transitions: `none`, `fade`, `slide`, `wipe`, `zoom`.

## Scope

This repository ships a single Rust binary (`svs`) plus its library
modules. It does not provide a server, GUI, or remote storage. The
only network calls go to the Gemini REST API.

It does:

- Convert PDFs to slide images (`pdftoppm`).
- Accept a pre-rendered slide directory as input.
- Call Gemini for notes and TTS.
- Cache notes (`.txt`) and audio (`.wav` + raw `.pcm`) on disk so
  re-runs skip paid calls.
- Encode per-slide MP4 segments with FFmpeg and assemble them.

It does not:

- Render its own PDF (delegates to Poppler).
- Encode video itself (delegates to FFmpeg).
- Manage credentials beyond reading `--api-key` / `GEMINI_API_KEY`.

## Architecture

```text
svs-cli/
├── src/
│   ├── main.rs        # binary entrypoint
│   ├── cli.rs         # clap CLI surface
│   ├── pipeline.rs    # orchestrates the end-to-end render
│   ├── gemini.rs      # Gemini REST client (notes + TTS)
│   ├── audio.rs       # PCM ↔ WAV, duration helpers
│   ├── pdf.rs         # pdftoppm wrapper, slide discovery
│   ├── ffmpeg.rs      # segment encode + concat/xfade
│   └── models.rs      # Voice, Transition enums
├── Cargo.toml
├── Taskfile.yml
├── README.md
└── AGENTS.md
```

## Runtime Requirements

- **Rust** 1.75+ (stable).
- **FFmpeg** on PATH (or in a common Homebrew prefix).
- **Poppler** (`pdftoppm`) on PATH when rendering PDFs.
- A Gemini API key supplied via `--api-key` or `GEMINI_API_KEY`. Never
  hard-code it.

## Development Commands

All common commands live in [Taskfile.yml](Taskfile.yml):

- `task` lists tasks.
- `task fmt` runs `cargo fmt`.
- `task lint` runs `cargo clippy -- -D warnings`.
- `task test` runs `cargo test`.
- `task build` runs `cargo build --release`.
- `task run -- render slides.pdf` runs the CLI in debug mode.
- `task check:all` runs fmt + lint + test.
- `task loc` counts lines with `tokei`.

Use `cargo` directly only when no Task target exists; if you reach for
it often, add a Task target.

Do not use Bun, Node, Flutter, or Swift tooling here — those belong to
the reference apps.

## Implementation Guidelines

- Keep the pipeline cache-friendly: notes and audio must be reusable
  across re-runs unless `--regenerate-*` is passed.
- Never block on long FFmpeg or Gemini calls without backpressure
  (semaphores cap concurrency).
- Treat the Gemini REST shape as the source of truth — both `inlineData`
  and `inline_data` casing must be tolerated.
- Audio durations come from the PCM byte count, not from FFmpeg.
- Keep CLI flags short, documented, and stable; add new flags rather
  than repurposing existing ones.

## Style And Formatting

- Format with `cargo fmt`; lint with `cargo clippy -- -D warnings`.
- Prefer `anyhow::Result` at API boundaries, `?` for propagation, and
  `.context(...)` for human-readable error chains.
- Keep modules small and single-purpose; prefer functions over types
  where state is not needed.

## Quality Gates

- Logic changes: `task test`.
- Surface changes (flags, help text): `task lint` and at least one
  end-to-end smoke (`task run -- render ...` against a small deck).
- Broad changes before handoff: `task check:all` and `task build`.

If a check is impossible to run in the current environment (no FFmpeg,
no API key), say so explicitly rather than implying it passed.

## Commit Conventions

Concise conventional prefixes:

- `feat` — new feature or user-visible capability
- `fix` — bug fix
- `refactor` — code improvement without behavior change
- `test` — adding or improving tests
- `docs` — documentation changes
- `chore` — tooling, dependency, or configuration changes

## Summary Mantra

Rasterise the deck. Draft the script. Speak the words. Encode the
segments. Stitch the story — automated, headless, in Rust.
