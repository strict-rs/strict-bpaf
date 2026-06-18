//! Tools to define primitive parsers
//!
//! # Ways to consume data
//!
//! ## Flag
//!
//! - [`flag`](Cx::flag) - a string that consists of two dashes (`--flag`) and a name and a single
//! dash and a single character (`-f`) created with [`long`](Cx) and [`short`](Cx) respectively.
//! Depending if this name is present or absent on the command line primitive flag parser produces
//! one of two values. User can combine several short flags in a single invocation: `-a -b -c` is
//! the same as `-abc`.
//!
#![cfg_attr(not(doctest), doc = include_str!("docs2/flag.md"))]
//!
//! ## Required flag
//!
//! Similar to `flag`, but instead of falling back to the second value required flag parser would
//! fail. Mostly useful in combination with other parsers, created with [`req_flag`](Cx::req_flag).
//!
#![cfg_attr(not(doctest), doc = include_str!("docs2/req_flag.md"))]
//!
//! ## Switch
//!
//! A special case of a flag that gets decoded into a `bool`, mostly serves as a convenient shortcut
//! to `.flag(true, false)`. Created with [`switch`](Cx::switch).
//!
#![cfg_attr(not(doctest), doc = include_str!("docs2/switch.md"))]
//!
//! ## Argument
//!
//! A short or long `flag` followed by either a space or `=` and then by a string literal.
//! `-f foo`, `--flag bar` or `-o=-` are all valid argument examples. Note, string literal can't
//! start with `-` unless separated from the flag with `=`. For short flags value can follow
//! immediately: `-fbar`.
//!
#![cfg_attr(not(doctest), doc = include_str!("docs2/argument.md"))]
//!
//! ## Positional
//!
//! A positional argument with no additonal name, for example in `vim main.rs` `main.rs` is a
//! positional argument. Can't start with `-`, created with [`positional`].
//!
#![cfg_attr(not(doctest), doc = include_str!("docs2/positional.md"))]
//!
//! ## Any
//!
//! Also a positional argument with no additional name, but unlike [`positional`] itself, [`any`]
//! isn't restricted to positional looking structure and would consume any items as they appear on
//! a command line. Can be useful to collect anything unused to pass to other applications.
//!
#![cfg_attr(not(doctest), doc = include_str!("docs2/any_simple.md"))]
#![cfg_attr(not(doctest), doc = include_str!("docs2/any_literal.md"))]
//!
//! ## Command
//!
//! A command defines a starting point for an independent subparser. Name must be a valid utf8
//! string. For example `cargo build` invokes command `"build"` and after `"build"` `cargo` starts
//! accepting values it won't accept otherwise
//!
#![cfg_attr(not(doctest), doc = include_str!("docs2/command.md"))]
//!
use std::{ffi::OsString, marker::PhantomData, str::FromStr};

use crate::{
    Doc, Error, Item, Meta, OptionParser, Parser,
    args::{Arg, State},
    cx::Cx,
    error::{Message, MissingItem},
    from_os_str::parse_os_str,
    item::ShortLong,
    meta_help::Metavar,
};

#[cfg(doc)]
use crate::{any, command, env, long, positional, short};

/// A named thing used to create [`flag`](Cx::flag), [`switch`](Cx::switch) or [`argument`](Cx::argument)
///
/// # Combinatoric usage
///
/// Named items (`argument`, `flag` and `switch`) can have up to 2 visible names (one short and one
/// long) and multiple hidden short and long aliases if needed. It's also possible to consume items
/// from environment variables using [`env`](Cx). You usually start with [`short`] or [`long`] function,
/// then apply [`short`](Cx) / [`long`](Cx) / [`env`](Cx) / [`help`](Cx) repeatedly to build a desired set of names then
/// transform it into a parser using `flag`, `switch` or `positional`.
///
#[cfg_attr(not(doctest), doc = include_str!("docs2/named_arg_combine.md"))]
///
/// # Derive usage
///
/// When using derive API it is possible to omit some or all the details:
/// 1. If no naming information is present at all - `bpaf` would use field name as a long name
///    (or a short name if field name consists of a single character)
/// 2. If `short` or `long` annotation is present without an argument - `bpaf` would use first character
///    or a full name as long and short name respectively. It won't try to add implicit long or
///    short name from the previous item.
/// 3. If `short` or `long` annotation is present with an argument - those are values `bpaf` would
///    use instead of the original field name
/// 4. You can specify many `short` and `long` names, any past the first one of each type will
///    become hidden aliases
/// 5. If `env(arg)` annotation is present - in addition to long/short names derived according to
///    rules 1..3 `bpaf` would also parse environment variable `arg` which can be a string literal
///    or an expression.
#[cfg_attr(not(doctest), doc = include_str!("docs2/named_arg_derive.md"))]
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct Named {
    /// The identifier this parser was created from. Because it is always present, a `Named` names
    /// at least one thing — the "no short, no long, no env" state simply cannot be represented.
    first: Name,
    rest: Vec<Name>,
    pub(crate) help: Option<Doc>,
}

