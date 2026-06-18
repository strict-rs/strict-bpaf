# AGENTS.md

`legacy` is a minimal smoke-test crate: a tiny `bpaf` parser built with the `derive`, `autocomplete`, and `docgen` features. Its only job is to compile and run the public API on the **minimum supported toolchain** (1.96.0), so an accidental rise in what `bpaf`/`bpaf_derive` require surfaces here instead of in a downstream user's build. It is not published and adds no library surface.

- CI's compat job (`.github/workflows/check-and-lint.yaml`) installs the pinned `1.96.0` toolchain and runs `cargo run --manifest-path legacy/Cargo.toml -- --help`; a failure means the core started needing a newer rustc than the declared `rust-version = "1.96"` floor.
- Keep it tiny — exercise the public API in a way that surfaces MSRV/edition regressions, nothing more.
