# SlideVoice Studio CLI

Headless, scriptable video producer for the SlideVoice Studio pipeline.

Given a PDF (or a folder of slide images), `svs` produces a narrated MP4
without any UI in the loop:

1. **Rasterise** — convert the PDF to per-slide JPEGs (via `pdftoppm`).
2. **Notes** — ask Gemini to write a presenter script for each slide.
3. **Narrate** — ask Gemini TTS to speak each script (PCM → WAV).
4. **Encode** — FFmpeg renders one MP4 segment per slide.
5. **Assemble** — concat or `xfade` the segments into the final MP4.

It mirrors the export pipeline of the reference apps
[`../fl-svs`](../fl-svs) (Flutter) and [`../swift-svs`](../swift-svs)
(Swift), so you can automate the same production end-to-end.

## Install

```sh
task install        # builds release and copies `svs` to ~/bin/
```

Or build without installing:

```sh
task build          # target/release/svs
```

## Requirements

- Rust 1.95+ (edition 2024).
- `ffmpeg` on PATH (`brew install ffmpeg`).
- `pdftoppm` on PATH for PDF input (`brew install poppler` /
  `apt install poppler-utils`).
- A Gemini API key in `GEMINI_API_KEY` or passed via `--api-key`.

## Usage

```sh
# From a PDF
export GEMINI_API_KEY=...
svs render deck.pdf

# From a folder of slide images (sorted by filename)
svs render ./slides --output presentation.mp4 \
    --voice kore --transition fade

# Resume an interrupted render
svs render deck.pdf --resume

# Clear cache and start fresh
svs render deck.pdf --clear

# Re-run with cached notes but regenerate audio
svs render deck.pdf --regenerate-audio

# Replace only the first slide image (from another PDF's first page)
svs render deck.pdf --replace-slide "1:~/Desktop/new_cover.pdf"

# Remove slide 2 from the deck (produces N-1 slides)
svs render deck.pdf --remove-slide 2 --resume
```

### Flags

| Flag | Default | Notes |
|------|---------|-------|
| `--output, -o` | `<stem>.mp4` next to input | Final MP4 path |
| `--cache-dir` | `<stem>.svs-cache/` | Per-slide notes/audio/segments |
| `--api-key` | `$GEMINI_API_KEY` | Required |
| `--voice` | `zephyr` | One of `zephyr puck charon kore fenrir` |
| `--transition` | `slide` | One of `none fade slide wipe zoom` |
| `--notes-model` | `gemini-2.5-flash` | Vision model for notes |
| `--width / --height / --fps` | `1920 / 1080 / 30` | Output video shape |
| `--gemini-concurrency` | `4` | Max parallel Gemini calls |
| `--encode-concurrency` | `cores/2` | Max parallel FFmpeg encodes |
| `--pdf-dpi` | `200` | Used by `pdftoppm` |
| `--pdf-jpeg-quality` | `85` | 1–100 |
| `--resume` | — | Resume without prompting |
| `--clear` | — | Clear cache without prompting |
| `--regenerate-notes / --regenerate-audio` | off | Bypass cache |
| `--replace-slide` | — | Replace slide image by index (`1:path`). Accepts images or PDFs (first page). Repeatable. |
| `--remove-slide` | — | Remove a slide by index (1-based). Repeatable. |
| `--keep-cache` | off | Keep segment MP4s after assembly |

## Resumable Production

When a cache directory already exists from a previous run, `svs` will
prompt interactively:

```
  Existing cache found: deck.svs-cache/

  [r] Resume previous production
  [c] Clear cache and start fresh

  Choice [r/c] (default: r):
```

Use `--resume` or `--clear` to skip the prompt in scripts.

## Configuration

Prompts and model names live in [`prompts.ron`](prompts.ron) (embedded at
compile time). Edit this file to tweak Gemini prompts or switch models
without changing Rust code.

## Cache Layout

```text
<stem>.svs-cache/
├── images/                  # rasterised slides (PDF input only)
├── notes/slide_0000.txt     # presenter scripts
├── audio/slide_0000.{wav,pcm}
└── segments/segment_0000.mp4
```

Notes and audio are reused on subsequent runs to keep iteration cheap
and Gemini bills small.

## Versioning

```sh
task version:patch   # 0.1.0 → 0.1.1
task version:minor   # 0.1.0 → 0.2.0
task version:major   # 0.1.0 → 1.0.0
task version:tag     # commit + git tag
```

VERSION file is the single source of truth; `Cargo.toml` is synced
automatically.

## License

[MIT](LICENSE)
