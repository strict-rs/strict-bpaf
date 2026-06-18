//! Compile-fail coverage for `#[derive(Bpaf)]` diagnostics.
//!
//! Each case in `tests/ui/` is an intentionally-invalid derive; trybuild freezes the emitted
//! `compile_error!` (and rustc context) into the sibling `.stderr` file. Regenerate the snapshots
//! after changing a diagnostic with `TRYBUILD=overwrite cargo test --test ui`.
#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
