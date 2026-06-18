//! Validated intermediate representation — the boundary between parsing and code generation.
//!
//! Everything here is fully *resolved*: names are filled in, implicit `optional`/`many` are
//! materialised, a command always has a name, an `external` always has an identifier, and there
//! are no spans or `Result`s. As a consequence [`codegen`](crate::codegen) over these types is
//! **infallible** — all the fallible work (validation, defaulting, inference) happens once in
//! [`lower`](crate::lower).

use proc_macro2::{Ident, Span};
use syn::{Expr, LitChar, LitStr, Path, Type, Visibility};

use crate::help::Help;

/// A fully-resolved top-level parser definition.
#[derive(Debug)]
pub(crate) struct Top {
    pub ty: Ident,
    pub vis: Visibility,
    pub generate: Ident,
    pub body: Body,
    pub mode: Mode,
    pub boxed: bool,
    pub adjacent: bool,
    /// left-anchored adjacency on the whole block; emitted as `.start_adjacent()` (Parser mode).
    pub start_adjacent: bool,
    pub attrs: Vec<PostDecor>,
    pub bpaf_path: Option<Path>,
}

/// A single struct branch, or the alternatives of an enum.
#[derive(Debug)]
pub(crate) enum Body {
    Single(Branch),
    Alternatives(Vec<EnumBranch>),
}

#[derive(Debug)]
pub(crate) struct EnumBranch {
    pub branch: Branch,
    pub attrs: Vec<EAttr>,
}

#[derive(Debug)]
pub(crate) struct Branch {
    /// `Some(Enum)` for enum variants (emitted as `Enum::`), `None` for a plain struct.
    pub enum_name: Option<Ident>,
    pub ident: Ident,
    pub fields: Fields,
}

#[derive(Debug)]
pub(crate) enum Fields {
    Named(Vec<Field>),
    Unnamed(Vec<Field>),
    Unit(Ident, Vec<StrictName>, Option<Help>),
    Pure(Box<Expr>),
}

#[derive(Debug)]
pub(crate) struct Field {
    pub name: Option<Ident>,
    pub env: Vec<StrictName>,
    pub naming: Vec<StrictName>,
    pub consumer: Consumer,
    pub postpr: Vec<Post>,
    pub help: Option<Help>,
}

impl Field {
    /// Binding name used inside the generated `construct!`: the field name, or `f{ix}` for an
    /// unnamed (tuple) field.
    pub(crate) fn var_name(&self, ix: usize) -> Ident {
        match &self.name {
            Some(name) => name.clone(),
            None => Ident::new(&format!("f{}", ix), Span::call_site()),
        }
    }
}

/// How a field consumes a value. Fully resolved: `External` carries its identifier, and
/// `Argument`/`Positional` carry a [`Metavar`] rather than an `Option<LitStr>`.
#[derive(Debug)]
pub(crate) enum Consumer {
    Switch,
    Flag {
        present: Expr,
        absent: Expr,
    },
    ReqFlag {
        present: Expr,
    },
    Any {
        metavar: LitStr,
        ty: Option<Type>,
        check: Box<Expr>,
    },
    Argument {
        metavar: Metavar,
        ty: Option<Type>,
    },
    Positional {
        metavar: Metavar,
        ty: Option<Type>,
    },
    External {
        ident: Path,
    },
    Pure {
        expr: Expr,
    },
    PureWith {
        expr: Expr,
    },
}

pub(crate) enum HelpPlacement {
    AtName,
    AtConsumer,
    NotAvailable,
}

impl Consumer {
    pub(crate) fn needs_name(&self) -> bool {
        matches!(
            self,
            Consumer::Switch
                | Consumer::Flag { .. }
                | Consumer::ReqFlag { .. }
                | Consumer::Argument { .. }
        )
    }

    pub(crate) fn help_placement(&self) -> HelpPlacement {
        match self {
            Consumer::Switch
            | Consumer::Flag { .. }
            | Consumer::ReqFlag { .. }
            | Consumer::Argument { .. } => HelpPlacement::AtName,
            Consumer::Any { .. } | Consumer::Positional { .. } => HelpPlacement::AtConsumer,
            Consumer::External { .. } | Consumer::Pure { .. } | Consumer::PureWith { .. } => {
                HelpPlacement::NotAvailable
            }
        }
    }
}

/// Metavar for `argument` / `positional`; [`Metavar::Default`] renders as `"ARG"`.
#[derive(Debug)]
pub(crate) enum Metavar {
    Given(LitStr),
    Default,
}

/// A resolved name: emitted as `short(..)` / `long(..)` / `env(..)`.
#[derive(Debug)]
pub(crate) enum StrictName {
    Short(LitChar),
    Long(LitStr),
    Env(Box<Expr>),
}

#[derive(Debug)]
pub(crate) enum Post {
    /// Can change the type of the result.
    Parse(PostParse),
    /// Can't change the type but can change the behaviour.
    Decor(PostDecor),
}

#[derive(Debug)]
pub(crate) enum PostParse {
    Adjacent,
    StartAdjacent,
    Catch,
    Many,
    Collect,
    Count,
    Some_(Box<Expr>),
    Map(Box<Expr>),
    Optional,
    Parse(Box<Expr>),
    Strict,
    NonStrict,
    Anywhere,
}

#[derive(Debug)]
pub(crate) enum PostDecor {
    Complete(Box<Expr>),
    CompleteGroup(LitStr),
    CompleteShell(Box<Expr>),
    DebugFallback,
    DisplayFallback,
    FormatFallback(Box<Expr>),
    Fallback(Box<Expr>),
    FallbackWith(Box<Expr>),
    Last,
    GroupHelp(Box<Expr>),
    Guard(Box<Expr>, Box<Expr>),
    Hide,
    CustomUsage(Box<Expr>),
    HideUsage,
}

#[derive(Debug)]
pub(crate) enum Mode {
    Command {
        command: CommandCfg,
        options: OptionsCfg,
    },
    Options {
        options: OptionsCfg,
    },
    Parser {
        group_help: Option<Help>,
    },
}

#[derive(Debug)]
pub(crate) struct CommandCfg {
    /// Always present — defaulted from the type name during lowering.
    pub name: LitStr,
    pub long: Vec<LitStr>,
    pub short: Vec<LitChar>,
    pub help: Option<Help>,
}

#[derive(Debug, Default)]
pub(crate) struct OptionsCfg {
    pub cargo_helper: Option<LitStr>,
    pub descr: Option<Help>,
    pub footer: Option<Help>,
    pub header: Option<Help>,
    pub usage: Option<Box<Expr>>,
    pub version: Option<Box<Expr>>,
    pub max_width: Option<Box<Expr>>,
    pub fallback_usage: bool,
}

/// Resolved enum-variant decorators. The parse-stage `UnnamedCommand` / `UnitShort` / `UnitLong`
/// are gone — lowering turns them into `NamedCommand` / branch `StrictName`s — so codegen needs no
/// `unreachable!` arm.
#[derive(Debug)]
pub(crate) enum EAttr {
    NamedCommand(LitStr),
    FallbackUsage,
    CommandShort(LitChar),
    CommandLong(LitStr),
    Adjacent,
    Hide,
    Descr(Help),
    Header(Help),
    Footer(Help),
    Usage(Box<Expr>),
    ToOptions,
}
