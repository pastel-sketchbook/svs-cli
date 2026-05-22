# 0001 — Headless CLI Architecture

## Context

SlideVoice Studio exists in three forms:

- **fl-svs** (Flutter) — desktop GUI with Riverpod state management.
- **swift-svs** (Swift) — native macOS GUI with @Observable view models.
- **svs-cli** (Rust) — this project: headless, scriptable, no UI.

All three share the same production pipeline: rasterise slides → extract
notes via Gemini vision → synthesize narration via Gemini TTS → encode
segments with FFmpeg → assemble the final MP4. The GUI apps expose this
through interactive controls; the CLI automates it end-to-end.

## Decision

Build the CLI as a single Rust binary that delegates all heavy lifting
to external tools (FFmpeg, pdftoppm) and the Gemini REST API, with no
native rendering or encoding.

## Rationale

### Why Rust, not a script or the existing Flutter/Swift code?

1. **Single static binary** — no runtime, no framework, no app bundle.
   Runs in CI, cron, SSH sessions, containers.
2. **Async concurrency without GC pauses** — tokio semaphores give the
   same bounded parallelism as Flutter's `Future.wait` worker pattern
   (see `fl-svs/lib/services/export_service.dart` `_runBounded()`) but
   with predictable memory.
3. **Cross-platform without compromise** — compiles for Linux/macOS/
   Windows from one source. The GUI apps are platform-specific by nature.

### Why delegate to FFmpeg and pdftoppm?

Same reasoning as `fl-svs` rationale
[0002_ffmpeg-and-macos-sandbox.md](../../fl-svs/docs/rationale/0002_ffmpeg-and-macos-sandbox.md):
FFmpeg is the industry standard for video encoding. Writing our own
encoder gains nothing and loses codec coverage, hardware acceleration,
and community trust. pdftoppm (Poppler) is the standard PDF rasteriser
on Unix.

### Why bounded-concurrency semaphores, not cross-stage pipelining?

The pipeline is staged: all notes → all audio → all encodes → assemble.
Within each stage, work runs in parallel bounded by a semaphore (default
4 for Gemini, cores/2 for FFmpeg — matching `fl-svs` exactly).

Cross-stage pipelining (start TTS for slide 1 while extracting notes for
slide 5) was considered and rejected:

- Gemini rate limits are the dominant bottleneck, not local compute.
- Staged execution makes caching and resumption trivial: check if the
  output file exists, skip if yes.
- Debugging is simpler when each stage completes fully before the next.
- The Flutter and Swift apps use the same staged model.

### Why a file-based cache, not a database?

The cache is a directory of plain files (`notes/*.txt`, `audio/*.wav`,
`audio/*.pcm`, `segments/*.mp4`). Benefits:

- Inspectable — open any intermediate with a text editor or media player.
- Resumable — re-run skips slides whose cache files exist.
- No migrations — add new file types without schema changes.
- Portable — move or copy the cache directory between machines.
- Matches fl-svs and swift-svs, which use the same sidecar pattern.

### Why embed prompts in a RON file?

Prompts and model names live in `prompts.ron`, compiled into the binary
via `include_str!`. This separates content (prompts) from logic (HTTP
calls, parsing), making prompt iteration a text edit rather than a code
change. RON is used over TOML/JSON because it supports Rust-native
syntax (multi-line strings, comments) without external formatting rules.

## Consequences

- The binary has zero network calls except to the Gemini API.
- Users must install FFmpeg and (for PDF input) pdftoppm themselves.
- Prompt changes require recompilation (acceptable for a CLI tool where
  the user owns the build).
- The same cache layout means fl-svs, swift-svs, and svs-cli can share
  intermediate files if needed.
