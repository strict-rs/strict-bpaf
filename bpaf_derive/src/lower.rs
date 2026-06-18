//! Stage B — lowering the raw parse tree into the validated [`ir`].
//!
//! All defaulting, name inference, consumer derivation, implicit `optional`/`many` insertion,
//! command-name defaulting and validation happens here, so [`codegen`](crate::codegen) is
//! infallible. The transformations mirror the pre-refactor `StructField::make`, `Top::parse` and
//! `ParsedEnumBranch::resolve` exactly, to keep the generated tokens byte-identical.

use proc_macro2::Ident;
use syn::{Error, LitChar, LitStr, Result, Type, Visibility, parse_quote, spanned::Spanned};

use crate::{
    attrs::{Consumer, Name, Post, PostDecor, PostParse, StrictName},
    field::{Shape, split_type},
    help::Help,
    ir,
    parse::{RawBody, RawBranch, RawField, RawFields, RawTop, RawVariant},
    td::{CommandCfg, EAttr, Mode, OptionsCfg, TopInfo},
    utils::{LineIter, to_kebab_case, to_snake_case},
};

fn ident_to_long(ident: &Ident) -> LitStr {
    LitStr::new(&to_kebab_case(&ident.to_string()), ident.span())
}

fn ident_to_short(ident: &Ident) -> LitChar {
    LitChar::new(
        to_kebab_case(&ident.to_string()).chars().next().unwrap(),
        ident.span(),
    )
}

/// Lower a fully-parsed raw item into the validated [`ir::Top`].
pub(crate) fn lower(raw: RawTop) -> Result<ir::Top> {
    let RawTop {
        ty,
        vis,
        info,
        mut help,
        body,
    } = raw;
    let TopInfo {
        private,
        custom_name,
        boxed,
        mut mode,
        attrs,
        ignore_rustdoc,
        adjacent,
        start_adjacent,
        bpaf_path,
    } = info;

    if ignore_rustdoc {
        help = None;
    }

    // Lower the structural body. For a named/unnamed command on a unit struct the body collapses
    // into a `pure`, and an unnamed command derives its name from the type.
    let mut body = lower_body(body, &ty)?;

    if let Mode::Command { command, .. } = &mut mode {
        if let Some(name) = &command.name {
            set_named_command(&mut body, name.span())?;
        } else {
            set_unnamed_command(&mut body);
            command.name = Some(ident_to_long(&ty));
        }
    }

    let mode = lower_mode(mode, help.take())?;

    Ok(ir::Top {
        vis: if private { Visibility::Inherited } else { vis },
        mode,
        generate: custom_name
            .unwrap_or_else(|| Ident::new(&to_snake_case(&ty.to_string()), ty.span())),
        ty,
        attrs: attrs.into_iter().map(lower_postdecor).collect(),
        body,
        boxed,
        adjacent,
        start_adjacent,
        bpaf_path,
    })
}

fn lower_body(body: RawBody, ty: &Ident) -> Result<ir::Body> {
    Ok(match body {
        RawBody::Struct(branch) => {
            // A unit struct becomes a `req_flag` and, like a unit enum variant, needs an implicit
            // long name derived from its type (this used to be defaulted in codegen). No-op for
            // named/unnamed structs.
            let mut branch = lower_branch(branch, None)?;
            let ident = branch.ident.clone();
            set_implicit_name(&mut branch, &ident);
            ir::Body::Single(branch)
        }
        RawBody::Enum(variants) => {
            let branches = variants
                .into_iter()
                .filter_map(|v| lower_variant(v, ty).transpose())
                .collect::<Result<Vec<_>>>()?;
            ir::Body::Alternatives(branches)
        }
    })
}

