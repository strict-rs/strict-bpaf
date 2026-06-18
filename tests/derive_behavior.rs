//! Behavioral coverage for the `#[derive(Bpaf)]` macro.
//!
//! Unlike the token-level snapshot tests inside `bpaf_derive`, these assert the *runtime* behavior
//! of the generated parsers (parse results, fallbacks, command dispatch, adjacency) through
//! `run_inner`, so they survive incidental changes to the emitted token shape and document what the
//! derive is actually expected to do.

use bpaf::*;

#[test]
fn switch_and_argument() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    struct Basic {
        /// be loud
        verbose: bool,
        /// who to greet
        name: String,
    }

    let parser = basic();

    assert_eq!(
        parser.run_inner(&["--name", "Bob"]).unwrap(),
        Basic {
            verbose: false,
            name: "Bob".to_owned(),
        }
    );
    assert_eq!(
        parser.run_inner(&["--verbose", "--name", "Alice"]).unwrap(),
        Basic {
            verbose: true,
            name: "Alice".to_owned(),
        }
    );

    // a required argument with no value is an error
    assert!(parser.run_inner(&[]).is_err());
}

#[test]
fn optional_and_many_are_implicit() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    struct Coll {
        #[bpaf(long("item"))]
        items: Vec<String>,
        maybe: Option<u32>,
    }

    let parser = coll();

    assert_eq!(
        parser.run_inner(&[]).unwrap(),
        Coll {
            items: Vec::new(),
            maybe: None,
        }
    );
    assert_eq!(
        parser
            .run_inner(&["--item", "a", "--item", "b", "--maybe", "5"])
            .unwrap(),
        Coll {
            items: vec!["a".to_owned(), "b".to_owned()],
            maybe: Some(5),
        }
    );
}

#[test]
fn positional_tuple_struct() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    struct Pos(#[bpaf(positional("FILE"))] String);

    let parser = pos();
    assert_eq!(
        parser.run_inner(&["hello.txt"]).unwrap(),
        Pos("hello.txt".to_owned())
    );
    assert!(parser.run_inner(&[]).is_err());
}

#[test]
fn metavar_is_explicit_when_given() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    struct Opts {
        #[bpaf(long, argument("LEVEL"))]
        log: u32,
    }

    let help = opts().run_inner(&["--help"]).unwrap_err().unwrap_stdout();
    assert!(
        help.contains("LEVEL"),
        "metavar should appear in help:\n{help}"
    );
    assert_eq!(opts().run_inner(&["--log", "4"]).unwrap(), Opts { log: 4 });
}

#[test]
fn enum_commands_dispatch() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    enum Cmd {
        #[bpaf(command)]
        Add(#[bpaf(positional("ITEM"))] String),
        #[bpaf(command)]
        Remove(#[bpaf(positional("ITEM"))] String),
    }

    let parser = cmd();
    assert_eq!(
        parser.run_inner(&["add", "milk"]).unwrap(),
        Cmd::Add("milk".to_owned())
    );
    assert_eq!(
        parser.run_inner(&["remove", "eggs"]).unwrap(),
        Cmd::Remove("eggs".to_owned())
    );
}

#[test]
fn external_composition() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    struct Inner {
        #[bpaf(short)]
        value: u32,
    }

    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    struct Outer {
        #[bpaf(external)]
        inner: Inner,
        #[bpaf(switch)]
        flag: bool,
    }

    let parser = outer();
    assert_eq!(
        parser.run_inner(&["-v", "5"]).unwrap(),
        Outer {
            inner: Inner { value: 5 },
            flag: false,
        }
    );
    assert_eq!(
        parser.run_inner(&["-v", "9", "--flag"]).unwrap(),
        Outer {
            inner: Inner { value: 9 },
            flag: true,
        }
    );
}

#[test]
fn fallback_value() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    struct Fb {
        #[bpaf(long, fallback(7))]
        level: u32,
    }

    assert_eq!(fb().run_inner(&[]).unwrap(), Fb { level: 7 });
    assert_eq!(fb().run_inner(&["--level", "3"]).unwrap(), Fb { level: 3 });
}

#[test]
fn guard_rejects_out_of_range() {
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(options)]
    struct Guarded {
        #[bpaf(long, guard(|n| *n < 10, "must be below ten"))]
        n: u32,
    }

    assert_eq!(
        guarded().run_inner(&["--n", "3"]).unwrap(),
        Guarded { n: 3 }
    );
    assert!(guarded().run_inner(&["--n", "50"]).is_err());
}

#[test]
fn start_adjacent_block() {
    // `#[bpaf(start_adjacent)]` is the top-level, left-anchored sibling of `#[bpaf(adjacent)]`:
    // it wraps the whole `construct!` block in `.start_adjacent()`.
    #[derive(Debug, Clone, Bpaf, PartialEq)]
    #[bpaf(start_adjacent)]
    struct Group {
        #[bpaf(short('a'))]
        a: (),
        #[bpaf(short('b'))]
        b: bool,
        #[bpaf(short('c'))]
        c: bool,
    }

    let parser = group().to_options();
    assert_eq!(
        parser.run_inner(&["-a", "-b", "-c"]).unwrap(),
        Group {
            a: (),
            b: true,
            c: true,
        }
    );
    assert_eq!(
        parser.run_inner(&["-a"]).unwrap(),
        Group {
            a: (),
            b: false,
            c: false,
        }
    );
}
