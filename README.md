# Frammpeg

A portable Rust desktop tool for debugging HTML/CSS animation bugs. Drop a
recorded video into it, scrub frame by frame, mark the moments where things
go wrong, draw shapes on top, and export annotated PNGs — then paste the
export path into an AI assistant and ask it to diagnose.

Frammpeg is deliberately **not** an MCP server, plugin, or LLM integration.
You are the courier between the tool and the assistant.

## How to run

Requirements: a recent Rust toolchain (`rustup default stable`). No system
`ffmpeg` needed — Frammpeg downloads a portable binary on first run via
[`ffmpeg-sidecar`](https://github.com/nathanbabcock/ffmpeg-sidecar).

```bash
cargo run --release
```

For a native macOS launch after building:

```bash
open target/release/frammpeg
```

Sessions land under `~/.frammpeg/sessions/<timestamp>/` and survive reboot.

## Workflow

1. **Record** the buggy animation with anything — QuickTime, OBS, Chrome
   DevTools recorder.
2. **Drop** the video onto the Frammpeg window. Frames extract to disk.
3. **Scrub** the filmstrip. Trim to just the interesting range if needed.
4. **Mark** notable frames. Draw rectangles or add text on the ones that
   show the bug.
5. **Export**. Each notable frame plus a small buffer window is written as
   PNGs (annotations burned into the noted frame), with a `note.md` per
   moment.
6. **Copy export path** → paste into the AI chat → ask for a diagnosis.

## More

- [ARCHITECTURE.md](./ARCHITECTURE.md) — stack, layers, disk layout, non-goals.
- [CONTRIBUTING.md](./CONTRIBUTING.md) — build, test, lint, and issue tracker.
- [AGENTS.md](./AGENTS.md) — conventions for AI collaborators working in this repo.
- `bd show frmpg-0` — the forever-lived vision ticket (rationale, rejected
  designs, export layout).
