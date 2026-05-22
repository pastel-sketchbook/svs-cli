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
# From the repo root
task install        # installs `svs` into ~/.cargo/bin
```

Or build a release binary:

```sh
task build          # target/release/svs
```

## Requirements

- Rust 1.75+ (stable).
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
    --voice Kore --transition fade

# Re-run with cached notes/audio (default), but regenerate audio
svs render deck.pdf --regenerate-audio
```

### Flags

| Flag | Default | Notes |
|------|---------|-------|
| `--output, -o` | `<stem>.mp4` next to input | Final MP4 path |
| `--cache-dir` | `<stem>.svs-cache/` | Per-slide notes/audio/segments |
| `--api-key` | `$GEMINI_API_KEY` | Required |
| `--voice` | `Zephyr` | One of `Zephyr Puck Charon Kore Fenrir` |
| `--transition` | `Slide` | One of `none fade slide wipe zoom` |
| `--notes-model` | `gemini-2.5-flash` | Vision model for notes |
| `--width / --height / --fps` | `1920 / 1080 / 30` | Output video shape |
| `--gemini-concurrency` | `4` | Max parallel Gemini calls |
| `--encode-concurrency` | `cores/2` | Max parallel FFmpeg encodes |
| `--pdf-dpi` | `200` | Used by `pdftoppm` |
| `--pdf-jpeg-quality` | `85` | 1–100 |
| `--regenerate-notes / --regenerate-audio` | off | Bypass cache |
| `--keep-cache` | off | Keep segment MP4s after assembly |

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

## License

MIT.
