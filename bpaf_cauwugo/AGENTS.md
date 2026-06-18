# AGENTS.md

`bpaf_cauwugo` builds the `cauwugo` binary — an alternative `cargo` frontend that dogfoods bpaf's dynamic shell completion (completing cargo subcommands, their options and option *values*, test names, and workspace binaries). It is a demonstration / eat-our-own-dogfood crate, not part of the library's published surface or core test matrix.

## Layout

- `src/main.rs` is the `cauwugo` binary entry (`[[bin]] name = "cauwugo"`); `src/lib.rs` wires the top-level parser.
- One module per wrapped cargo subcommand: `build.rs`, `check.rs`, `test.rs`, `run.rs`, `add.rs`, `clean.rs`. `opts.rs` holds shared option parsers and `shared.rs` shared helpers.
- `metadata.rs` uses `cargo_metadata` to enumerate the workspace (targets, test names, binaries) so parsers can offer those as **dynamic** completions — this is the part that exercises bpaf's completion under realistic, data-driven conditions.
- Built with `bpaf`'s `derive` + `autocomplete` features; switched on for the workspace via `[workspace.metadata.cauwugo] bpaf = true` in the root `Cargo.toml`.
