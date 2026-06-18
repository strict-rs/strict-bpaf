use bpaf::*;

#[test]
fn test_adjacent() {
    let a = short('a').req_flag(());
    let b = short('b').switch();
    let c = short('c').switch();
    let parser = construct!(a, b, c).adjacent().many().to_options();

    let r = parser
        .run_inner(&["-a", "-c", "-a", "-b", "-a", "-b", "-c"])
        .unwrap();
    // adjacent groups here argument
    // -a [-b] -c  | -a -b [-c] | -a -b -c
    assert_eq!(r, &[((), false, true), ((), true, false), ((), true, true)]);

    let r = parser.run_inner(&["-a"]).unwrap();
    assert_eq!(r, &[((), false, false)]);

    let r = parser.run_inner(&["-a", "-c"]).unwrap();
    assert_eq!(r, &[((), false, true)]);

    let r = parser.run_inner(&[]).unwrap();
    assert_eq!(r, &[]);
}

#[test]
fn start_adjacent_basic() {
    // `start_adjacent` is the left-anchored variant of `adjacent`: the consumed block must begin
    // at the current position with nothing unparsed to its left.
    let a = short('a').req_flag(());
    let b = short('b').switch();
    let c = short('c').switch();
    let parser = construct!(a, b, c).start_adjacent().to_options();

    let r = parser.run_inner(&["-a", "-b", "-c"]).unwrap();
    assert_eq!(r, ((), true, true));

    let r = parser.run_inner(&["-a"]).unwrap();
    assert_eq!(r, ((), false, false));
}

#[test]
fn start_adjacent_left_anchors() {
    // `adjacent` finds its block anywhere on the command line; `start_adjacent` only consumes a
    // block that begins at the current position, with nothing unparsed to its left.
    let adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        let ab = construct!(a, b).adjacent().optional();
        let c = short('c').switch();
        construct!(ab, c).to_options()
    };
    let start_adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        let ab = construct!(a, b).start_adjacent().optional();
        let c = short('c').switch();
        construct!(ab, c).to_options()
    };

    // Block at the front of the args, or no block at all: `adjacent` and `start_adjacent` agree.
    assert_eq!(
        adjacent.run_inner(&["-a", "1"]).unwrap(),
        (Some(((), 1)), false)
    );
    assert_eq!(
        start_adjacent.run_inner(&["-a", "1"]).unwrap(),
        (Some(((), 1)), false)
    );
    assert_eq!(
        adjacent.run_inner(&["-a", "1", "-c"]).unwrap(),
        (Some(((), 1)), true)
    );
    assert_eq!(
        start_adjacent.run_inner(&["-a", "1", "-c"]).unwrap(),
        (Some(((), 1)), true)
    );
    assert_eq!(adjacent.run_inner(&["-c"]).unwrap(), (None, true));
    assert_eq!(start_adjacent.run_inner(&["-c"]).unwrap(), (None, true));

    // Leading gap: `-c` sits before the `-a 1` block. `adjacent` scans past it and consumes the
    // block; `start_adjacent` refuses it (not left-anchored), so `-a 1` are left with no consumer
    // and the whole parse fails.
    assert_eq!(
        adjacent.run_inner(&["-c", "-a", "1"]).unwrap(),
        (Some(((), 1)), true)
    );
    assert!(start_adjacent.run_inner(&["-c", "-a", "1"]).is_err());
}

#[test]
fn adjacent_finds_block_regardless_of_position() {
    let a = short('a').req_flag(());
    let b = positional::<usize>("X");
    let ab = construct!(a, b).adjacent();
    let c = short('c').switch();
    let parser = construct!(ab, c).to_options();

    // POSITIVE: a required `adjacent` block is consumed wherever it sits, even behind another token.
    assert_eq!(parser.run_inner(&["-a", "1"]).unwrap(), (((), 1), false));
    assert_eq!(
        parser.run_inner(&["-a", "1", "-c"]).unwrap(),
        (((), 1), true)
    );
    assert_eq!(
        parser.run_inner(&["-c", "-a", "1"]).unwrap(),
        (((), 1), true)
    );

    // NEGATIVE: with no matching `-a` anywhere the required block fails instead of yielding a value.
    assert!(parser.run_inner(&["-c"]).is_err());
    assert!(parser.run_inner(&[]).is_err());
}

#[test]
fn start_adjacent_requires_the_block_at_the_front() {
    let a = short('a').req_flag(());
    let b = positional::<usize>("X");
    let ab = construct!(a, b).start_adjacent();
    let c = short('c').switch();
    let parser = construct!(ab, c).to_options();

    // POSITIVE: a block that begins the still-unparsed input is consumed.
    assert_eq!(parser.run_inner(&["-a", "1"]).unwrap(), (((), 1), false));
    assert_eq!(
        parser.run_inner(&["-a", "1", "-c"]).unwrap(),
        (((), 1), true)
    );

    // NEGATIVE: unlike `adjacent`, it must NOT scan past a leading token — a block behind `-c` is
    // not left-anchored, so the parse fails (this is exactly the input `adjacent` accepts above).
    assert!(parser.run_inner(&["-c", "-a", "1"]).is_err());
    assert!(parser.run_inner(&["-c"]).is_err());
    assert!(parser.run_inner(&[]).is_err());
}