/// One way to refer to a named parser: a short flag, a long flag, or an environment variable.
#[derive(Copy, Clone, Debug)]
enum Name {
    Short(char),
    Long(&'static str),
    Env(&'static str),
}

/// How a `Named` is identified for diagnostics: by a visible flag name, or — when it has none —
/// by an environment variable. Total by construction; see [`Named::identity`].
enum Identity {
    Flag(ShortLong),
    Env(&'static str),
}

impl Named {
    pub(crate) fn from_short(short: char) -> Self {
        Named {
            first: Name::Short(short),
            rest: Vec::new(),
            help: None,
        }
    }

    pub(crate) fn from_long(long: &'static str) -> Self {
        Named {
            first: Name::Long(long),
            rest: Vec::new(),
            help: None,
        }
    }

    pub(crate) fn from_env(env: &'static str) -> Self {
        Named {
            first: Name::Env(env),
            rest: Vec::new(),
            help: None,
        }
    }

    /// All identifiers, primary first.
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        std::iter::once(self.first).chain(self.rest.iter().copied())
    }

    pub(crate) fn shorts(&self) -> impl Iterator<Item = char> + '_ {
        self.names().filter_map(|n| match n {
            Name::Short(s) => Some(s),
            _ => None,
        })
    }

    pub(crate) fn longs(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.names().filter_map(|n| match n {
            Name::Long(l) => Some(l),
            _ => None,
        })
    }

    pub(crate) fn envs(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.names().filter_map(|n| match n {
            Name::Env(e) => Some(e),
            _ => None,
        })
    }

    /// Classify this parser for "missing"/"no env" diagnostics. Total by construction: `first`
    /// always exists, so the "no flag name and no env" case behind the old `unreachable!()` cannot
    /// occur — when there is no short/long, `first` is necessarily the env.
    fn identity(&self) -> Identity {
        match self.first {
            Name::Short(s) => Identity::Flag(ShortLong::short_maybe_long(s, self.longs().next())),
            Name::Long(l) => Identity::Flag(ShortLong::long_maybe_short(self.shorts().next(), l)),
            Name::Env(e) => match ShortLong::try_from(self) {
                Ok(name) => Identity::Flag(name),
                Err(()) => Identity::Env(e),
            },
        }
    }

    fn flag_item_with(&self, name: ShortLong) -> Item {
        Item::Flag {
            name,
            help: self.help.clone(),
            env: self.envs().next(),
            shorts: self.shorts().collect(),
        }
    }

    pub(crate) fn flag_item(&self) -> Option<Item> {
        Some(self.flag_item_with(ShortLong::try_from(self).ok()?))
    }
}

impl Cx<Named> {
    /// Add a short name to a flag/switch/argument
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/short_long_env.md"))]
    #[must_use]
    pub fn short(mut self, short: char) -> Self {
        self.0.rest.push(Name::Short(short));
        self
    }

    /// Add a long name to a flag/switch/argument
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/short_long_env.md"))]
    #[must_use]
    pub fn long(mut self, long: &'static str) -> Self {
        self.0.rest.push(Name::Long(long));
        self
    }

