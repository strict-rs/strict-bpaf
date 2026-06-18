//! Code generation — `impl ToTokens for ir::*`.
//!
//! This stage is **infallible**: the [`ir`](crate::ir) is fully resolved by
//! [`lower`](crate::lower), so there is nothing left to validate or default here. The token output
//! must remain byte-identical to the pre-refactor derive; the `top_tests` / `field_tests` snapshot
//! suites are the oracle.

use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{ItemFn, Type, parse_quote, visit_mut::VisitMut};

use crate::{custom_path::CratePathReplacer, ir::*};

/// `::<#ty>` turbofish for `argument` / `positional` / `any`.
struct TurboFish<'a>(&'a Type);

impl ToTokens for TurboFish<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty = &self.0;
        quote!(::<#ty>).to_tokens(tokens);
    }
}

impl ToTokens for Top {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Top {
            ty,
            vis,
            generate,
            body,
            mode,
            attrs,
            boxed,
            adjacent,
            start_adjacent,
            bpaf_path,
        } = self;
        let boxed = if *boxed { quote!(.boxed()) } else { quote!() };
        let adjacent = if *start_adjacent {
            quote!(.start_adjacent())
        } else if *adjacent {
            quote!(.adjacent())
        } else {
            quote!()
        };

        let original = match mode {
            Mode::Command { command, options } => {
                let OptionsCfg {
                    cargo_helper: _,
                    usage,
                    version,
                    descr,
                    footer,
                    header,
                    max_width,
                    fallback_usage,
                } = options;

                let version = version.as_ref().map(|v| quote!(.version(#v)));
                let usage = usage.as_ref().map(|v| quote!(.usage(#v)));
                let descr = descr.as_ref().map(|v| quote!(.descr(#v)));
                let footer = footer.as_ref().map(|v| quote!(.footer(#v)));
                let header = header.as_ref().map(|v| quote!(.header(#v)));
                let max_width = max_width.as_ref().map(|v| quote!(.max_width(#v)));
                let fallback_usage = if *fallback_usage {
                    Some(quote!(.fallback_to_usage()))
                } else {
                    None
                };
                let CommandCfg {
                    name,
                    long,
                    short,
                    help,
                } = command;
                let long = long.iter().map(|v| quote!(.long(#v)));
                let short = short.iter().map(|v| quote!(.short(#v)));
                let help = help.as_ref().map(|v| quote!(.help(#v)));
                quote! {
                    #[doc(hidden)]
                    #vis fn #generate() -> impl ::bpaf::Parser<#ty> {

                        #[allow(unused_imports)]
                        use ::bpaf::Parser;
                        #body
                        #(.#attrs)*
                        .to_options()
                        #fallback_usage
                        #version
                        #descr
                        #header
                        #footer
                        #usage
                        #max_width
                        .command(#name)
                        #(#short)*
                        #(#long)*
                        #help
                        #adjacent
                        #boxed
                    }
                }
            }
            Mode::Options { options } => {
                let OptionsCfg {
                    cargo_helper,
                    usage,
                    version,
                    descr,
                    footer,
                    header,
                    max_width,
                    fallback_usage,
                } = options;
                let body = match cargo_helper {
                    Some(cargo) => quote!(::bpaf::cargo_helper(#cargo, #body)),
                    None => quote!(#body),
                };

                let fallback_usage = if *fallback_usage {
                    Some(quote!(.fallback_to_usage()))
                } else {
                    None
                };
                let version = version.as_ref().map(|v| quote!(.version(#v)));
                let usage = usage.as_ref().map(|v| quote!(.usage(#v)));
                let descr = descr.as_ref().map(|v| quote!(.descr(#v)));
                let footer = footer.as_ref().map(|v| quote!(.footer(#v)));
                let header = header.as_ref().map(|v| quote!(.header(#v)));
                let max_width = max_width.as_ref().map(|v| quote!(.max_width(#v)));

                quote! {
                    #[doc(hidden)]
                    #vis fn #generate() -> ::bpaf::OptionParser<#ty> {
                        #[allow(unused_imports)]
                        use ::bpaf::Parser;
                        #body
                        #(.#attrs)*
                        .to_options()
                        #fallback_usage
                        #version
                        #descr
                        #header
                        #footer
                        #usage
                        #max_width
                    }
                }
            }
            Mode::Parser { group_help } => {
                let group_help = group_help.as_ref().map(|v| quote!(.group_help(#v)));
                quote! {
                    #[doc(hidden)]
                    #vis fn #generate() -> impl ::bpaf::Parser<#ty> {
                        #[allow(unused_imports)]
                        use ::bpaf::Parser;
                        #body
                        #adjacent
                        #group_help
                        #(.#attrs)*
                        #boxed
                    }
                }
            }
        };

        if let Some(custom_path) = bpaf_path {
            let mut replaced: ItemFn = parse_quote!(#original);
            CratePathReplacer::new(parse_quote!(::bpaf), custom_path.clone())
                .visit_item_fn_mut(&mut replaced);
            replaced.to_token_stream()
        } else {
            original
        }
        .to_tokens(tokens)
    }
}

impl ToTokens for Body {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Body::Single(branch) => quote!(#branch),
            Body::Alternatives(b) if b.len() == 1 => {
                let branch = &b[0];
                quote!(#branch)
            }
            Body::Alternatives(b) => {
                let branches = b.iter();
                let mk = |i| Ident::new(&format!("alt{}", i), Span::call_site());
                let name_f = b.iter().enumerate().map(|(n, _)| mk(n));
                let name_t = name_f.clone();
                quote! {{
                    #( let #name_f = #branches; )*
                    ::bpaf::construct!([ #( #name_t, )* ])
                }}
            }
        }
        .to_tokens(tokens);
    }
}

impl ToTokens for EnumBranch {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let EnumBranch { branch, attrs } = self;
        quote!(#branch #(.#attrs)*).to_tokens(tokens);
    }
}

impl ToTokens for Branch {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Branch {
            enum_name,
            ident,
            fields,
        } = self;
        let enum_name = enum_name.as_ref().map(|n| quote!(#n ::));
        match fields {
            Fields::Named(fields) if fields.is_empty() => {
                quote! {
                    ::bpaf::pure( #enum_name #ident {})
                }
            }
            Fields::Named(fields) => {
                let names = fields
                    .iter()
                    .enumerate()
                    .map(|(ix, field)| field.var_name(ix))
                    .collect::<Vec<_>>();
                let value = fields.iter();
                quote! {{
                    #( let #names = #value; )*
                    ::bpaf::construct!( #enum_name #ident { #( #names , )* })
                }}
            }

            Fields::Unnamed(fields) if fields.is_empty() => {
                quote! {
                    ::bpaf::pure( #enum_name #ident ())
                }
            }
            Fields::Unnamed(fields) => {
                let names = fields
                    .iter()
                    .enumerate()
                    .map(|(ix, field)| field.var_name(ix))
                    .collect::<Vec<_>>();
                let value = fields.iter();
                quote! {{
                    #( let #names = #value; )*
                    ::bpaf::construct!( #enum_name #ident ( #( #names , )* ))
                }}
            }
            Fields::Unit(ident, names, help) => {
                // `names` is always non-empty: lowering defaults a long name from the ident.
                let help = help.iter();
                quote!(::bpaf:: #( #names .)* #(help(#help).)* req_flag(#enum_name #ident))
            }
            Fields::Pure(x) => quote!(#x),
        }
        .to_tokens(tokens);
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Field {
            name: _,
            env,
            naming,
            consumer,
            postpr,
            help,
        } = self;

        let names = naming.iter().chain(env.iter());

        let prefix = if consumer.needs_name() {
            quote!(::bpaf::)
        } else {
            quote!()
        };

        let help = help.iter();

        match consumer.help_placement() {
            HelpPlacement::AtName => {
                quote!(#prefix #( #names .)* #(help(#help).)* #consumer #(.#postpr)*)
            }
            HelpPlacement::AtConsumer => {
                quote!(#prefix #( #names .)* #consumer #(.help(#help))* #(.#postpr)*)
            }
            HelpPlacement::NotAvailable => quote!(#prefix #(#names.)* #consumer #(.#postpr)*),
        }
        .to_tokens(tokens);
    }
}

impl ToTokens for Consumer {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Consumer::Switch => quote!(switch()),
            Consumer::Flag { present, absent } => quote!(flag(#present, #absent)),
            Consumer::ReqFlag { present } => quote!(req_flag(#present)),
            Consumer::Any { metavar, ty, check } => match ty {
                Some(ty) => quote!(::bpaf::any::<#ty, _, _>(#metavar, #check)),
                None => quote!(::bpaf::any(#metavar, #check)),
            },
            Consumer::Argument { metavar, ty } => {
                let tf = ty.as_ref().map(TurboFish);
                quote!(argument #tf(#metavar))
            }
            Consumer::Positional { metavar, ty } => {
                let tf = ty.as_ref().map(TurboFish);
                quote!(::bpaf::positional #tf(#metavar))
            }
            Consumer::External { ident } => {
                quote!(#ident())
            }
            Consumer::Pure { expr } => {
                quote!(::bpaf::pure(#expr))
            }
            Consumer::PureWith { expr } => {
                quote!(::bpaf::pure_with(#expr))
            }
        }
        .to_tokens(tokens);
    }
}

impl ToTokens for Metavar {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Metavar::Given(mv) => mv.to_tokens(tokens),
            Metavar::Default => quote!("ARG").to_tokens(tokens),
        }
    }
}

impl ToTokens for StrictName {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            StrictName::Short(name) => quote!(short(#name)),
            StrictName::Long(name) => quote!(long(#name)),
            StrictName::Env(name) => quote!(env(#name)),
        }
        .to_tokens(tokens);
    }
}

impl ToTokens for Post {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Post::Parse(p) => p.to_tokens(tokens),
            Post::Decor(p) => p.to_tokens(tokens),
        }
    }
}

impl ToTokens for PostParse {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            PostParse::Adjacent => quote!(adjacent()),
            PostParse::StartAdjacent => quote!(start_adjacent()),
            PostParse::Catch => quote!(catch()),
            PostParse::Many => quote!(many()),
            PostParse::Collect => quote!(collect()),
            PostParse::Count => quote!(count()),
            PostParse::Some_(msg) => quote!(some(#msg)),
            PostParse::Map(f) => quote!(map(#f)),
            PostParse::Optional => quote!(optional()),
            PostParse::Parse(f) => quote!(parse(#f)),
            PostParse::Strict => quote!(strict()),
            PostParse::NonStrict => quote!(non_strict()),
            PostParse::Anywhere => quote!(anywhere()),
        }
        .to_tokens(tokens);
    }
}

impl ToTokens for PostDecor {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            PostDecor::Complete(f) => quote!(complete(#f)),
            PostDecor::CompleteGroup(group) => quote!(group(#group)),
            PostDecor::CompleteShell(f) => quote!(complete_shell(#f)),
            PostDecor::DebugFallback => quote!(debug_fallback()),
            PostDecor::DisplayFallback => quote!(display_fallback()),
            PostDecor::FormatFallback(formatter) => quote!(format_fallback(#formatter)),
            PostDecor::Fallback(value) => quote!(fallback(#value)),
            PostDecor::FallbackWith(f) => quote!(fallback_with(#f)),
            PostDecor::Last => quote!(last()),
            PostDecor::GroupHelp(doc) => quote!(group_help(#doc)),
            PostDecor::Guard(check, msg) => quote!(guard(#check, #msg)),
            PostDecor::Hide => quote!(hide()),
            PostDecor::CustomUsage(usage) => quote!(custom_usage(#usage)),
            PostDecor::HideUsage => quote!(hide_usage()),
        }
        .to_tokens(tokens);
    }
}

impl ToTokens for EAttr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            EAttr::ToOptions => quote!(to_options()),
            EAttr::NamedCommand(n) => quote!(command(#n)),
            EAttr::CommandShort(n) => quote!(short(#n)),
            EAttr::CommandLong(n) => quote!(long(#n)),
            EAttr::Adjacent => quote!(adjacent()),
            EAttr::Descr(d) => quote!(descr(#d)),
            EAttr::Header(d) => quote!(header(#d)),
            EAttr::Footer(d) => quote!(footer(#d)),
            EAttr::Usage(u) => quote!(usage(#u)),
            EAttr::Hide => quote!(hide()),
            EAttr::FallbackUsage => quote!(fallback_to_usage()),
        }
        .to_tokens(tokens);
    }
}
