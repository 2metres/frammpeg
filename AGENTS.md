# Frammpeg — Agent Instructions

## About Frammpeg

**Frammpeg** is a portable Rust desktop tool for debugging HTML/CSS animation bugs. A user records a buggy animation, drops the video into Frammpeg, scrubs frame-by-frame, marks "notable" frames, annotates them with shapes and notes, and exports the annotated frames to disk. They then paste the export path into an AI assistant chat and the assistant reads the PNGs to help diagnose.

Frammpeg is deliberately not an MCP server, not a plugin, and has no built-in AI integration. The human is the courier between Frammpeg and the assistant.

**Forever-lived vision and design record:**

```bash
bd show frmpg-0
```

That epic ticket carries the workflow, stack decisions, alternatives considered (and why rejected), the export directory layout, and the non-goals list. Read it before making architectural choices.

**Stack:**

- Rust
- [egui](https://github.com/emilk/egui) via `eframe` for the UI
- [ffmpeg-sidecar](https://github.com/nathanbabcock/ffmpeg-sidecar) for portable frame extraction

Single portable binary. No `brew install ffmpeg` prerequisite.

**Current status:** pre-alpha. `frmpg-0.bootstrap` is the next actionable ticket — project scaffold and shell.

**Conventions:**

- All task tracking in `bd`. Sub-issues use dot-notation (`frmpg-0.bootstrap`, `frmpg-0.mvp`, etc).
- Non-goals in `frmpg-0` are load-bearing — challenge scope that contradicts them before adding it.
- Portability is a requirement; anything assuming a system dependency needs a portable replacement path called out.
- Rust style: `cargo fmt`, `cargo clippy -- -D warnings` clean.

## Build & Test

```bash
cargo run --release           # launch the desktop app
cargo test                    # unit tests (or use nextest)
cargo nextest run             # recommended test runner with better output
cargo clippy -- -D warnings   # lint
```

**nextest** is the recommended test runner. Install it once globally:

```bash
cargo install cargo-nextest --locked
```

If installation fails due to a yanked upstream dependency (e.g. `chacha20 v0.10.0`), pin an older nextest version:

```bash
cargo install cargo-nextest --version 0.9.72 --locked
```

Project test config lives in `.config/nextest.toml`. Both `cargo test` and `cargo nextest run` work.

## Spell-Checking

The repo uses [typos-cli](https://github.com/crate-ci/typos) to catch spelling errors in identifiers and comments.

**Global install:**
```bash
cargo install typos-cli
```

**Usage:**
```bash
typos                         # check entire repo
typos --write-changes         # auto-fix typos
```

Configuration lives in `_typos.toml` at repo root. The allowlist is there for legitimate technical terms that typos flags as errors.

**Integration:** Consider wiring typos into lefthook pre-commit as a soft check (warn-not-fail) — useful for catching obvious typos early without blocking commits when it over-flags.

## Dependency Auditing

Periodically check for unused dependencies with `cargo machete`:

```bash
cargo install cargo-machete   # one-time install
cargo machete                 # find unused deps
```

Or use the wrapper script:

```bash
scripts/check-unused.sh
```

## Git Hooks

To install pre-commit hooks (cargo fmt --check, cargo clippy), run once per clone:

```bash
./scripts/install-hooks.sh
```

This requires lefthook (`brew install lefthook` or `cargo install lefthook`). The install script will prompt if lefthook is not found.

## Architecture Overview

Single-binary Rust desktop app, three layers:

- **UI** (`egui` / `eframe`): dropzone → workspace (viewport, timeline scrubber, moments panel, tool palette)
- **Extraction** (`ffmpeg-sidecar`): manages a portable ffmpeg binary; decodes video → PNG frames on disk
- **Annotation model** (in-memory JSON): shapes stored per-frame; burned into pixels on export

**Disk layout:**

- Session dir: `~/.frammpeg/sessions/<timestamp>/`
- Export dir: `<session>/export/moment-NN/` — buffer frames as clean PNGs, noted frame as `frame-NNNN-annotated.png` with shapes burned in, `note.md` per moment

Full design record and rationale live in `bd show frmpg-0`.

---

## Bead / Tooling Instructions

This project uses **bd** (beads) for issue tracking. Run `bd prime` for full workflow context.

> **Architecture in one line:** Issues live in a local Dolt database
> (`.beads/dolt/`); cross-machine sync uses `bd dolt push/pull` (a
> git-compatible protocol), stored under `refs/dolt/data` on your git
> remote — separate from `refs/heads/*` where your code lives.
> `.beads/issues.jsonl` is a passive export, not the wire protocol.
>
> See [SYNC_CONCEPTS.md](https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md)
> for the one-screen overview and anti-patterns (don't treat JSONL as the
> source of truth; don't `bd import` during normal operation; don't
> reach for third-party Dolt hosting before trying the default).

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
bd dolt push          # Push beads data to remote
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

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

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->