    /// Environment variable fallback
    ///
    /// If named value isn't present - try to fallback to this environment variable.
    ///
    /// You can specify it multiple times, `bpaf` would use items past the first one as hidden aliases.
    ///
    /// For [`flag`](Cx::flag) and [`switch`](Cx::switch) environment variable being present gives the same result as the flag
    /// being present, allowing to implement things like `NO_COLOR` variables:
    ///
    /// ```console
    /// $ NO_COLOR=1 app --do-something
    /// ```
    #[cfg_attr(not(doctest), doc = include_str!("docs2/short_long_env.md"))]
    #[must_use]
    pub fn env(mut self, variable: &'static str) -> Self {
        self.0.rest.push(Name::Env(variable));
        self
    }

    /// Add a help message to a `flag`/`switch`/`argument`
    ///
    /// `bpaf` converts doc comments and string into help by following those rules:
    /// 1. Everything up to the first blank line is included into a "short" help message
    /// 2. Everything is included into a "long" help message
    /// 3. `bpaf` preserves linebreaks followed by a line that starts with a space
    /// 4. Linebreaks are removed otherwise
    ///
    /// You can pass anything that can be converted into [`Doc`], if you are not using documentation
    /// generation functionality ([`doc`](crate::doc)) this can be `&str`.
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/switch_help.md"))]
    #[must_use]
    pub fn help<M>(mut self, help: M) -> Self
    where
        M: Into<Doc>,
    {
        self.0.help = Some(help.into());
        self
    }

    /// Simple boolean flag
    ///
    /// A special case of a [`flag`](Cx::flag) that gets decoded into a `bool`, mostly serves as a convenient
    /// shortcut to `.flag(true, false)`.
    ///
    /// In Derive API bpaf would use `switch` for `bool` fields inside named structs that don't have
    /// other consumer annotations ([`flag`](Cx::flag), [`argument`](Cx::argument), etc).
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/switch.md"))]
    #[must_use]
    pub fn switch(self) -> Cx<Flag<bool>> {
        Cx(build_flag_parser(true, Some(false), self.0))
    }

    /// Flag with custom present/absent values
    ///
    /// More generic version of [`switch`](Cx::switch) that can use arbitrary type instead of [`bool`].
    #[cfg_attr(not(doctest), doc = include_str!("docs2/flag.md"))]
    #[must_use]
    pub fn flag<T>(self, present: T, absent: T) -> Cx<Flag<T>>
    where
        T: Clone + 'static,
    {
        Cx(build_flag_parser(present, Some(absent), self.0))
    }

    /// Required flag with custom value
    ///
    /// Similar to [`flag`](Cx::flag) takes no option arguments, but would only succeed if user specifies its
    /// name on a command line. Works best in combination with other parsers.
    ///
    /// In derive style API `bpaf` would transform field-less enum variants into a parser that
    /// accepts one of it's variant names as `req_flag`. Additionally `bpaf` handles `()` fields as
    /// `req_flag`.
    #[cfg_attr(not(doctest), doc = include_str!("docs2/req_flag.md"))]
    #[must_use]
    pub fn req_flag<T>(self, present: T) -> Cx<Flag<T>>
    where
        T: Clone + 'static,
    {
        Cx(build_flag_parser(present, None, self.0))
    }

