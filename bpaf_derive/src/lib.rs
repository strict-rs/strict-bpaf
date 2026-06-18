//! # Derive macro for bpaf command line parser
//!
//! For documentation refer to `bpaf` library docs: <https://docs.rs/bpaf/latest/bpaf/>

mod attrs;
mod codegen;
mod field;
mod ir;
mod lower;
mod parse;
mod utils;

#[cfg(test)]
mod field_tests;
#[cfg(test)]
mod top_tests;

mod help;

mod custom_path;
mod td;

/// Derive macro for bpaf command line parser
///
/// For documentation refer to bpaf library: <https://docs.rs/bpaf/latest/bpaf/>
#[proc_macro_derive(Bpaf, attributes(bpaf))]
pub fn derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand(input.into()).into()
}

/// The three-stage pipeline: parse (raw) → lower (validate/resolve) → codegen. A failure in either
/// fallible stage becomes a `compile_error!`.
fn expand(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let raw = match parse::parse(input) {
        Ok(raw) => raw,
        Err(e) => return e.to_compile_error(),
    };
    match lower::lower(raw) {
        Ok(ir) => quote::ToTokens::to_token_stream(&ir),
        Err(e) => e.to_compile_error(),
    }
}

/// Run a single field through `parse → lower_field → codegen`. Used by `field_tests` so they keep
/// exercising field-level resolution in isolation against the real pipeline.
#[cfg(test)]
pub(crate) fn expand_field(
    named: bool,
    input: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    use quote::ToTokens;
    use syn::parse::Parser;
    let raw = (move |input: syn::parse::ParseStream| {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let _vis = input.parse::<syn::Visibility>()?;
        let name = if named {
            let n = input.parse::<proc_macro2::Ident>()?;
            input.parse::<syn::Token![:]>()?;
            Some(n)
        } else {
            None
        };
        let ty = input.parse::<syn::Type>()?;
        let (attrs, help) = attrs::parse_bpaf_doc_attrs::<attrs::FieldAttrs>(&attrs)?;
        syn::Result::Ok(parse::RawField {
            name,
            ty,
            attrs: attrs.unwrap_or_default(),
            help,
        })
    })
    .parse2(input)?;
    Ok(lower::lower_field(raw)?.to_token_stream())
}
