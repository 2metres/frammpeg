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

## AI collaborators

Automated agents follow [AGENTS.md](./AGENTS.md) — includes subagent
roles, worktree conventions, and session-completion protocol. If you are
one, read it before you touch anything.
