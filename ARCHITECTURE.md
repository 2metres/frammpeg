# Architecture

Single-binary Rust desktop app, three layers.

## Stack

- **[egui](https://github.com/emilk/egui)** via `eframe` — cross-platform
  immediate-mode GUI. Custom `Visuals`, embedded Inter + JetBrains Mono, and
  Lucide icons carry it from "90s debug UI" defaults to something readable.
  [Rerun](https://rerun.io) is proof of what egui can look like when styled.
- **[ffmpeg-sidecar](https://github.com/nathanbabcock/ffmpeg-sidecar)** —
  downloads and manages a portable `ffmpeg` binary at runtime. No
  `brew install` prerequisite; the app stays portable.
- **[image](https://crates.io/crates/image)** + **[imageproc](https://crates.io/crates/imageproc)**
  for annotation burn; **[resvg](https://crates.io/crates/resvg)** for icon
  rasterization.

## Layers

- **UI** (`src/app.rs`, `src/transport.rs`, `src/filmstrip.rs`, `src/theme.rs`,
  `src/icons.rs`) — dropzone → viewport → transport row → filmstrip →
  moments panel. Immediate mode; state lives on `FrammpegApp`.
- **Extraction** (`src/extract.rs`, `src/session.rs`, `src/thumbs.rs`) —
  ffmpeg-sidecar decodes the video into per-frame PNGs on disk. A thumb
  cache serves the filmstrip.
- **Annotation model** (`src/model.rs`, `src/annotate.rs`, `src/history.rs`) —
  shapes stored per-frame in memory, with an undo/redo history. On export
  they're burned into the noted frame's pixels.

## Session disk layout

```
~/.frammpeg/sessions/<timestamp>/
├── frames/                             # all extracted frames
└── export/
    ├── moment-01/
    │   ├── frame-0038.png              # buffer, clean
    │   ├── frame-0039.png              # buffer, clean
    │   ├── frame-0040-annotated.png    # noted, shapes burned in
    │   ├── frame-0041.png              # buffer, clean
    │   ├── frame-0042.png              # buffer, clean
    │   └── note.md
    └── moment-02/…
```

Sessions survive reboot. Cleanup is manual for now.

## Non-goals

- MCP server or Claude Code skill integration — human is the courier.
- WebCodecs / browser rendering — evaluated, rejected as incompatible with
  a Rust GUI stack.
- Automatic bug or jank detection — human eyes decide what's notable.
- Team sharing or collaboration — personal debugging tool.
- Video re-encoding — read-only, extract-and-view only.

## Rejected approaches

- **Buntralino (Bun + Neutralino) + WebCodecs** — cleanest zero-bundle path
  (Chromium ships hardware video decode). Rejected in favor of a single
  portable Rust binary.
- **Python + `imageio-ffmpeg` via `uv`** — heavier stack (Python runtime +
  ffmpeg); only earns its keep if numpy/scipy/OpenCV frame analysis becomes
  a goal.
- **[GPUI](https://www.gpui.rs/)** (Zed's GUI) — GPU-accelerated and
  beautiful, but not on crates.io (git-dep on the Zed monorepo), the API
  churns, docs are sparse. egui is the pragmatic Rust GUI choice at this
  size.
- **npm `ffmpeg-static`** — same idea as ffmpeg-sidecar, wrong ecosystem
  for a Rust binary.

Full design record and rationale live in `bd show frmpg-0`.
