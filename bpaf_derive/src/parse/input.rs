//! The darling-backed entry point: `DeriveInput` → [`RawTop`].
//!
//! darling recovers the outer shape (struct vs enum, the fields/variants) and forwards the raw
//! attributes; the ordered `#[bpaf(...)]` grammar and `#[doc]` extraction stay in the retained
//! custom parser [`parse_bpaf_doc_attrs`](crate::attrs::parse_bpaf_doc_attrs).

use darling::{FromDeriveInput, FromField, FromVariant, ast};
use proc_macro2::TokenStream;
use syn::{Attribute, DeriveInput, Ident, Result, Type, Visibility};

use crate::{
    attrs::{FieldAttrs, parse_bpaf_doc_attrs},
    parse::{RawBody, RawBranch, RawField, RawFields, RawTop, RawVariant},
    td::{Ed, TopInfo},
};

#[derive(FromDeriveInput)]
#[darling(forward_attrs)]
struct TopReceiver {
    ident: Ident,
    vis: Visibility,
    data: ast::Data<VariantReceiver, FieldReceiver>,
    attrs: Vec<Attribute>,
}

#[derive(FromField)]
#[darling(forward_attrs)]
struct FieldReceiver {
    ident: Option<Ident>,
    ty: Type,
    attrs: Vec<Attribute>,
}

#[derive(FromVariant)]
#[darling(forward_attrs)]
struct VariantReceiver {
    ident: Ident,
    fields: ast::Fields<FieldReceiver>,
    attrs: Vec<Attribute>,
}

/// Parse a derive input token stream into the raw tree, ready for [`lower`](crate::lower).
pub(crate) fn parse(input: TokenStream) -> Result<RawTop> {
    let di: DeriveInput = syn::parse2(input)?;
    let top = TopReceiver::from_derive_input(&di)
        .map_err(|e| syn::Error::new(e.span(), e.to_string()))?;

    let (info, help) = parse_bpaf_doc_attrs::<TopInfo>(&top.attrs)?;
    let info = info.unwrap_or_default();

    let body = match top.data {
        ast::Data::Struct(fields) => RawBody::Struct(RawBranch {
            ident: top.ident.clone(),
            fields: raw_fields(fields)?,
        }),
        ast::Data::Enum(variants) => {
            let vs = variants
                .into_iter()
                .map(parse_variant)
                .collect::<Result<Vec<_>>>()?;
            RawBody::Enum(vs)
        }
    };

    Ok(RawTop {
        ty: top.ident,
        vis: top.vis,
        info,
        help,
        body,
    })
}

fn raw_fields(fields: ast::Fields<FieldReceiver>) -> Result<RawFields> {
    Ok(match fields.style {
        ast::Style::Unit => RawFields::Unit,
        ast::Style::Tuple => RawFields::Unnamed(
            fields
                .fields
                .into_iter()
                .map(parse_field)
                .collect::<Result<Vec<_>>>()?,
        ),
        ast::Style::Struct => RawFields::Named(
            fields
                .fields
                .into_iter()
                .map(parse_field)
                .collect::<Result<Vec<_>>>()?,
        ),
    })
}

fn parse_field(f: FieldReceiver) -> Result<RawField> {
    let (attrs, help) = parse_bpaf_doc_attrs::<FieldAttrs>(&f.attrs)?;
    Ok(RawField {
        name: f.ident,
        ty: f.ty,
        attrs: attrs.unwrap_or_default(),
        help,
    })
}

fn parse_variant(v: VariantReceiver) -> Result<RawVariant> {
    let (ed, help) = parse_bpaf_doc_attrs::<Ed>(&v.attrs)?;
    let branch = RawBranch {
        ident: v.ident.clone(),
        fields: raw_fields(v.fields)?,
    };
    Ok(RawVariant {
        branch,
        ed: ed.unwrap_or_default(),
        help,
    })
}