    /// Argument
    ///
    /// A short (`-a`) or long (`--name`) name followed by  either a space or `=` and then by a
    /// string literal.  `-f foo`, `--flag bar` or `-o=-` are all valid argument examples. Note,
    /// string literal can't start with `-` unless separated from the flag with `=`. For short flags
    /// value can follow immediately: `-fbar`.
    ///
    /// When using combinatoring API you can specify the type with turbofish, for parsing types
    /// that don't implement [`FromStr`] you can use consume a `String`/`OsString` first and parse
    /// it by hands.
    ///
    /// For `metavar` value you should pick something short and descriptive about the parameter,
    /// usually in capital letters. For example for an abstract file parameter it could be `"FILE"`,
    /// for a username - `"USER"`, etc.
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/argument.md"))]
    ///
    /// You can further restrict it using [`adjacent`](Cx)
    #[must_use]
    pub fn argument<T>(self, metavar: &'static str) -> Cx<Argument<T>>
    where
        T: FromStr + 'static,
    {
        Cx(build_argument(self.0, metavar))
    }
}

impl Named {
    /// `adjacent` requires for the argument to be present in the same word as the flag: `-f bar` -
    /// no, `-fbar` or `-f=bar` - yes.
    pub(crate) fn matches_arg(&self, arg: &Arg, adjacent: bool) -> bool {
        match arg {
            Arg::Short(s, is_adj, _) => self.shorts().any(|c| c == *s) && (!adjacent || *is_adj),
            Arg::Long(l, is_adj, _) => {
                self.longs().any(|x| x == l.as_str()) && (!adjacent || *is_adj)
            }
            Arg::ArgWord(_) | Arg::Word(_) | Arg::PosWord(_) => false,
        }
    }
}

impl<T> OptionParser<T> {
    /// Parse a subcommand
    ///
    /// Subcommands allow to use a totally independent parser inside a current one. Inner parser
    /// can have its own help message, description, version and so on. You can nest them arbitrarily
    /// too.
    ///
    /// # Important restriction
    /// When parsing command arguments from command lines you should have parsers for all your named
    /// values before parsers for commands and positional items. In derive API fields parsed as
    /// positional should be at the end of your `struct`/`enum`. Same rule applies to parsers with
    /// positional fields or commands inside: such parsers should go to the end as well.
    ///
    /// Use [`check_invariants`](OptionParser::check_invariants) in your test to ensure correctness.
    ///
    /// For example for non positional `non_pos` and a command `command` parsers
    /// ```rust
    /// # use bpaf::*;
    /// # let non_pos = || short('n').switch();
    /// # let command = || pure(()).to_options().command("POS");
    /// let valid = construct!(non_pos(), command());
    /// let invalid = construct!(command(), non_pos());
    /// ```
    ///
    /// **`bpaf` panics during help generation unless if this restriction holds**
    ///
    /// You can attach a single visible short alias and multiple hidden short and long aliases using
    /// [`short`](Cx) and [`long`](Cx) methods.
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/command.md"))]
    ///
    /// To represent multiple possible commands it is convenient to use enums
    #[cfg_attr(not(doctest), doc = include_str!("docs2/command_enum.md"))]
    #[must_use]
    pub fn command(self, name: &'static str) -> Cx<Command<T>>
    where
        T: 'static,
    {
        Cx(Command {
            longs: vec![name],
            shorts: Vec::new(),
            help: self.short_descr(),
            subparser: self,
            adjacent: false,
        })
    }
}

/// Builder structure for the [`command`]
///
/// Created with [`command`], implements parser for the inner structure, gives access to [`help`](Cx).
#[doc(hidden)]
pub struct Command<T> {
    pub(crate) longs: Vec<&'static str>,
    pub(crate) shorts: Vec<char>,
    // short help!
    pub(crate) help: Option<Doc>,
    pub(crate) subparser: OptionParser<T>,
    pub(crate) adjacent: bool,
}

impl<P> Cx<Command<P>> {
    /// Add a brief description to a command
    ///
    /// `bpaf` uses this description along with the command name in help output so it shouldn't
    /// exceed one or two lines. If `help` isn't specified `bpaf` falls back to [`descr`](OptionParser::descr) from the
    /// inner parser.
    ///
    /// # Combinatoric usage
    ///
    /// ```rust
    /// # use bpaf::*;
    /// fn inner() -> OptionParser<bool> {
    ///     short('i')
    ///         .help("Mysterious inner switch")
    ///         .switch()
    ///         .to_options()
    ///         .descr("performs an operation")
    /// }
    ///
    /// fn mysterious_parser() -> impl Parser<bool> {
    ///     inner().command("mystery")
    ///         .help("This command performs a mystery operation")
    /// }
    /// ```
    ///
    /// # Derive usage
    /// `bpaf_derive` uses doc comments for inner parser, no specific options are available. See
    /// [`descr`](OptionParser::descr) for more details
    /// ```rust
    /// # use bpaf::*;
    /// /// This command performs a mystery operation
    /// #[derive(Debug, Clone, Bpaf)]
    /// #[bpaf(command)]
    /// struct Mystery {
    ///     #[bpaf(short)]
    ///     /// Mysterious inner switch
    ///     inner: bool,
    /// }
    /// ```
    ///
    /// # Example
    /// ```console
    /// $ app --help
    ///     <skip>
    /// Available commands:
    ///     mystery  This command performs a mystery operation
    /// ```
    #[must_use]
    pub fn help<M>(mut self, help: M) -> Self
    where
        M: Into<Doc>,
    {
        self.0.help = Some(help.into());
        self
    }

