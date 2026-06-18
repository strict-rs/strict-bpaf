//! A bare `bool` field with no name can't be turned into a positional item.
use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options)]
struct Opts(bool);

fn main() {}