#[test]
fn start_adjacent_allows_parsed_items_to_its_left() {
    // "Nothing unparsed to its left" means *unparsed*: an item already consumed by an earlier
    // parser does not block a left-anchored group. Here `c` parses `-c` first, so `-a 1` becomes
    // the front of what's left, and `start_adjacent` consumes it just as `adjacent` would.
    let start_adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        let ab = construct!(a, b).start_adjacent();
        let c = short('c').switch();
        construct!(c, ab).to_options()
    };
    let adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        let ab = construct!(a, b).adjacent();
        let c = short('c').switch();
        construct!(c, ab).to_options()
    };
    // POSITIVE: a *parsed* `-c` to the left does not stop the left-anchored block.
    assert_eq!(
        start_adjacent.run_inner(&["-c", "-a", "1"]).unwrap(),
        (true, ((), 1))
    );
    assert_eq!(
        adjacent.run_inner(&["-c", "-a", "1"]).unwrap(),
        (true, ((), 1))
    );
}

#[test]
fn adjacent_block_requires_contiguous_items() {
    // NEGATIVE for both flavors: the items of an adjacent block must be next to each other. With
    // `-c` wedged between `-a` and its positional the block can't form, and because `-a` already
    // started it the missing positional is a hard error that `optional` does not swallow.
    let adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        let ab = construct!(a, b).adjacent().optional();
        let c = short('c').switch();
        construct!(ab, c).to_options()
    };
    let start_adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        let ab = construct!(a, b).start_adjacent().optional();
        let c = short('c').switch();
        construct!(ab, c).to_options()
    };
    assert!(adjacent.run_inner(&["-a", "-c", "1"]).is_err());
    assert!(start_adjacent.run_inner(&["-a", "-c", "1"]).is_err());
}

#[test]
fn adjacent_many_collects_repeated_blocks() {
    let adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        construct!(a, b).adjacent().many().to_options()
    };
    let start_adjacent = {
        let a = short('a').req_flag(());
        let b = positional::<usize>("X");
        construct!(a, b).start_adjacent().many().to_options()
    };
    // POSITIVE: a run of back-to-back blocks is collected, in order, by both flavors.
    assert_eq!(
        adjacent.run_inner(&["-a", "1", "-a", "2"]).unwrap(),
        &[((), 1), ((), 2)]
    );
    assert_eq!(
        start_adjacent.run_inner(&["-a", "1", "-a", "2"]).unwrap(),
        &[((), 1), ((), 2)]
    );
    // NEGATIVE: a stray leading token the repetition can't consume fails the whole parse.
    assert!(adjacent.run_inner(&["-c", "-a", "1", "-a", "2"]).is_err());
    assert!(
        start_adjacent
            .run_inner(&["-c", "-a", "1", "-a", "2"])
            .is_err()
    );
}

#[test]
fn test_adjacent_prefix() {
    let a = short('a').req_flag(());
    let b = positional::<usize>("X");
    let ab = construct!(a, b).adjacent().optional();
    let c = short('c').switch();
    let parser = construct!(ab, c).to_options();

    let r = parser.run_inner(&["-c"]).unwrap();
    assert_eq!(r, (None, true));

    let r = parser.run_inner(&["-a", "1"]).unwrap();
    assert_eq!(r, (Some(((), 1)), false));

    let r = parser.run_inner(&["-c", "-a", "1"]).unwrap();
    assert_eq!(r, (Some(((), 1)), true));

    let r = parser.run_inner(&["-a", "1", "-c"]).unwrap();
    assert_eq!(r, (Some(((), 1)), true));
}

#[test]
fn adjacent_error_message_pos_single() {
    let a = short('a').req_flag(());
    let b = positional::<usize>("B");
    let c = positional::<usize>("C");
    let d = short('d').switch();
    let adj = construct!(a, b, c).adjacent();
    let parser = construct!(adj, d).to_options();

    let r = parser.run_inner(&["-a", "10"]).unwrap_err().unwrap_stderr();
    assert_eq!(r, "expected `C`, pass `--help` for usage information");
}

#[test]
fn adjacent_error_message_arg_single() {
    let a = short('a').req_flag(());
    let b = short('b').argument::<usize>("B");
    let c = short('c').argument::<usize>("C");
    let d = short('d').switch();
    let adj = construct!(a, b, c).adjacent();
    let parser = construct!(adj, d).to_options();

    let r = parser.run_inner(&["-a", "10"]).unwrap_err().unwrap_stderr();
    assert_eq!(
        r,
        "expected `-b=B`, got `10`. Pass `--help` for usage information"
    );
}

