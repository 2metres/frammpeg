# Frammpeg — Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

## About Frammpeg

**Frammpeg** is a portable Rust desktop tool for debugging HTML/CSS animation bugs. A user records a buggy animation, drops the video into Frammpeg, scrubs frame-by-frame, marks "notable" frames, annotates them with shapes and notes, and exports the annotated frames to disk. They then paste the export path into an AI assistant chat and the assistant reads the PNGs to help diagnose.

Frammpeg is deliberately not an MCP server, not a Claude Code plugin, and has no built-in AI integration. The human is the courier between Frammpeg and the assistant. See "Non-goals" in the epic for the full list of what this tool is *not*.

**Forever-lived vision and design record:**

```bash
bd show frmpg-0
```

That epic ticket carries the workflow, stack decisions, alternatives considered (and why rejected), and the export directory layout. Read it before making architectural choices.

**Stack:**

- Rust
- [egui](https://github.com/emilk/egui) via `eframe` for the UI
- [ffmpeg-sidecar](https://github.com/nathanbabcock/ffmpeg-sidecar) for portable frame extraction (no `brew install` prerequisite)

**Current status:** pre-alpha. `frmpg-0.bootstrap` is the next actionable ticket — project scaffold and shell.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->


## Build & Test

Nothing to build yet — pre-alpha, see `bd show frmpg-0.bootstrap`.

Once the scaffold lands:

```bash
cargo run --release           # launch the desktop app
cargo test                    # unit tests
cargo clippy -- -D warnings   # lint
```

## Architecture Overview

Single-binary Rust desktop app, three layers:

- **UI** (`egui` / `eframe`): dropzone → workspace (viewport, timeline scrubber, moments panel, tool palette)
- **Extraction** (`ffmpeg-sidecar`): manages a portable ffmpeg binary; decodes video → PNG frames on disk
- **Annotation model** (in-memory JSON): shapes stored per-frame; burned into pixels on export

**Disk layout:**

- Session dir: `~/.frammpeg/sessions/<timestamp>/`
- Export dir: `<session>/export/moment-NN/` — buffer frames as clean PNGs, noted frame as `frame-NNNN-annotated.png` with shapes burned in, `note.md` per moment

Full design record and rationale live in `bd show frmpg-0`.

## Conventions & Patterns

- **All task tracking in `bd`.** The forever-lived vision is `frmpg-0`. Sub-issues use dot-notation (`frmpg-0.bootstrap`, `frmpg-0.mvp`, etc). No markdown TODO lists.
- **Non-goals are load-bearing.** They're listed in the `frmpg-0` body. Challenge any scope that contradicts them before adding it.
- **Portability is a requirement, not a preference.** Anything that assumes `brew install X` or a system dependency needs a portable replacement path called out explicitly.
- **Rust style:** `cargo fmt` on save, `cargo clippy -- -D warnings` clean before commit.

## Subagent Roles

When dispatching subagents, each one has ONE role. Do not mix roles in one run.

- **Implementer** — writes code against a specific bd ticket (or bundled tickets). Runs quality gates. Does NOT file new tickets for issues they notice outside their scope; they flag them in the handoff for the orchestrator.
- **Reviewer** — reads a diff, verifies claims, finds defects. Reports findings inline (or via `ReportFindings` where available). Does NOT edit code to fix what they find.
- **Polish agent** — sweeps a target area for defects, dead code, inconsistencies, small bugs, or missed spec items. Files each finding as a nested bd task under a parent polish ticket via `bd create --parent=<parent-id>`. Does NOT implement fixes in the same run.

Why: separating discovery from fix keeps each finding reviewable, keeps the orchestrator in charge of prioritization, prevents "helpful" polish-pass rewrites that expand scope silently, and produces a durable bd trail. A polish agent that also implements can mask its own findings by fixing them inline without a ticket.

How to apply: dispatch prompts for reviewers and polish-sweep agents must include an explicit **"you file tickets, you do NOT edit code"** clause. Implementers are dispatched afterward against the filed tickets — individually or bundled.
