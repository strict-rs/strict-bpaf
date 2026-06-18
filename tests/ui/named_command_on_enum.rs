//! A *named* command annotation can only sit on a struct, not an enum.
use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(command("run"))]
enum Opts {
    Alpha,
    Beta,
}

fn main() {}