fn lower_branch(branch: RawBranch, enum_name: Option<Ident>) -> Result<ir::Branch> {
    let RawBranch { ident, fields } = branch;
    let fields = match fields {
        RawFields::Named(fs) => ir::Fields::Named(
            fs.into_iter()
                .map(lower_field)
                .collect::<Result<Vec<_>>>()?,
        ),
        RawFields::Unnamed(fs) => ir::Fields::Unnamed(
            fs.into_iter()
                .map(lower_field)
                .collect::<Result<Vec<_>>>()?,
        ),
        RawFields::Unit => ir::Fields::Unit(ident.clone(), Vec::new(), None),
    };
    Ok(ir::Branch {
        enum_name,
        ident,
        fields,
    })
}

/// Mirrors `Body::set_named_command`: a named command may only sit on a struct, and turns a unit
/// branch into a `pure`.
fn set_named_command(body: &mut ir::Body, span: proc_macro2::Span) -> Result<()> {
    match body {
        ir::Body::Single(branch) => {
            set_command(branch);
            Ok(())
        }
        ir::Body::Alternatives(_) => Err(Error::new(
            span,
            "You can't annotate `enum` with a named command.",
        )),
    }
}

fn set_unnamed_command(body: &mut ir::Body) {
    if let ir::Body::Single(branch) = body {
        set_command(branch);
    }
}