    /// Add a custom short alias for a command
    ///
    /// Behavior is similar to [`short`](Cx), only first short name is visible.
    #[must_use]
    pub fn short(mut self, short: char) -> Self {
        self.0.shorts.push(short);
        self
    }

    /// Add a custom hidden long alias for a command
    ///
    /// Behavior is similar to [`long`](Cx), but since you had to specify the first long name when
    /// making the command - this one becomes a hidden alias.
    #[must_use]
    pub fn long(mut self, long: &'static str) -> Self {
        self.0.longs.push(long);
        self
    }

    /// Allow for the command to succeed even if there are non consumed items present
    ///
    /// Normally a subcommand parser should handle the rest of the unconsumed elements thus allowing
    /// only "vertical" chaining of commands. `adjacent` modifier lets command parser to succeed if
    /// there are leftovers for as long as all comsumed items form a single adjacent block. This
    /// opens possibilities to chain commands sequentially.
    ///
    /// Let's consider two examples with consumed items marked in bold :
    ///
    /// - <code>**cmd** **-a** -b **-c** -d</code>
    /// - <code>**cmd** **-a** **-c** -b -d</code>
    ///
    /// In the first example `-b` breaks the adjacency for all the consumed items so parsing will fail,
    /// while here in the second one the name and all the consumed items are adjacent to each other so
    /// parsing will succeed.
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/adjacent_command.md"))]
    #[must_use]
    pub fn adjacent(mut self) -> Self {
        self.0.adjacent = true;
        self
    }
}

impl<T> Parser<T> for Command<T> {
    fn eval(&self, args: &mut State) -> Result<T, Error> {
        // used to avoid allocations for short names
        let mut tmp = String::new();
        if self.longs.iter().any(|long| args.take_cmd(long))
            || self.shorts.iter().any(|s| {
                tmp.clear();
                tmp.push(*s);
                args.take_cmd(&tmp)
            })
        {
            #[cfg(feature = "autocomplete")]
            if args.touching_last_remove() {
                // in completion mode prefer to autocomplete the command name vs going inside the
                // parser
                args.clear_comps();
                args.push_command(self.longs[0], self.shorts.first().copied(), &self.help);
                return Err(Error(Message::Missing(Vec::new())));
            }

            if let Some(cur) = args.current {
                args.set_scope(cur..args.scope().end);
            }

            args.path.push(self.longs[0].to_string());
            if self.adjacent {
                let mut orig_args = args.clone();

                // narrow down the scope to adjacently available elements
                args.set_scope(args.adjacently_available_from(args.scope().start + 1));

                match self
                    .subparser
                    .run_subparser(args)
                    .map_err(Message::ParseFailure)
                {
                    Ok(ok) => {
                        args.set_scope(orig_args.scope());
                        Ok(ok)
                    }
                    Err(err) => {
                        let orig_scope = args.scope();
                        if let Some(narrow_scope) = args.adjacent_scope(&orig_args) {
                            orig_args.set_scope(narrow_scope);
                            if let Ok(res) = self.subparser.run_subparser(&mut orig_args) {
                                orig_args.set_scope(orig_scope);
                                std::mem::swap(&mut orig_args, args);
                                return Ok(res);
                            }
                        }
                        Err(Error(err))
                    }
                }
            } else {
                self.subparser
                    .run_subparser(args)
                    .map_err(|e| Error(Message::ParseFailure(e)))
            }
        } else {
            #[cfg(feature = "autocomplete")]
            args.push_command(self.longs[0], self.shorts.first().copied(), &self.help);

            let missing = MissingItem {
                item: self.item(),
                position: args.scope().start,
                scope: args.scope(),
            };
            Err(Error(Message::Missing(vec![missing])))
        }
    }

