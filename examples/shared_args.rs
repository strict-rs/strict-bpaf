//! It is possible to have shared args returned alongside a subcommand

use bpaf::*;

#[derive(Debug, Clone, Bpaf)]
struct Action {
    verbose: bool,
    number: u32,
}

#[derive(Debug, Clone, Bpaf)]
struct Build {
    verbose: bool,
}

#[derive(Debug, Clone)]
enum Command {
    Action(Action),
    Build(Build),
}

fn shared() -> impl Parser<Vec<String>> {
    positional("ARG").many()
}

fn parse_command() -> impl Parser<(Command, Vec<String>)> {
    let action = action().map(Command::Action);
    let action = construct!(action, shared()).to_options().command("action");
    let build = build().map(Command::Build);
    let build = construct!(build, shared()).to_options().command("build");
    construct!([action, build])
}

fn print_command(command: Command, args: Vec<String>) {
    match command {
        Command::Action(Action { verbose, number }) => {
            println!("action: verbose={verbose}, number={number}, args={args:?}");
        }
        Command::Build(Build { verbose }) => {
            println!("build: verbose={verbose}, args={args:?}");
        }
    }
}

fn main() {
    let (command, args) = parse_command().to_options().run();

    print_command(command, args);
}
