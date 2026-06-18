# AGENTS.md

`docs2` generates the per-feature example pages `../src/docs2/*.md` that the library's doc comments embed via `#[doc = include_str!("docs2/<name>.md")]`. It is an unpublished build crate. It runs real parsers and captures their `--help` / output, so the rendered docs cannot drift from actual behavior.

## How it works

- Each `src/<name>/` directory is one example page (~80 of them) holding some of: `combine.rs` (a combinatoric `options()`), `derive.rs` (a derive `options()`), and a **required** `cases.md`.
- `cases.md` is a line-oriented script: `> <args>` runs the parser on those args and renders result/help/error; a bare `>` runs it with no args; `zsh> <args>` renders a live shell completion (only with the `comptester` feature, otherwise skipped); every other line is copied through as explanatory prose — **this is where intra-doc links live**.
- `build.rs` turns each directory into a generated `#[test] fn all_the_test_cases()` (emitted to `OUT_DIR`, `include!`d by `src/lib.rs`). If a `<name>/` has a matching `../examples/<name>.rs`, that example file is imported instead of `combine.rs`/`derive.rs`.
- Render helpers live in `src/lib.rs`: `run_and_render` (runs `options().run_inner(..)`, emits the `<div class='bpaf-doc'>` block), `compare_parsers` (asserts the combinatoric and derive `options()` agree), `run_and_render_completion` (the `comptester` path), `import_escaped_source`, and `write_updated` (writes `../src/docs2/<name>.md` only when changed — idempotent).

## Regenerate

- `cargo test -p docs2` — runs the generated tests, which rewrite `../src/docs2/*.md` in place. Requires the workspace to compile. Add `--features comptester` to also render the `zsh>` blocks (needs `zsh` + a pty — see `../comptester`).
- CI guards freshness with `cargo build --all-features -p docs2 && git diff-index --quiet HEAD --`.
- **Edit the sources here, never `../src/docs2/*.md`** (build outputs). Because `cases.md` prose is copied verbatim, fix link targets and wording in `cases.md`, then regenerate.
