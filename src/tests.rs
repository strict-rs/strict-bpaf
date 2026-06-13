use crate::args::Arg;

#[test]
#[cfg(any(windows, unix))]
fn wtf_shenanigans_1() {
    use crate::args::{split_os_argument, Arg, ArgType};
    use std::ffi::OsString;

    for (i_c, prefix) in [
        (ArgType::Short, "f"),
        (ArgType::Long, "foo"),
        (ArgType::Long, "口水鸡"),
    ] {
        let i_prefix = OsString::from(prefix);
        let i_suffix;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            i_suffix = OsString::from_wide(&[0x0066, 0x006f, 0xD800, 0x006f]);
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::ffi::OsStringExt;
            i_suffix = OsString::from_vec(vec![0x66, 0x6f, 0xD8, 0x6f]);
        }

        let mut os_string = OsString::new();
        match i_c {
            ArgType::Short => os_string.push("-"),
            ArgType::Long => os_string.push("--"),
        }
        os_string.push(&i_prefix);
        os_string.push("=");
        os_string.push(&i_suffix);

        let (o_c, o_prefix, o_suffix) = split_os_argument(&os_string).unwrap();
        assert_eq!(i_c, o_c);
        assert_eq!(i_prefix.to_str().unwrap(), o_prefix);
        assert_eq!(Arg::ArgWord(i_suffix), o_suffix.unwrap());
    }
}

#[test]
fn wtf_shenanigans_2() {
    use crate::args::{split_os_argument, split_os_argument_fallback, ArgType};
    use std::ffi::OsString;

    for (i_c, prefix) in [
        (ArgType::Short, "f"),
        (ArgType::Long, "foo"),
        (ArgType::Long, "口水鸡"),
    ] {
        let i_prefix = OsString::from(prefix);
        let i_suffix = OsString::from("口水鸡");

        let mut os_string = OsString::new();
        match i_c {
            ArgType::Short => os_string.push("-"),
            ArgType::Long => os_string.push("--"),
        }
        os_string.push(&i_prefix);
        os_string.push("=");
        os_string.push(&i_suffix);

        let (o_c, o_prefix, o_suffix) = split_os_argument(&os_string).unwrap();
        assert_eq!(i_c, o_c);
        assert_eq!(i_prefix.to_str().unwrap(), o_prefix);
        assert_eq!(Arg::ArgWord(i_suffix.clone()), o_suffix.unwrap());

        let (o_c, o_prefix, o_suffix) = split_os_argument_fallback(&os_string).unwrap();
        assert_eq!(i_c, o_c);
        assert_eq!(i_prefix.to_str().unwrap(), o_prefix);
        assert_eq!(Arg::ArgWord(i_suffix.clone()), o_suffix.unwrap());
    }
}

#[test]
fn fallback_with_strange_args_produces_same_results() {
    use crate::args::{split_os_argument, split_os_argument_fallback};
    let s = std::ffi::OsString::from("-Obits=2048");
    let r1 = split_os_argument(&s);
    let r2 = split_os_argument_fallback(&s);
    assert_eq!(r1, r2);
}

#[test]
fn de_yoda() {
    use bpaf::*;
    let parser = construct!(a(short('a').switch()), b(short('b').switch())).to_options();

    let r = parser.run_inner(&[]).unwrap();
    assert_eq!(r, (false, false));

    let r = parser.run_inner(&["-a", "-b"]).unwrap();
    assert_eq!(r, (true, true));
}

#[test]
fn choice_with_uses_custom_empty_message() {
    use bpaf::*;
    let parsers: Vec<Box<dyn Parser<u32>>> = Vec::new();
    let err = choice_with(parsers, "nothing registered")
        .to_options()
        .run_inner(&[])
        .unwrap_err()
        .unwrap_stderr();
    assert!(err.contains("nothing registered"), "got: {err}");

    // `choice` keeps its original fixed message.
    let empty: Vec<Box<dyn Parser<u32>>> = Vec::new();
    let err = choice(empty)
        .to_options()
        .run_inner(&[])
        .unwrap_err()
        .unwrap_stderr();
    assert!(err.contains("Invalid choice usage"), "got: {err}");
}

#[test]
fn choice_with_folds_non_empty_parsers() {
    use bpaf::*;
    let a = short('a').req_flag(1u32).boxed();
    let b = short('b').req_flag(2u32).boxed();
    let parser = choice_with(vec![a, b], "unused").to_options();
    assert_eq!(parser.run_inner(&["-a"]).unwrap(), 1);
    assert_eq!(parser.run_inner(&["-b"]).unwrap(), 2);
}