    fn meta(&self) -> Meta {
        Meta::from(self.item())
    }
}

impl<T> Command<T> {
    fn item(&self) -> Item {
        Item::Command {
            name: self.longs[0],
            short: self.shorts.first().copied(),
            help: self.help.clone(),
            meta: Box::new(self.subparser.inner.meta()),
            info: Box::new(self.subparser.info.clone()),
        }
    }
}

fn build_flag_parser<T>(present: T, absent: Option<T>, named: Named) -> Flag<T>
where
    T: Clone + 'static,
{
    Flag {
        present,
        absent,
        named,
    }
}

#[derive(Clone)]
#[doc(hidden)]
/// Parser for a named switch, created with [`flag`](Cx::flag) or [`switch`](Cx::switch)
pub struct Flag<T> {
    present: T,
    absent: Option<T>,
    named: Named,
}

impl<T: Clone + 'static> Parser<T> for Flag<T> {
    fn eval(&self, args: &mut State) -> Result<T, Error> {
        if args.take_flag(&self.named) || self.named.envs().find_map(std::env::var_os).is_some() {
            #[cfg(feature = "autocomplete")]
            if args.touching_last_remove() {
                args.push_flag(&self.named);
            }
            Ok(self.present.clone())
        } else {
            #[cfg(feature = "autocomplete")]
            args.push_flag(&self.named);
            match &self.absent {
                Some(ok) => Ok(ok.clone()),
                None => match self.named.identity() {
                    Identity::Flag(name) => {
                        let missing = MissingItem {
                            item: self.named.flag_item_with(name),
                            position: args.scope().start,
                            scope: args.scope(),
                        };
                        Err(Error(Message::Missing(vec![missing])))
                    }
                    Identity::Env(name) => Err(Error(Message::NoEnv(name))),
                },
            }
        }
    }

    fn meta(&self) -> Meta {
        if let Some(item) = self.named.flag_item() {
            item.required(self.absent.is_none())
        } else {
            Meta::Skip
        }
    }
}

impl<T> Cx<Flag<T>> {
    /// Add a help message to `flag`
    ///
    /// See [`help`](Cx)
    #[must_use]
    pub fn help<M>(mut self, help: M) -> Self
    where
        M: Into<Doc>,
    {
        self.0.named.help = Some(help.into());
        self
    }
}

impl<T> Cx<Argument<T>> {
    /// Add a help message to an `argument`
    ///
    /// See [`help`](Cx)
    #[must_use]
    pub fn help<M>(mut self, help: M) -> Self
    where
        M: Into<Doc>,
    {
        self.0.named.help = Some(help.into());
        self
    }
}

fn build_argument<T>(named: Named, metavar: &'static str) -> Argument<T> {
    Argument {
        named,
        metavar,
        ty: PhantomData,
        adjacent: false,
    }
}

/// Parser for a named argument, created with [`argument`](Cx::argument).
#[derive(Clone)]
#[doc(hidden)]
pub struct Argument<T> {
    ty: PhantomData<T>,
    named: Named,
    metavar: &'static str,
    adjacent: bool,
}

impl<T> Cx<Argument<T>> {
    /// Restrict parsed arguments to have both flag and a value in the same word:
    ///
    /// In other words an adjacent-restricted argument would accept `--flag=value` or `-fbar` but
    /// not `--flag value`. Note, this is different from [`adjacent`](Cx), just plays a similar role.
    ///
    /// Should allow to parse some of the more unusual things
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/adjacent_argument.md"))]
    #[must_use]
    pub fn adjacent(mut self) -> Self {
        self.0.adjacent = true;
        self
    }
}

impl<T> Argument<T> {
    fn item(&self) -> Option<Item> {
        Some(self.item_with(ShortLong::try_from(&self.named).ok()?))
    }

    fn item_with(&self, name: ShortLong) -> Item {
        Item::Argument {
            name,
            metavar: Metavar(self.metavar),
            env: self.named.envs().next(),
            help: self.named.help.clone(),
            shorts: self.named.shorts().collect(),
        }
    }

