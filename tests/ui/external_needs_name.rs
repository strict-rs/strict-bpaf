//! `external` on an unnamed field has no field name to derive the parser function from.
use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options)]
struct Opts(#[bpaf(external)] u32);

fn main() {}
