# AGENTS.md

`bpaf_derive` is the `#[derive(Bpaf)]` proc-macro crate (proc-macros must be their own crate). It has no runtime: it turns annotated `struct`/`enum` definitions into the same combinatoric parser code a user would write by hand against `bpaf`. It is enabled through `bpaf`'s `derive` feature — depend on `bpaf` with that feature, not on this crate directly.

## Pipeline — `lib.rs` runs `parse → lower → codegen`

- **Stage A · `parse/` (`mod.rs`, `input.rs`)** — a hybrid front end. `darling` (`FromDeriveInput`/`FromField`/`FromVariant`) reads the outer shape: struct vs enum, the field/variant list, doc-comment forwarding, and flat flags (`private`, `generate`, `boxed`, `ignore_rustdoc`, `path`). The ordered, position-sensitive `#[bpaf(...)]` grammar is deliberately NOT handed to darling — it is parsed by hand-written, span-carrying `syn::Parse` types kept in `attrs.rs` (`FieldAttrs`/`Consumer`/`Name`/`PostParse`/`PostDecor`/`StrictName`) and `td.rs` (`TopInfo`/`Ed`/`EAttr`/`Mode`), via `attr.parse_args`. Spans are preserved here so errors point at the right token.
- **Stage B · `lower.rs` → `ir/mod.rs`** — all resolution and validation: consumer inference, implicit `optional`/`many` insertion, naming/env split, command-name defaulting (`ident_to_long`), help splitting, enum-branch ordering. The output `ir` tree is pure data (`Debug`, **no spans, no `Result`**) so codegen cannot fail — anything invalid becomes a `compile_error!` here. Uses let-chains (edition 2024). Semantic anchors are unit-tested in `#[cfg(test)] mod golden_ir` inside `lower.rs`.
- **Stage C · `codegen.rs`** — `impl ToTokens for ir::*`, emitting `::bpaf::…` free-fn/method chains and `construct!{…}`. `custom_path.rs` (`CratePathReplacer`) rewrites `::bpaf` afterward when `#[bpaf(... path)]` is set. `field.rs` (`Shape`), `help.rs`, `utils.rs` are shared helpers.

## Invariants & tests

- **Emitted token shapes must stay byte-identical** to the previous derive: it calls the same surface (`short().long().argument::<T>()`, `positional::<T>()`, `any::<T,_,_>()`, `.many()`, `.adjacent()`, `construct!`), which now resolves to `Cx<…>` in the core — so nothing in the output string changed. `top_tests.rs` / `field_tests.rs` assert `to_token_stream().to_string()` (via `pretty_assertions`) and are the parity oracle: `cargo test -p bpaf_derive`.
- Generated parser fns emit `#[allow(unused_imports)] use ::bpaf::Parser;` — a switch-only parser never calls a trait method, so this silences a warning in *generated output*, not in this crate.
- Compile-fail messages (the Stage-B `compile_error!`s) are asserted from the **main crate** via trybuild (`../tests/ui/*.rs`), to avoid a `bpaf_derive → bpaf` dev-dependency cycle.
- Dependencies: `darling`, `syn` (full/extra-traits/visit-mut), `proc-macro2`, `quote`; keep `bpaf_derive` and `darling` on the same `syn 3.x` package along the derive path. Unrelated transitive `syn 2.x` packages elsewhere in the workspace graph do not violate this invariant.
