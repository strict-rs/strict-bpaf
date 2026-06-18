//! An unrecognised top-level `#[bpaf(..)]` keyword is rejected with a span-anchored error.
use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, nonsense)]
struct Opts {
    #[bpaf(short)]
    value: u32,
}

fn main() {}