/// Mirrors `Branch::set_command`: replace a unit branch's fields with `::bpaf::pure(Variant)`.
fn set_command(branch: &mut ir::Branch) {
    if let ir::Fields::Unit(..) = branch.fields {
        let ident = &branch.ident;
        let prefix = branch.enum_name.as_ref().map(quote_prefix);
        branch.fields = ir::Fields::Pure(parse_quote!(::bpaf::pure(#prefix #ident)));
    }
}

fn quote_prefix(name: &Ident) -> proc_macro2::TokenStream {
    quote::quote!(#name ::)
}

/// Mirrors `StructField::make` — the field-level resolution.
#[allow(clippy::too_many_lines)]
pub(crate) fn lower_field(raw: RawField) -> Result<ir::Field> {
    let RawField {
        name,
        ty,
        mut attrs,
        mut help,
    } = raw;

    if attrs.ignore_rustdoc {
        help = None;
    }

    let derived_consumer = attrs.consumer.is_empty();

    let mut cons = match attrs.consumer.pop() {
        Some(cons) => cons,
        None => derive_consumer(name.is_some() || !attrs.naming.is_empty(), &ty)?,
    };

    if let Consumer::External { span, ident: None } = &cons {
        let span = *span;
        match name.as_ref() {
            Some(n) => {
                let ident = Ident::new(&to_snake_case(&n.to_string()), n.span());
                cons = Consumer::External {
                    span,
                    ident: Some(ident.into()),
                };
            }
            None => {
                return Err(Error::new(
                    span,
                    "Can't derive name for this external, try specifying one",
                ));
            }
        }
    }

    let mut env = Vec::new();
    let mut naming = Vec::new();
    for attr in attrs.naming {
        if let Name::Env { name, .. } = attr {
            env.push(StrictName::Env { name });
        } else {
            naming.push(StrictName::from_name(attr, &name)?);
        }
    }

    match (cons.needs_name(), !naming.is_empty()) {
        (true, true) | (false, false) => {}
        (true, false) => match &name {
            Some(n) => {
                let span = n.span();
                if n.to_string().chars().count() == 1 {
                    let short = Name::Short { name: None, span };
                    naming.push(StrictName::from_name(short, &name)?);
                } else {
                    let long = Name::Long { name: None, span };
                    naming.push(StrictName::from_name(long, &name)?);
                }
            }
            None => {
                return Err(Error::new(
                    cons.span(),
                    "This consumer needs a name, you can specify it with long(\"name\") or short('n')",
                ));
            }
        },
        (false, true) => {
            return Err(Error::new_spanned(
                ty,
                "field doesn't take a name annotation",
            ));
        }
    };

    let mut postpr = std::mem::take(&mut attrs.postpr);

    let shape = split_type(&ty);

    if let Consumer::Argument { ty, .. }
    | Consumer::Positional { ty, .. }
    | Consumer::Any { ty, .. } = &mut cons
        && ty.is_none()
    {
        match &shape {
            Shape::Optional(t) | Shape::Multiple(t) | Shape::Direct(t) => {
                *ty = Some(t.clone());
            }
            _ => {}
        }
    }

    if derived_consumer {
        for pp in &postpr {
            if !pp.can_derive() {
                let err = Error::new(
                    pp.span(),
                    "Can't derive implicit consumer with this annotation present",
                );
                return Err(err);
            }
        }
    }
    let span = ty.span();

    if !(postpr.iter().any(|p| matches!(p, Post::Parse(_)))
        || matches!(cons, Consumer::External { .. } | Consumer::Pure { .. }))
    {
        match shape {
            Shape::Optional(_) => postpr.insert(0, Post::Parse(PostParse::Optional { span })),
            Shape::Multiple(_) => postpr.insert(0, Post::Parse(PostParse::Many { span })),
            Shape::Bool => {
                if name.is_none() && naming.is_empty() && matches!(cons, Consumer::Switch { .. }) {
                    let msg = "Can't derive consumer for unnamed boolean field, try adding one of #[bpaf(positional)], #[bpaf(long(\"name\")] or #[bpaf(short('n'))] annotations to it";
                    let err = Error::new_spanned(ty, msg);
                    return Err(err);
                }
            }
            Shape::Unit | Shape::Direct(_) => {}
        }
    }

    let help = match attrs.help.pop() {
        Some(h) => Some(Help::Custom(h.doc)),
        None => help,
    };

    Ok(ir::Field {
        name,
        env: env.into_iter().map(lower_strict_name).collect(),
        naming: naming.into_iter().map(lower_strict_name).collect(),
        consumer: lower_consumer(cons),
        postpr: postpr.into_iter().map(lower_post).collect(),
        help,
    })
}

/// Mirrors `derive_consumer`.
fn derive_consumer(name_present: bool, ty: &Type) -> Result<Consumer> {
    let span = ty.span();
    Ok(match split_type(ty) {
        Shape::Bool => {
            if name_present {
                Consumer::Switch { span }
            } else {
                let msg = "Refusing to derive a positional item for bool, you can fix this by either adding a short/long name or making it positional explicitly";
                return Err(Error::new(ty.span(), msg));
            }
        }
        Shape::Unit => {
            if name_present {
                Consumer::ReqFlag {
                    present: parse_quote!(()),
                    span,
                }
            } else {
                let msg = "Refusing to derive a positional item for (), you can fix this by either adding a short/long name or making it positional explicitly";
                return Err(Error::new(ty.span(), msg));
            }
        }
        Shape::Optional(t) | Shape::Multiple(t) | Shape::Direct(t) => {
            let ty = Some(t);
            let metavar = None;
            if name_present {
                Consumer::Argument { metavar, ty, span }
            } else {
                Consumer::Positional { metavar, ty, span }
            }
        }
    })
}

/// Mirrors `ParsedEnumBranch::resolve` — resolves a single enum variant (or skips it).
fn lower_variant(raw: RawVariant, _enum_ty: &Ident) -> Result<Option<ir::EnumBranch>> {
    let RawVariant {
        branch,
        ed,
        mut help,
    } = raw;

    if ed.skip {
        return Ok(None);
    }

    let variant_ident = branch.ident.clone();
    let mut branch = lower_branch(branch, Some(_enum_ty.clone()))?;

    let mut attrs = Vec::with_capacity(ed.attrs.len());
    let mut has_options = None;
    let mut fallback_usage = false;
    for attr in ed.attrs {
        match attr {
            EAttr::NamedCommand(name) => {
                set_command(&mut branch);
                attrs.push(ir::EAttr::ToOptions);
                has_options = Some(attrs.len());
                attrs.push(ir::EAttr::NamedCommand(name));
            }
            EAttr::UnnamedCommand => {
                set_command(&mut branch);
                attrs.push(ir::EAttr::ToOptions);
                has_options = Some(attrs.len());
                attrs.push(ir::EAttr::NamedCommand(ident_to_long(&variant_ident)));
            }
            EAttr::CommandShort(c) => attrs.push(ir::EAttr::CommandShort(c)),
            EAttr::CommandLong(l) => attrs.push(ir::EAttr::CommandLong(l)),
            EAttr::UnitShort(n) => set_unit_name(
                &mut branch,
                ir::StrictName::Short(n.unwrap_or_else(|| ident_to_short(&variant_ident))),
            ),
            EAttr::UnitLong(n) => set_unit_name(
                &mut branch,
                ir::StrictName::Long(n.unwrap_or_else(|| ident_to_long(&variant_ident))),
            ),
            EAttr::Env(name) => set_unit_name(&mut branch, ir::StrictName::Env(name)),
            EAttr::Usage(u) => {
                if let Some(o) = attrs.iter().position(|i| matches!(i, ir::EAttr::ToOptions)) {
                    attrs.insert(o + 1, ir::EAttr::Usage(u));
                } else {
                    unreachable!();
                }
            }
            EAttr::Adjacent => attrs.push(ir::EAttr::Adjacent),
            EAttr::Hide => attrs.push(ir::EAttr::Hide),
            EAttr::Header(h) => insert_after_options(&mut attrs, ir::EAttr::Header(h)),
            EAttr::Footer(f) => insert_after_options(&mut attrs, ir::EAttr::Footer(f)),
            EAttr::FallbackUsage => fallback_usage = true,
        }
    }

    if let Some(opts_at) = has_options {
        if fallback_usage {
            attrs.insert(opts_at, ir::EAttr::FallbackUsage);
        }

        if let Some(h) = std::mem::take(&mut help) {
            split_ehelp_into(h, opts_at, &mut attrs);
        }
    }
    set_implicit_name(&mut branch, &variant_ident);
    if let Some(help) = help {
        push_help(&mut branch, help);
    }

    Ok(Some(ir::EnumBranch { branch, attrs }))
}

fn insert_after_options(attrs: &mut Vec<ir::EAttr>, attr: ir::EAttr) {
    if let Some(o) = attrs.iter().position(|i| matches!(i, ir::EAttr::ToOptions)) {
        attrs.insert(o + 1, attr);
    }
}

fn set_unit_name(branch: &mut ir::Branch, name: ir::StrictName) {
    if let ir::Fields::Unit(_, names, _) = &mut branch.fields {
        names.push(name);
    }
}

fn set_implicit_name(branch: &mut ir::Branch, ident: &Ident) {
    if let ir::Fields::Unit(_, names, _) = &mut branch.fields
        && !names
            .iter()
            .any(|n| matches!(n, ir::StrictName::Long(_) | ir::StrictName::Short(_)))
    {
        names.push(ir::StrictName::Long(ident_to_long(ident)));
    }
}

fn push_help(branch: &mut ir::Branch, help: Help) {
    if let ir::Fields::Unit(_, _, h) = &mut branch.fields {
        *h = Some(help);
    }
}

/// Mirrors `Top::parse`'s help split + `Mode` resolution into [`ir::Mode`].
fn lower_mode(mode: Mode, help: Option<Help>) -> Result<ir::Mode> {
    Ok(match mode {
        Mode::Command { command, options } => {
            let mut options = lower_options(options);
            if let Some(help) = help {
                split_options_help(help, &mut options);
            }
            let CommandCfg {
                name,
                long,
                short,
                help: cmd_help,
            } = command;
            ir::Mode::Command {
                command: ir::CommandCfg {
                    name: name.expect("command name defaulted during lowering"),
                    long,
                    short,
                    help: cmd_help,
                },
                options,
            }
        }
        Mode::Options { options } => {
            let mut options = lower_options(options);
            if let Some(help) = help {
                split_options_help(help, &mut options);
            }
            ir::Mode::Options { options }
        }
        Mode::Parser { parser } => {
            let mut group_help = parser.group_help;
            if let Some(help) = help
                && group_help.is_none()
            {
                group_help = Some(help);
            }
            ir::Mode::Parser { group_help }
        }
    })
}

fn lower_options(opts: OptionsCfg) -> ir::OptionsCfg {
    let OptionsCfg {
        cargo_helper,
        descr,
        footer,
        header,
        usage,
        version,
        max_width,
        fallback_usage,
    } = opts;
    ir::OptionsCfg {
        cargo_helper,
        descr,
        footer,
        header,
        usage,
        version,
        max_width,
        fallback_usage,
    }
}

/// Mirrors `split_options_help`.
fn split_options_help(h: Help, opts: &mut ir::OptionsCfg) {
    match &h {
        Help::Custom(_) => {
            if opts.descr.is_none() {
                opts.descr = Some(h);
            }
        }
        Help::Doc(c) => {
            let mut chunks = LineIter::from(c.as_str());
            if let Some(s) = chunks.next()
                && opts.descr.is_none()
            {
                opts.descr = Some(Help::Doc(s));
            }
            if let Some(s) = chunks.next()
                && !s.is_empty()
                && opts.header.is_none()
            {
                opts.header = Some(Help::Doc(s));
            }
            if let Some(s) = chunks.rest()
                && opts.footer.is_none()
            {
                opts.footer = Some(Help::Doc(s));
            }
        }
    }
}

/// Mirrors `split_ehelp_into`.
fn split_ehelp_into(h: Help, opts_at: usize, attrs: &mut Vec<ir::EAttr>) {
    match &h {
        Help::Custom(_) => attrs.push(ir::EAttr::Descr(h)),
        Help::Doc(c) => {
            let mut chunks = LineIter::from(c.as_str());
            if let Some(s) = chunks.next() {
                attrs.insert(opts_at, ir::EAttr::Descr(Help::Doc(s)));
            }
            if let Some(s) = chunks.next()
                && !s.is_empty()
            {
                attrs.insert(opts_at, ir::EAttr::Header(Help::Doc(s)));
            }
            if let Some(s) = chunks.rest() {
                attrs.insert(opts_at, ir::EAttr::Footer(Help::Doc(s)));
            }
        }
    }
}

// ---- raw → ir conversions for the leaf enums ----

fn lower_consumer(c: Consumer) -> ir::Consumer {
    match c {
        Consumer::Switch { .. } => ir::Consumer::Switch,
        Consumer::Flag {
            present, absent, ..
        } => ir::Consumer::Flag { present, absent },
        Consumer::ReqFlag { present, .. } => ir::Consumer::ReqFlag { present },
        Consumer::Any {
            metavar, ty, check, ..
        } => ir::Consumer::Any { metavar, ty, check },
        Consumer::Argument { metavar, ty, .. } => ir::Consumer::Argument {
            metavar: lower_metavar(metavar),
            ty,
        },
        Consumer::Positional { metavar, ty, .. } => ir::Consumer::Positional {
            metavar: lower_metavar(metavar),
            ty,
        },
        Consumer::External { ident, .. } => ir::Consumer::External {
            ident: ident.expect("external ident resolved during lowering"),
        },
        Consumer::Pure { expr, .. } => ir::Consumer::Pure { expr },
        Consumer::PureWith { expr, .. } => ir::Consumer::PureWith { expr },
    }
}

fn lower_metavar(m: Option<LitStr>) -> ir::Metavar {
    match m {
        Some(mv) => ir::Metavar::Given(mv),
        None => ir::Metavar::Default,
    }
}

fn lower_strict_name(n: StrictName) -> ir::StrictName {
    match n {
        StrictName::Short { name } => ir::StrictName::Short(name),
        StrictName::Long { name } => ir::StrictName::Long(name),
        StrictName::Env { name } => ir::StrictName::Env(name),
    }
}

fn lower_post(p: Post) -> ir::Post {
    match p {
        Post::Parse(pp) => ir::Post::Parse(lower_postparse(pp)),
        Post::Decor(pd) => ir::Post::Decor(lower_postdecor(pd)),
    }
}

fn lower_postparse(p: PostParse) -> ir::PostParse {
    match p {
        PostParse::Adjacent { .. } => ir::PostParse::Adjacent,
        PostParse::StartAdjacent { .. } => ir::PostParse::StartAdjacent,
        PostParse::Catch { .. } => ir::PostParse::Catch,
        PostParse::Many { .. } => ir::PostParse::Many,
        PostParse::Collect { .. } => ir::PostParse::Collect,
        PostParse::Count { .. } => ir::PostParse::Count,
        PostParse::Some_ { msg, .. } => ir::PostParse::Some_(msg),
        PostParse::Map { f, .. } => ir::PostParse::Map(f),
        PostParse::Optional { .. } => ir::PostParse::Optional,
        PostParse::Parse { f, .. } => ir::PostParse::Parse(f),
        PostParse::Strict { .. } => ir::PostParse::Strict,
        PostParse::NonStrict { .. } => ir::PostParse::NonStrict,
        PostParse::Anywhere { .. } => ir::PostParse::Anywhere,
    }
}

fn lower_postdecor(p: PostDecor) -> ir::PostDecor {
    match p {
        PostDecor::Complete { f, .. } => ir::PostDecor::Complete(f),
        PostDecor::CompleteGroup { group, .. } => ir::PostDecor::CompleteGroup(group),
        PostDecor::CompleteShell { f, .. } => ir::PostDecor::CompleteShell(f),
        PostDecor::DebugFallback { .. } => ir::PostDecor::DebugFallback,
        PostDecor::DisplayFallback { .. } => ir::PostDecor::DisplayFallback,
        PostDecor::FormatFallback { formatter, .. } => ir::PostDecor::FormatFallback(formatter),
        PostDecor::Fallback { value, .. } => ir::PostDecor::Fallback(value),
        PostDecor::FallbackWith { f, .. } => ir::PostDecor::FallbackWith(f),
        PostDecor::Last { .. } => ir::PostDecor::Last,
        PostDecor::GroupHelp { doc, .. } => ir::PostDecor::GroupHelp(doc),
        PostDecor::Guard { check, msg, .. } => ir::PostDecor::Guard(check, msg),
        PostDecor::Hide { .. } => ir::PostDecor::Hide,
        PostDecor::CustomUsage { usage, .. } => ir::PostDecor::CustomUsage(usage),
        PostDecor::HideUsage { .. } => ir::PostDecor::HideUsage,
    }
}

/// Golden-IR assertions: these pin the *resolved* shape produced by `parse → lower` (consumer
/// inference, implicit `optional`/`many`, env/naming split, external-name derivation, metavar and
/// mode/command defaulting). They are deliberately structural — the `ir` types carry no `PartialEq`,
/// so we destructure and `matches!` rather than compare whole trees.
#[cfg(test)]
mod golden_ir {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::quote;

    fn lower_top(ts: TokenStream) -> ir::Top {
        let raw = crate::parse::parse(ts).expect("parse stage failed");
        lower(raw).expect("lower stage failed")
    }

    /// Extract the single struct branch's named fields, or panic.
    fn named_fields(top: &ir::Top) -> &[ir::Field] {
        let ir::Body::Single(branch) = &top.body else {
            panic!("expected a single struct branch");
        };
        match &branch.fields {
            ir::Fields::Named(fs) => fs,
            other => panic!("expected named fields, got {other:?}"),
        }
    }

    #[test]
    fn named_string_infers_long_argument() {
        let top = lower_top(quote! { struct Opts { name: String } });
        let fields = named_fields(&top);
        assert_eq!(fields.len(), 1);
        let f = &fields[0];
        assert!(matches!(
            &f.consumer,
            ir::Consumer::Argument {
                metavar: ir::Metavar::Default,
                ty: Some(_)
            }
        ));
        assert!(matches!(&f.naming[..], [ir::StrictName::Long(l)] if l.value() == "name"));
        assert!(f.env.is_empty());
        assert!(f.postpr.is_empty());
        assert!(matches!(top.mode, ir::Mode::Parser { .. }));
    }

    #[test]
    fn optional_inserts_implicit_optional() {
        let top = lower_top(quote! { struct Opts { name: Option<String> } });
        let f = &named_fields(&top)[0];
        assert!(matches!(&f.consumer, ir::Consumer::Argument { .. }));
        assert!(matches!(
            &f.postpr[..],
            [ir::Post::Parse(ir::PostParse::Optional)]
        ));
    }

    #[test]
    fn vec_inserts_implicit_many() {
        let top = lower_top(quote! { struct Opts { items: Vec<u32> } });
        let f = &named_fields(&top)[0];
        assert!(matches!(
            &f.postpr[..],
            [ir::Post::Parse(ir::PostParse::Many)]
        ));
    }

    #[test]
    fn named_bool_infers_switch() {
        let top = lower_top(quote! { struct Opts { verbose: bool } });
        let f = &named_fields(&top)[0];
        assert!(matches!(&f.consumer, ir::Consumer::Switch));
        assert!(matches!(&f.naming[..], [ir::StrictName::Long(l)] if l.value() == "verbose"));
    }

    #[test]
    fn unnamed_field_infers_positional() {
        let top = lower_top(quote! { struct Opts(String); });
        let ir::Body::Single(branch) = &top.body else {
            panic!("expected single branch");
        };
        let ir::Fields::Unnamed(fields) = &branch.fields else {
            panic!("expected unnamed fields");
        };
        assert!(matches!(
            &fields[0].consumer,
            ir::Consumer::Positional { .. }
        ));
        assert!(fields[0].naming.is_empty());
    }

    #[test]
    fn env_and_naming_split() {
        let top = lower_top(quote! {
            struct Opts {
                #[bpaf(env("HOME_DIR"), long("dir"))]
                dir: String,
            }
        });
        let f = &named_fields(&top)[0];
        assert!(matches!(&f.env[..], [ir::StrictName::Env(_)]));
        assert!(matches!(&f.naming[..], [ir::StrictName::Long(l)] if l.value() == "dir"));
    }

    #[test]
    fn external_name_derived_from_field() {
        let top = lower_top(quote! {
            struct Opts {
                #[bpaf(external)]
                config: Config,
            }
        });
        let f = &named_fields(&top)[0];
        let ir::Consumer::External { ident } = &f.consumer else {
            panic!("expected external consumer");
        };
        assert_eq!(ident.segments.last().unwrap().ident.to_string(), "config");
    }

    #[test]
    fn explicit_metavar_is_given() {
        let top = lower_top(quote! {
            struct Opts {
                #[bpaf(argument("FILE"))]
                path: String,
            }
        });
        let f = &named_fields(&top)[0];
        assert!(matches!(
            &f.consumer,
            ir::Consumer::Argument {
                metavar: ir::Metavar::Given(m),
                ..
            } if m.value() == "FILE"
        ));
    }

    #[test]
    fn command_name_defaulted_from_type() {
        let top = lower_top(quote! {
            #[bpaf(command)]
            struct DoThing {
                #[bpaf(short)]
                x: u32,
            }
        });
        let ir::Mode::Command { command, .. } = &top.mode else {
            panic!("expected command mode");
        };
        assert_eq!(command.name.value(), "do-thing");
    }

    #[test]
    fn options_doc_becomes_descr() {
        let top = lower_top(quote! {
            #[doc = " a tool"]
            #[bpaf(options)]
            struct Opts {
                #[bpaf(short)]
                x: u32,
            }
        });
        let ir::Mode::Options { options } = &top.mode else {
            panic!("expected options mode");
        };
        assert!(options.descr.is_some());
    }

    #[test]
    fn top_level_start_adjacent_is_recorded() {
        let top = lower_top(quote! {
            #[bpaf(start_adjacent)]
            struct Group {
                #[bpaf(short('a'))]
                a: (),
            }
        });
        assert!(top.start_adjacent);
        assert!(!top.adjacent);
    }
}
