/// parser inspired by https://github.com/hood/rudo/blob/e448942b752c56dd2be2e2bb5026ced45e215ed6/src/main.rs
///
use bpaf::*;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options)]
struct Options {
    /// help
    #[bpaf(external, fallback(Action::List))]
    action: Action,
}

#[derive(Debug, Clone, Bpaf)]
enum Action {
    /// Add a new TODO item
    #[bpaf(command)]
    Add(String),

    /// Mark nth item as done
    #[bpaf(command)]
    Mark(usize),

    /// Read nth item
    #[bpaf(command)]
    Read(usize),

    /// Lists everything
    // name argument for command is optional
    #[bpaf(command("list"))]
    List,
}

fn print_action(action: Action) {
    match action {
        Action::Add(item) => println!("add: {item}"),
        Action::Mark(item) => println!("mark: {item}"),
        Action::Read(item) => println!("read: {item}"),
        Action::List => println!("list"),
    }
}

fn main() {
    let Options { action } = options().run();
    print_action(action);
}