#[test]
fn adjacent_error_message_pos_many() {
    let a = short('a').req_flag(());
    let b = positional::<usize>("B");
    let c = positional::<usize>("C");
    let d = short('d').switch();
    let adj = construct!(a, b, c).adjacent().many();
    let parser = construct!(adj, d).to_options();

    let r = parser.run_inner(&["-a", "10"]).unwrap_err().unwrap_stderr();
    assert_eq!(r, "expected `C`, pass `--help` for usage information");
}

#[test]
fn adjacent_error_message_arg_many() {
    let a = short('a').req_flag(());
    let b = short('b').argument::<usize>("B");
    let c = short('c').argument::<usize>("C");
    let d = short('d').switch();
    let adj = construct!(a, b, c).adjacent().many();
    let parser = construct!(adj, d).to_options();

    let r = parser.run_inner(&["-a", "10"]).unwrap_err().unwrap_stderr();
    // this should ask for -b or -c and complain on 10...
    assert_eq!(
        r,
        "expected `-b=B`, got `10`. Pass `--help` for usage information"
    );
}

#[test]
fn adjacent_is_adjacent() {
    let a = short('a').req_flag(());
    let b = positional::<usize>("B");
    let parser = construct!(a, b).adjacent().map(|t| t.1).many().to_options();

    let r = parser
        .run_inner(&["-a", "-a", "10", "20"])
        .unwrap_err()
        .unwrap_stderr();
    assert_eq!(r, "expected `B`, pass `--help` for usage information");

    let r = parser.run_inner(&["-a", "10", "-a", "20"]).unwrap();
    assert_eq!(r, [10, 20]);
}

#[test]
fn adjacent_with_switch() {
    let a = short('a').req_flag(());
    let b = positional::<usize>("B");
    let ab = construct!(a, b).adjacent().map(|t| t.1).many();
    let c = short('c').switch();
    let parser = construct!(ab, c).to_options();

    let r = parser.run_inner(&["-a", "10", "-c"]).unwrap();
    assert_eq!(r, (vec![10], true));

    let r = parser.run_inner(&["-a", "10", "-c", "-a", "20"]).unwrap();
    assert_eq!(r, (vec![10, 20], true));

    let r = parser.run_inner(&["-c", "-a", "10", "-a", "20"]).unwrap();
    assert_eq!(r, (vec![10, 20], true));
}

#[test]
fn adjacent_limits_commands() {
    let x = pure(()).to_options().command("a").adjacent();
    let s = short('s').switch();
    let parser = construct!(s, x).to_options();

    let r = parser.run_inner(&["a", "-s"]).unwrap();
    assert_eq!(r, (true, ()));
}

#[test]
fn commands_and_adjacent() {
    let eat = positional::<String>("FOOD")
        .to_options()
        .command("eat")
        .help("eat something")
        .adjacent();

    let sleep = long("time")
        .argument::<String>("HOURS")
        .to_options()
        .command("sleep")
        .help("sleep for a bit")
        .adjacent();

    let cmds = construct!([eat, sleep]);
    let switch = short('s').switch();

    let parser = construct!(switch, cmds).to_options();

    let r = parser.run_inner(&["sleep", "--time", "12", "-s"]).unwrap();
    assert_eq!(r, (true, "12".to_owned()));

    let r = parser.run_inner(&["--help"]).unwrap_err().unwrap_stdout();

    // TODO - this is ugly
    let expected = "\
Usage: [-s] COMMAND ...

Available options:
    -s
    -h, --help  Prints help information

Available commands:
    eat         eat something
    sleep       sleep for a bit
";

    assert_eq!(r, expected);
}

#[test]
fn two_adjacent_args() {
    let x = short('x').argument::<usize>("X");
    let y = short('y').argument::<usize>("Y");
    let c = short('c').switch();
    let point = construct!(x, y).adjacent();
    let parser = construct!(point, c).to_options();

    let r = parser.run_inner(&["-x", "3", "-y", "4", "-c"]).unwrap();
    assert_eq!(r, ((3, 4), true));

    // they are adjacent to each other, but the way it is coded currently - they must be adjacent
    // to the first element.
    // Proper fix is to split "adjacent" into "adjacent to" and "adjacent block"

    // let r = parser.run_inner(&["-y", "3", "-x", "4", "-c"]).unwrap();
    // assert_eq!(r, ((4, 3), true));

    let r = parser
        .run_inner(&["-y", "3", "-c", "-x", "4"])
        .unwrap_err()
        .unwrap_stderr();
    assert_eq!(r, "expected `-y=Y`, pass `--help` for usage information");
}
