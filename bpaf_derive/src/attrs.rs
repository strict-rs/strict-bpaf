use proc_macro2::Span;
use syn::{
    Attribute, Error, Expr, Ident, LitChar, LitStr, Path, Result, Type,
    parse::{Parse, ParseStream},
    token,
};

use crate::{
    help::Help,
    utils::{
        doc_comment, parse_arg, parse_arg2, parse_expr, parse_lit_char, parse_lit_str,
        parse_opt_metavar, to_kebab_case,
    },
};

#[inline(never)]
fn type_fish(input: ParseStream) -> Result<Option<Type>> {
    Ok(if input.peek(token::Colon) {
        input.parse::<token::Colon>()?;
        input.parse::<token::Colon>()?;
        input.parse::<token::Lt>()?;
        let ty = input.parse::<Type>()?;
        input.parse::<token::Gt>()?;
        Some(ty)
    } else {
        None
    })
}

#[derive(Debug, Clone)]
pub enum Consumer {
    Switch {
        span: Span,
    },
    Flag {
        present: Expr,
        absent: Expr,
        span: Span,
    },
    ReqFlag {
        present: Expr,
        span: Span,
    },
    Any {
        metavar: LitStr,
        ty: Option<Type>,
        check: Box<Expr>,
        span: Span,
    },
    Argument {
        metavar: Option<LitStr>,
        ty: Option<Type>,
        span: Span,
    },
    Positional {
        metavar: Option<LitStr>,
        ty: Option<Type>,
        span: Span,
    },
    External {
        ident: Option<Path>,
        span: Span,
    },
    Pure {
        expr: Expr,
        span: Span,
    },
    PureWith {
        expr: Expr,
        span: Span,
    },
}

impl Consumer {
    pub fn span(&self) -> Span {
        match self {
            Consumer::Switch { span }
            | Consumer::Flag { span, .. }
            | Consumer::ReqFlag { span, .. }
            | Consumer::Any { span, .. }
            | Consumer::Argument { span, .. }
            | Consumer::Positional { span, .. }
            | Consumer::External { span, .. }
            | Consumer::PureWith { span, .. }
            | Consumer::Pure { span, .. } => *span,
        }
    }
}

