# AGENTS.md

`documentation` generates `../src/_documentation.rs` — the combined Introduction / Tutorials / How-to / Cookbook / Explanation module exposed under `bpaf`'s `extradocs` feature. It is an unpublished build binary with **no dependencies** that does not link `bpaf`, so it runs even when the rest of the workspace doesn't compile.

## How it works

- `_documentation/` is the source tree: numbered section dirs (`_0_intro`, `_1_tutorials`, `_2_howto`, `_3_cookbook`, `_4_explanation`) nested arbitrarily, each level holding an `index.md`. The numeric prefixes set ordering and become module names; `index.md` headings become the section titles.
- `src/main.rs` walks the tree from `_documentation/` and `write_updated`s the assembled module into `../src/_documentation.rs`. The walker (`Entry`/`Title`, `walk`, navigation + heading emission) lives in `src/lib.rs`.
- `index.md` files mix prose with ```rust doc-test code blocks; once embedded these compile as doctests, so any type in type position and every `use` must be valid against the current `bpaf` API.

## Regenerate

- `cargo run -p documentation` — rewrites `../src/_documentation.rs`. Runs standalone (no workspace compile needed).
- **Edit the `_documentation/**/index.md` sources, never `../src/_documentation.rs`** (a build output). Intra-doc links and code blocks are copied through verbatim — fix them here, then regenerate.