    fn take_argument(&self, args: &mut State) -> Result<OsString, Error> {
        match args.take_arg(&self.named, self.adjacent, Metavar(self.metavar)) {
            Ok(Some(w)) => {
                #[cfg(feature = "autocomplete")]
                if args.touching_last_remove() {
                    args.push_metavar(self.metavar, &self.named.help, true);
                }
                Ok(w)
            }
            Err(err) => {
                #[cfg(feature = "autocomplete")]
                args.push_argument(&self.named, self.metavar);
                Err(err)
            }
            _ => {
                #[cfg(feature = "autocomplete")]
                args.push_argument(&self.named, self.metavar);
                if let Some(val) = self.named.envs().find_map(std::env::var_os) {
                    args.current = None;
                    return Ok(val);
                }

                match self.named.identity() {
                    Identity::Flag(name) => {
                        let missing = MissingItem {
                            item: self.item_with(name),
                            position: args.scope().start,
                            scope: args.scope(),
                        };
                        Err(Error(Message::Missing(vec![missing])))
                    }
                    Identity::Env(name) => Err(Error(Message::NoEnv(name))),
                }
            }
        }
    }
}

impl<T> Parser<T> for Argument<T>
where
    T: FromStr + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    fn eval(&self, args: &mut State) -> Result<T, Error> {
        let os = self.take_argument(args)?;
        match parse_os_str::<T>(os) {
            Ok(ok) => Ok(ok),
            Err(err) => Err(Error(Message::ParseFailed(args.current, err))),
        }
    }

    fn meta(&self) -> Meta {
        if let Some(item) = self.item() {
            Meta::from(item)
        } else {
            Meta::Skip
        }
    }
}

pub(crate) fn build_positional<T>(metavar: &'static str) -> Positional<T> {
    Positional {
        metavar,
        help: None,
        position: Position::Unrestricted,
        ty: PhantomData,
    }
}

/// Parse a positional item, created with [`positional`](crate::positional)
///
/// You can add extra information to positional parsers with [`help`](Self::help), [`strict`](Self::strict), or [`non_strict`](Self::non_strict) on
/// this struct.
#[derive(Clone)]
#[doc(hidden)]
pub struct Positional<T> {
    metavar: &'static str,
    help: Option<Doc>,
    position: Position,
    ty: PhantomData<T>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Position {
    Unrestricted,
    Strict,
    NonStrict,
}

impl<T> Cx<Positional<T>> {
    /// Add a help message to a [`positional`] parser
    ///
    /// `bpaf` converts doc comments and string into help by following those rules:
    /// 1. Everything up to the first blank line is included into a "short" help message
    /// 2. Everything is included into a "long" help message
    /// 3. `bpaf` preserves linebreaks followed by a line that starts with a space
    /// 4. Linebreaks are removed otherwise
    ///
    /// You can pass anything that can be converted into [`Doc`], if you are not using documentation
    /// generation functionality ([`doc`](crate::doc)) this can be `&str`.
    ///
    #[cfg_attr(not(doctest), doc = include_str!("docs2/positional.md"))]
    #[must_use]
    pub fn help<M>(mut self, help: M) -> Self
    where
        M: Into<Doc>,
    {
        self.0.help = Some(help.into());
        self
    }

    /// Changes positional parser to be a "strict" positional
    ///
    /// Usually positional items can appear anywhere on a command line:
    /// ```console
    /// $ ls -d bpaf
    /// $ ls bpaf -d
    /// ```
    /// here `ls` takes a positional item `bpaf` and a flag `-d`
    ///
    /// But in some cases it might be useful to have a stricter separation between positonal items
    /// and flags, such as passing arguments to a subprocess:
    /// ```console
    /// $ cargo run --example basic -- --help
    /// ```
    ///
    /// here `cargo` takes a `--help` as a positional item and passes it to the example
    ///
    /// `bpaf` allows to require user to pass `--` for positional items with `strict` annotation.
    /// `bpaf` would display such positional elements differently in usage line as well.
    #[cfg_attr(not(doctest), doc = include_str!("docs2/positional_strict.md"))]
    #[must_use]
    #[inline(always)]
    pub fn strict(mut self) -> Self {
        self.0.position = Position::Strict;
        self
    }

