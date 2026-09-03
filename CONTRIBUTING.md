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

## Reporting bugs or requesting features

Open a [GitHub issue](https://github.com/2metres/frammpeg/issues) with:

- **Bug:** what you did, what happened, what you expected. Include the
  video source, the frame index, and a screenshot if it's visual.
- **Feature:** the workflow problem you're hitting, not the
  implementation you have in mind. See the non-goals in
  [ARCHITECTURE.md](./ARCHITECTURE.md#non-goals) before filing —
  Frammpeg is deliberately small.

## Style

- Rust: `cargo fmt`, `cargo clippy -- -D warnings` clean. Lint policy in
  `Cargo.toml` under `[lints]`.
- Default to no comments; add one only when the *why* is non-obvious.
- Small, focused commits with a short subject line describing the change.

## Sending a pull request

1. **Discuss first for non-trivial changes.** Open (or comment on) a
   GitHub issue so the direction is agreed before you invest time.
   Drive-by fixes for obvious bugs or docs don't need this.
2. **Fork** the repo on GitHub.
3. **Branch off `main`** with a short, descriptive name — e.g.
   `fix-scrub-jitter` or `docs-readme-examples`.
4. **Commit small.** Focused changes are easier to review than one big
   drop.
5. **Run the gates** before you push:
   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   cargo nextest run
   ```
   If your change touches UI, launch the app (`cargo build --release`
   then `open target/release/frammpeg` on macOS) and confirm the flow
   you changed.
6. **Open the PR:**
   ```bash
   gh pr create --fill --web
   ```
   The description should say what you changed and why in one paragraph,
   link the issue (`Closes #123`), and include either a screenshot (UI
   change) or the test output (behavior change) that shows it works.
   Keep the diff focused — split unrelated cleanups into their own PRs.
7. **Review.** Expect a round or two of feedback. Rebase (don't merge
   `main` into your branch) if `main` moves under you; force-push the
   rebased branch. Squash-merge lands the PR as one clean commit on
   `main`.

## For maintainers

Internal work is tracked in a local [Beads](https://github.com/gastownhall/beads)
DB (`bd ready`, `bd show <id>`); commits reference bead ids like
`frmpg-<n>: <subject>`. AI-collaborator conventions live in
[AGENTS.md](./AGENTS.md). Neither is required knowledge for external
contributors.