impl Consumer {
    pub(crate) fn needs_name(&self) -> bool {
        match self {
            Consumer::Switch { .. }
            | Consumer::Flag { .. }
            | Consumer::ReqFlag { .. }
            | Consumer::Argument { .. } => true,
            Consumer::Pure { .. }
            | Consumer::PureWith { .. }
            | Consumer::Positional { .. }
            | Consumer::Any { .. }
            | Consumer::External { .. } => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Name {
    /// Short name, with override, if specified
    Short { name: Option<LitChar>, span: Span },
    /// Long name, with override, if specified
    Long { name: Option<LitStr>, span: Span },
    /// Enum variable, name must be specified
    Env { name: Box<Expr> },
}

impl StrictName {
    pub(crate) fn from_name(name: Name, ident: &Option<Ident>) -> Result<Self> {
        Ok(match name {
            Name::Short {
                name: Some(name), ..
            } => Self::Short { name },
            Name::Short { name: None, span } => match ident {
                Some(name) => {
                    let derived_name = to_kebab_case(&name.to_string()).chars().next().unwrap();
                    Self::Short {
                        name: LitChar::new(derived_name, span),
                    }
                }
                None => {
                    return Err(Error::new(
                        span,
                        "Can't derive an explicit name for unnamed struct, try adding a name here like short('f')",
                    ));
                }
            },
            Name::Long {
                name: Some(name), ..
            } => StrictName::Long { name },
            Name::Long { name: None, span } => match ident {
                Some(name) => {
                    let derived_name = to_kebab_case(&name.to_string());
                    Self::Long {
                        name: LitStr::new(&derived_name, span),
                    }
                }
                None => {
                    return Err(Error::new(
                        span,
                        "Can't derive an explicit name for unnamed struct, try adding a name here like long(\"arg\")",
                    ));
                }
            },
            Name::Env { name, .. } => Self::Env { name },
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum StrictName {
    Short { name: LitChar },
    Long { name: LitStr },
    Env { name: Box<Expr> },
}

#[derive(Debug, Clone)]
pub(crate) enum Post {
    /// Those items can change the type of the result
    Parse(PostParse),
    /// Those items can't change the type but can change the behavior
    Decor(PostDecor),
}

#[derive(Debug, Clone)]
pub(crate) enum PostParse {
    Adjacent { span: Span },
    StartAdjacent { span: Span },
    Catch { span: Span },
    Many { span: Span },
    Collect { span: Span },
    Count { span: Span },
    Some_ { span: Span, msg: Box<Expr> },
    Map { span: Span, f: Box<Expr> },
    Optional { span: Span },
    Parse { span: Span, f: Box<Expr> },
    Strict { span: Span },
    NonStrict { span: Span },
    Anywhere { span: Span },
}
impl PostParse {
    fn span(&self) -> Span {
        match self {
            Self::Adjacent { span }
            | Self::StartAdjacent { span }
            | Self::Catch { span }
            | Self::Many { span }
            | Self::Collect { span }
            | Self::Count { span }
            | Self::Some_ { span, .. }
            | Self::Map { span, .. }
            | Self::Optional { span }
            | Self::Parse { span, .. }
            | Self::Strict { span }
            | Self::NonStrict { span }
            | Self::Anywhere { span } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PostDecor {
    Complete {
        span: Span,
        f: Box<Expr>,
    },
    CompleteGroup {
        span: Span,
        group: LitStr,
    },
    CompleteShell {
        span: Span,
        f: Box<Expr>,
    },
    DebugFallback {
        span: Span,
    },
    DisplayFallback {
        span: Span,
    },
    FormatFallback {
        span: Span,
        formatter: Box<Expr>,
    },
    Fallback {
        span: Span,
        value: Box<Expr>,
    },
    FallbackWith {
        span: Span,
        f: Box<Expr>,
    },
    Last {
        span: Span,
    },
    GroupHelp {
        span: Span,
        doc: Box<Expr>,
    },
    Guard {
        span: Span,
        check: Box<Expr>,
        msg: Box<Expr>,
    },
    Hide {
        span: Span,
    },
    CustomUsage {
        usage: Box<Expr>,
        span: Span,
    },
    HideUsage {
        span: Span,
    },
}
impl PostDecor {
    fn span(&self) -> Span {
        match self {
            Self::Complete { span, .. }
            | Self::CompleteGroup { span, .. }
            | Self::CompleteShell { span, .. }
            | Self::DebugFallback { span }
            | Self::DisplayFallback { span }
            | Self::FormatFallback { span, .. }
            | Self::Fallback { span, .. }
            | Self::Last { span }
            | Self::FallbackWith { span, .. }
            | Self::GroupHelp { span, .. }
            | Self::Guard { span, .. }
            | Self::Hide { span }
            | Self::CustomUsage { span, .. }
            | Self::HideUsage { span } => *span,
        }
    }
}

impl Post {
    pub fn can_derive(&self) -> bool {
        match self {
            Post::Parse(_) => false,
            Post::Decor(_) => true,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Post::Parse(p) => p.span(),
            Post::Decor(d) => d.span(),
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct FieldAttrs {
    /// Names given with short, long and env
    pub naming: Vec<Name>,

    /// consumer attribute, derived
    pub consumer: Vec<Consumer>,

    /// post processing functions
    pub postpr: Vec<Post>,

    /// help specified by help(xxx)
    pub help: Vec<CustomHelp>,

    pub(crate) ignore_rustdoc: bool,
}

impl Name {
    pub(crate) fn parse(input: ParseStream, kw: &Ident) -> Result<Option<Self>> {
        let span = kw.span();
        Ok(Some(if kw == "short" {
            let name = if input.peek(token::Paren) {
                Some(parse_lit_char(input)?)
            } else {
                None
            };
            Name::Short { name, span }
        } else if kw == "long" {
            let name = if input.peek(token::Paren) {
                Some(parse_lit_str(input)?)
            } else {
                None
            };
            Name::Long { name, span }
        } else if kw == "env" {
            let name = parse_expr(input)?;
            Name::Env { name }
        } else {
            return Ok(None);
        }))
    }
}

impl Consumer {
    fn parse(input: ParseStream, kw: &Ident) -> Result<Option<Self>> {
        let span = kw.span();
        Ok(Some(if kw == "argument" {
            let ty = type_fish(input)?;
            let metavar = parse_opt_metavar(input)?;
            Consumer::Argument { metavar, ty, span }
        } else if kw == "positional" {
            let ty = type_fish(input)?;
            let metavar = parse_opt_metavar(input)?;
            Consumer::Positional { metavar, ty, span }
        } else if kw == "any" {
            let ty = type_fish(input)?;
            let (metavar, check) = parse_arg2(input)?;
            Consumer::Any {
                metavar,
                ty,
                check,
                span,
            }
        } else if kw == "switch" {
            Consumer::Switch { span }
        } else if kw == "flag" {
            let (present, absent) = parse_arg2(input)?;
            Consumer::Flag {
                present,
                absent,
                span,
            }
        } else if kw == "req_flag" {
            let present = parse_arg(input)?;
            Consumer::ReqFlag { present, span }
        } else if kw == "external" {
            let ident = if input.peek(token::Paren) {
                Some(parse_arg(input)?)
            } else {
                None
            };
            Consumer::External { ident, span }
        } else if kw == "pure" {
            let expr = parse_arg(input)?;
            Consumer::Pure { expr, span }
        } else if kw == "pure_with" {
            let expr = parse_arg(input)?;
            Consumer::PureWith { expr, span }
        } else {
            return Ok(None);
        }))
    }
}

impl PostParse {
    pub(crate) fn parse(input: ParseStream, kw: &Ident) -> Result<Option<Self>> {
        let span = kw.span();
        Ok(Some(if kw == "adjacent" {
            Self::Adjacent { span }
        } else if kw == "start_adjacent" {
            Self::StartAdjacent { span }
        } else if kw == "catch" {
            Self::Catch { span }
        } else if kw == "many" {
            Self::Many { span }
        } else if kw == "collect" {
            Self::Collect { span }
        } else if kw == "count" {
            Self::Count { span }
        } else if kw == "map" {
            let f = parse_arg(input)?;
            Self::Map { span, f }
        } else if kw == "optional" {
            Self::Optional { span }
        } else if kw == "parse" {
            let f = parse_arg(input)?;
            Self::Parse { span, f }
        } else if kw == "strict" {
            Self::Strict { span }
        } else if kw == "non_strict" {
            Self::NonStrict { span }
        } else if kw == "some" {
            let msg = parse_arg(input)?;
            Self::Some_ { span, msg }
        } else if kw == "anywhere" {
            Self::Anywhere { span }
        } else {
            return Ok(None);
        }))
    }
}

impl PostDecor {
    pub(crate) fn parse(input: ParseStream, kw: &Ident) -> Result<Option<Self>> {
        let span = kw.span();
        Ok(Some(if kw == "complete" {
            let f = parse_arg(input)?;
            Self::Complete { span, f }
        } else if kw == "group" {
            let group = parse_lit_str(input)?;
            Self::CompleteGroup { span, group }
        } else if kw == "complete_shell" {
            let f = parse_arg(input)?;
            Self::CompleteShell { span, f }
        } else if kw == "debug_fallback" {
            Self::DebugFallback { span }
        } else if kw == "display_fallback" {
            Self::DisplayFallback { span }
        } else if kw == "format_fallback" {
            let formatter = parse_expr(input)?;
            Self::FormatFallback { span, formatter }
        } else if kw == "fallback" {
            let value = parse_expr(input)?;
            Self::Fallback { span, value }
        } else if kw == "last" {
            Self::Last { span }
        } else if kw == "fallback_with" {
            let f = parse_expr(input)?;
            Self::FallbackWith { span, f }
        } else if kw == "group_help" {
            let doc = parse_expr(input)?;
            Self::GroupHelp { span, doc }
        } else if kw == "guard" {
            let (check, msg) = parse_arg2(input)?;
            Self::Guard { span, check, msg }
        } else if kw == "hide" {
            Self::Hide { span }
        } else if kw == "hide_usage" {
            Self::HideUsage { span }
        } else if kw == "custom_usage" {
            let usage = parse_arg(input)?;
            Self::CustomUsage { usage, span }
        } else {
            return Ok(None);
        }))
    }
}

#[derive(Debug)]
pub(crate) struct CustomHelp {
    pub span: Span,
    pub doc: Box<Expr>,
}

impl CustomHelp {
    fn parse(input: ParseStream, kw: &Ident) -> Result<Option<Self>> {
        let span = kw.span();
        Ok(if kw == "help" {
            let doc = parse_arg(input)?;
            Some(CustomHelp { span, doc })
        } else {
            None
        })
    }

    fn span(&self) -> Span {
        self.span
    }
}

impl Parse for FieldAttrs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut res = FieldAttrs::default();
        loop {
            let fork = input.fork();
            let kw = input.parse::<Ident>()?;
            if kw == "ignore_rustdoc" {
                res.ignore_rustdoc = true;
            } else if let Some(name) = Name::parse(input, &kw)? {
                res.naming.push(name);
            } else if let Some(cons) = Consumer::parse(input, &kw)? {
                res.consumer.push(cons);
            } else if let Some(pp) = PostParse::parse(input, &kw)? {
                res.postpr.push(Post::Parse(pp));
            } else if let Some(pp) = PostDecor::parse(input, &kw)? {
                res.postpr.push(Post::Decor(pp));
            } else if let Some(help) = CustomHelp::parse(input, &kw)? {
                res.help.push(help);
            } else {
                return Err(fork.error("Unexpected attribute in field annotation"));
            }

            if input.is_empty() {
                break;
            }
            input.parse::<token::Comma>()?;
            if input.is_empty() {
                break;
            }
        }
        res.validate()?;
        Ok(res)
    }
}

impl FieldAttrs {
    fn validate(&self) -> Result<()> {
        if self.consumer.len() > 1 {
            return Err(Error::new(
                self.consumer[1].span(),
                "Structure annotation can have only one consumer attribute",
            ));
        }

        if self.help.len() > 1 {
            return Err(Error::new(
                self.help[1].span(),
                "Structure annotation can have only one help attribute",
            ));
        }

        Ok(())
    }
}

pub(crate) fn parse_bpaf_doc_attrs<T>(attrs: &[Attribute]) -> Result<(Option<T>, Option<Help>)>
where
    T: Parse,
{
    let mut help = Vec::new();
    let mut parsed = None;

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Some(doc) = doc_comment(attr) {
                help.push(doc);
            }
        } else if attr.path().is_ident("bpaf") {
            parsed = Some(attr.parse_args::<T>()?);
        }
    }

    let help = if help.is_empty() {
        None
    } else {
        Some(Help::Doc(help.join("\n")))
    };

    Ok((parsed, help))
}
