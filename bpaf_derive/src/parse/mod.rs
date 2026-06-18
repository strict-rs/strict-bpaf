//! Stage A — parsing `DeriveInput` into the **raw** (unresolved, span-carrying) tree.
//!
//! darling ([`FromDeriveInput`]/[`FromField`]/[`FromVariant`]) recovers the outer shape (struct vs
//! enum, the fields/variants, and `#[doc]` forwarding); the ordered, mode-sensitive `#[bpaf(...)]`
//! grammar is consumed by the retained custom [`syn::parse`](syn::parse::Parse) parsers
//! ([`FieldAttrs`], [`TopInfo`], [`Ed`]) via [`Attribute::parse_args`](syn::Attribute::parse_args).
//!
//! Nothing here is resolved — defaulting, name inference, implicit `optional`/`many`, command-name
//! defaulting, and validation all happen in [`lower`](crate::lower).

use proc_macro2::Ident;
use syn::{Type, Visibility};

use crate::{
    attrs::FieldAttrs,
    help::Help,
    td::{Ed, TopInfo},
};

mod input;

pub(crate) use input::parse;

/// Raw top-level item: the type, its visibility, the parsed `#[bpaf(...)]` top annotation and
/// rustdoc, plus the structural body.
pub(crate) struct RawTop {
    pub ty: Ident,
    pub vis: Visibility,
    pub info: TopInfo,
    pub help: Option<Help>,
    pub body: RawBody,
}

pub(crate) enum RawBody {
    Struct(RawBranch),
    Enum(Vec<RawVariant>),
}

pub(crate) struct RawBranch {
    pub ident: Ident,
    pub fields: RawFields,
}

pub(crate) enum RawFields {
    Named(Vec<RawField>),
    Unnamed(Vec<RawField>),
    Unit,
}

pub(crate) struct RawField {
    pub name: Option<Ident>,
    pub ty: Type,
    pub attrs: FieldAttrs,
    pub help: Option<Help>,
}

pub(crate) struct RawVariant {
    pub branch: RawBranch,
    pub ed: Ed,
    pub help: Option<Help>,
}
