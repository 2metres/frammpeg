# Contributing

## Build & test

```bash
cargo run --release             # launch the app
cargo nextest run               # tests (or `cargo test`)
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

`nextest` is recommended (`cargo install cargo-nextest --locked`). If the
current version fails to install due to a yanked transitive dep, pin an
older one — see [AGENTS.md](./AGENTS.md#build--test) for the exact command.

## Spell-check

```bash
cargo install typos-cli
typos                           # check the tree
typos --write-changes           # auto-fix
```

Allowlist lives in `_typos.toml`.

## Git hooks

Pre-commit hooks (fmt, clippy) run through [lefthook](https://github.com/evilmartians/lefthook).
Install once per clone:

```bash
./scripts/install-hooks.sh
```

## Shared cargo target dir

`.cargo/config.toml` sets `target-dir = ~/.frammpeg/target-shared` so
parallel worktrees share build artifacts. There is currently no size cap
on that directory (tracked in `bd show frmpg-0.77`).

## Unused-dependency check

```bash
cargo install cargo-machete
cargo machete
# or: scripts/check-unused.sh
```

## Issue tracker

Everything is tracked in [Beads](https://github.com/gastownhall/beads).

```bash
bd ready              # available work
bd show <id>          # details
bd update <id> --claim
bd close <id>
```

The forever-lived vision ticket is `frmpg-0`. Sub-issues follow
dot-notation (`frmpg-0.<n>`, `frmpg-0.mvp`, …). Do **not** treat
`.beads/issues.jsonl` as the source of truth — the Dolt DB is.

## Style

- Rust: `cargo fmt`, `cargo clippy -- -D warnings` clean. Lint policy in
  `Cargo.toml` under `[lints]`.
- Default to no comments; add one only when the *why* is non-obvious.
- Small, focused commits. `frmpg-<id>: <one-line>` subject.

## Sending a pull request

1. **Pick or file a bead.** `bd ready` shows what's up for grabs;
   `bd create` opens a new one under `frmpg-0` if your idea isn't there
   yet. Trivial one-line fixes can skip this — anything else, get the
   context into bd first so the discussion has a home.
2. **Fork on GitHub**, or ask for direct-push access if you'll be
   contributing regularly.
3. **Branch off `main`** with the bead id in the name:
   `frmpg-<id>-<short-slug>` (e.g. `frmpg-0.42-fix-scrub-jitter`).
   Naming this way makes it easy to reconcile open branches against
   `bd list --status=open`.
4. **Commit small.** Subject line `frmpg-<id>: <one-line>`; body only
   when the *why* isn't obvious from the diff.
5. **Run the gates** before you push:
   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   cargo nextest run
   ```
   If your change touches UI, launch the app (`open target/release/frammpeg`
   on macOS after `cargo build --release`) and verify the flow you
   changed.
6. **Open the PR:**
   ```bash
   gh pr create --fill --web
   ```
   The description should say what you changed and why in one paragraph,
   link the bead (`Closes frmpg-<id>`), and include either a screenshot
   (UI change) or the test output (behavior change) that shows it works.
   Keep the diff focused — split unrelated cleanups into their own PRs.
7. **Review.** Expect a round or two of feedback. Rebase (don't merge
   `main` into your branch) if `main` moves under you; force-push the
   rebased branch. Squash-merge lands the PR as one clean commit on
   `main`.

## AI collaborators

Automated agents follow [AGENTS.md](./AGENTS.md) — includes subagent
roles, worktree conventions, and session-completion protocol. If you are
one, read it before you touch anything.