    /// Changes positional parser to be a "not strict" positional
    ///
    /// Ensures the parser always rejects "strict" positions to the right of the separator, `--`.
    /// Essentially the inverse operation to [`strict`](Cx::strict), which can be used to ensure adjacent strict
    /// and nonstrict args never conflict with eachother.
    #[must_use]
    #[inline(always)]
    pub fn non_strict(mut self) -> Self {
        self.0.position = Position::NonStrict;
        self
    }
}

impl<T> Positional<T> {
    #[inline(always)]
    fn meta(&self) -> Meta {
        let meta = Meta::from(Item::Positional {
            metavar: Metavar(self.metavar),
            help: self.help.clone(),
        });
        match self.position {
            Position::Strict => Meta::Strict(Box::new(meta)),
            _ => meta,
        }
    }
}
#[allow(unused_variables)] // used when autocomplete is enabled
fn parse_pos_word(
    args: &mut State,
    metavar: Metavar,
    help: &Option<Doc>,
    position: Position,
) -> Result<OsString, Error> {
    match args.take_positional_word(metavar) {
        Ok((ix, is_strict, word)) => {
            match position {
                Position::Strict => {
                    if !is_strict {
                        #[cfg(feature = "autocomplete")]
                        args.push_pos_sep();
                        return Err(Error(Message::StrictPos(ix, metavar)));
                    }
                }
                Position::NonStrict => {
                    if is_strict {
                        return Err(Error(Message::NonStrictPos(ix, metavar)));
                    }
                }
                Position::Unrestricted => {}
            }

            #[cfg(feature = "autocomplete")]
            if args.touching_last_remove() && !args.check_no_pos_ahead() {
                args.push_metavar(metavar.0, help, false);
                args.set_no_pos_ahead();
            }
            Ok(word)
        }
        Err(err) => {
            #[cfg(feature = "autocomplete")]
            if !args.check_no_pos_ahead() {
                args.push_metavar(metavar.0, help, false);
                args.set_no_pos_ahead();
            }
            Err(err)
        }
    }
}

impl<T> Parser<T> for Positional<T>
where
    T: FromStr + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    fn eval(&self, args: &mut State) -> Result<T, Error> {
        let os = parse_pos_word(args, Metavar(self.metavar), &self.help, self.position)?;
        match parse_os_str::<T>(os) {
            Ok(ok) => Ok(ok),
            Err(err) => Err(Error(Message::ParseFailed(args.current, err))),
        }
    }

    #[inline(always)]
    fn meta(&self) -> Meta {
        self.meta()
    }
}

/// Consume an arbitrary value that satisfies a condition, created with [`any`], implements
/// [`anywhere`](Cx::anywhere).
#[doc(hidden)]
pub struct Anything<T> {
    pub(crate) metavar: Doc,
    pub(crate) help: Option<Doc>,
    pub(crate) check: Box<dyn Fn(OsString) -> Option<T>>,
    pub(crate) anywhere: bool,
}

impl<T> Anything<T> {
    pub(crate) fn item(&self) -> Item {
        Item::Any {
            metavar: self.metavar.clone(),
            help: self.help.clone(),
            anywhere: self.anywhere,
        }
    }
}

impl<T> Cx<Anything<T>> {
    /// Add a help message to [`any`] parser. See examples in [`any`]
    #[must_use]
    pub fn help<M: Into<Doc>>(mut self, help: M) -> Self {
        self.0.help = Some(help.into());
        self
    }

    /// Replace metavar with a custom value See examples in [`any`]
    #[must_use]
    pub fn metavar<M: Into<Doc>>(mut self, metavar: M) -> Self {
        self.0.metavar = metavar.into();
        self
    }

    /// Try to apply the parser to each unconsumed element instead of just the front one
    ///
    /// By default `any` tries to parse just the front unconsumed item behaving similar to
    /// [`positional`] parser, `anywhere` changes it so it applies to every unconsumed item, similar
    /// to argument parser.
    ///
    /// See examples in [`any`]
    #[must_use]
    pub fn anywhere(mut self) -> Self {
        self.0.anywhere = true;
        self
    }
}

impl<T> Parser<T> for Anything<T> {
    fn eval(&self, args: &mut State) -> Result<T, Error> {
        for (ix, x) in args.items_iter() {
            let (os, next) = match x {
                Arg::Short(_, next, os) | Arg::Long(_, next, os) => (os, *next),
                Arg::ArgWord(os) | Arg::Word(os) | Arg::PosWord(os) => (os, false),
            };
            if let Some(i) = (self.check)(os.clone()) {
                args.remove(ix);
                if next {
                    args.remove(ix + 1);
                }

                return Ok(i);
            }
            if !self.anywhere {
                break;
            }
        }
        let missing_item = MissingItem {
            item: self.item(),
            position: args.scope().start,
            scope: args.scope(),
        };
        Err(Error(Message::Missing(vec![missing_item])))
    }

    fn meta(&self) -> Meta {
        Meta::Item(Box::new(self.item()))
    }
}
