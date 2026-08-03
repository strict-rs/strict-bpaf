# AGENTS.md

`bpaf` is a command-line argument parser with a combinatoric API and a `#[derive(Bpaf)]` macro that share one synchronous runtime. This is a Cargo workspace; the library is the **root crate** (`src/`), and every sibling crate has its **own `AGENTS.md` that is authoritative for that directory** — read it before working there.

## Workspace map

- **`bpaf_derive/`** — the `#[derive(Bpaf)]` proc-macro (a `parse → lower → codegen` pipeline). → `bpaf_derive/AGENTS.md`
- **`docs2/`** — generator for the per-feature example pages `src/docs2/*.md`. → `docs2/AGENTS.md`
- **`documentation/`** — generator for the tutorial/how-to module `src/_documentation.rs`. → `documentation/AGENTS.md`
- **`comptester/`** — pty-driven shell-completion test harness. → `comptester/AGENTS.md`
- **`bpaf_cauwugo/`** — `cauwugo`, a cargo frontend dogfooding dynamic completion. → `bpaf_cauwugo/AGENTS.md`
- **`legacy/`** — minimum-toolchain smoke test. → `legacy/AGENTS.md`

## Toolchain

Edition 2024, MSRV 1.97. Stable for everything except `cargo +nightly fmt` (edition-2024 formatting) and `cargo +nightly doc2readme --expand-macros` (README).

## Commands

- **Test:** `cargo test -p bpaf -p bpaf_derive --all-targets --no-fail-fast` (also run with `--no-default-features` and `--all-features`). Single test: `cargo test -p bpaf <name>`; one integration file: `cargo test -p bpaf --test <file>`; doctests: `cargo test --doc`; compile-fail: `cargo test -p bpaf --test ui` (`TRYBUILD=overwrite` to refresh `.stderr`).
- **Lint / format:** `cargo clippy --workspace --all-targets` (+ `--no-default-features`, `--all-features`); `cargo +nightly fmt --all -- --check`.
- **Docs / MSRV:** `cargo doc --all --no-deps --all-features` under `RUSTDOCFLAGS=-Dwarnings` (broken intra-doc links fail); `cargo +1.97 build --workspace`.
- **Regenerate generated docs** (build outputs — edit the rust sources): 
  - `src/docs2/*.md` → `cargo test -p docs2` (see `docs2/AGENTS.md`);
  - `src/_documentation.rs` → `cargo run -p documentation` (see `documentation/AGENTS.md`);
  - `README.md` → `cargo +nightly doc2readme --expand-macros` (from the `src/lib.rs` `//!` docs; `+nightly --expand-macros` is required or the `include_str!`-injected content is silently dropped).

## Core Combinatoric Library (`src/`)

- **`Parser<T>` (`lib.rs`)** is the synchronous engine: `eval(&self, &mut State) -> Result<T, Error>` consumes args, `meta() -> Meta` describes structure for help/usage (both `#[doc(hidden)]`); the rest of the trait is combinators.
- **`Cx<I>` (`cx.rs`) is the single canonical builder** — a newtype over an inner state marker `I` with one blanket `impl<T, I> Parser<T> for Cx<I> where I: Parser<T>`, so `Cx<I>` is a parser exactly when its inner state is finished. Every constructor/combinator returns some `Cx<…>`; a half-built `Cx<Named>` is *not* a parser (`#[diagnostic::on_unimplemented]` explains why) until a consumer like `.switch()`/`.argument()` finishes it.
- **Inner markers** are `#[doc(hidden)] pub` in `pub mod parsers`: primitives in `params.rs` (`Named`/`Flag`/`Argument`/`Positional`/`Anything`/`Command`), combinators in `structs.rs` (`Many`/`Optional`/`Map`/`Parsed`/`Guard`/`Fallback`/`Alt`/`Con`/`Adjacent`/`StartAdjacent`/…). `many`/`some`/`collect`/`take`/`at_least`/`in_range` fold into one bounded `Many<P, C, T>`; `construct!` builds `Cx<Con<…>>` / `Cx<Alt<…>>`; `NamedArg` is a `#[deprecated]` alias for `Cx<Named>`.
- **`OptionParser<T>` (`info.rs`)** is the `.to_options()` terminal (descr/header/footer/usage/version; `run`/`run_inner`/`check_invariants`). Supporting layers by role: `args.rs`/`arg.rs` (input `State` + adjacency scanning), `meta*.rs` (the `Meta` tree → help/usage/"did you mean"), `error.rs` (`ParseFailure`/`render_message`), `buffer.rs`/`doc.rs` (the `Doc` model → markdown/html/manpage/colored terminal), `complete_*.rs` (`autocomplete` feature), `batteries.rs` (public-API helpers).

## Doc-comment wrapping

Line width is judged by **rendered** text, not raw source: a `[display](target)` intra-doc link costs only `display` (the `(target)` is invisible once rendered), so source lines with long link targets are intentionally long — wrap so the *visible* width is ~95–100, and don't reflow to a raw 100-column count.

## Commit messages

- **Subject:** `type(scope): structural imperative description/summary`. `type` is one of `feat` / `fix` / `refactor` / `docs` / `test` / `build` / `ci` / `perf` / `style`; `scope` is required. **`chore` is never valid** — pick the precise type (dep bump → `build`, CI tweak → `ci`, doc edit → `docs`, formatting-only → `style`).
- **Body:** 1 to 5 sections. Each section starts with a plain-text heading line (no `#` markers, no trailing colon, no underline) immediately followed by a 3-to-5-item bullet list (no blank line between heading and bullets). Sections are separated from each other by exactly one blank line. No prose paragraphs — bullets only. Do not pad to multiple sections artificially; one section is fine for small changes.
