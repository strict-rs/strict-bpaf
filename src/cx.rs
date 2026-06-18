//! The [`Cx`] parser builder — the single canonical surface for constructing parsers.
//!
//! `Cx<I>` is a thin newtype around an inner parser-state `I`. Every constructor
//! ([`short`](crate::short), [`long`](crate::long), [`positional`](crate::positional), ...) and
//! every combinator ([`many`](crate::Parser::many), [`map`](crate::Parser::map), ...) returns some
//! `Cx<…>`, so a whole parser chain stays inside this one family of types. A single blanket
//! [`Parser`] implementation makes `Cx<I>` a parser exactly when its inner `I` is one — which is
//! how the type-state works: a half-built `Cx<Named>` is *not* a parser (you still have to pick a
//! consumer such as `switch` or `argument`), while a finished `Cx<Flag<bool>>` is.
//!
//! When you need to name a parser, use its concrete `Cx<…>` type, or erase it with
//! [`boxed`](crate::Parser::boxed) into a `Box<dyn Parser<T>>`. Hand-written code never needs
//! `impl Parser<T>`.

use crate::{Error, Meta, Parser, args::State};

/// A parser builder.
///
/// The inner type parameter records how far the parser has been built, and the methods available
/// depend on it. You normally don't keep a `Cx` value around long enough to spell its type — with
/// combinatoric usage you chain methods until you have something to hand to
/// [`construct!`](crate::construct!) or [`to_options`](Parser::to_options), and with the derive API
/// the macro takes care of it.
///
/// See the [module documentation](crate::cx) for the full story.
#[derive(Clone, Debug)]
pub struct Cx<I>(pub(crate) I);

impl<I> Cx<I> {
    /// Wrap an inner parser state into the [`Cx`] family.
    ///
    /// Used by the [`construct!`](crate::construct!) macro to build parsers in downstream crates;
    /// not part of the stable public API.
    #[doc(hidden)]
    pub fn new(inner: I) -> Self {
        Cx(inner)
    }
}

impl<T, I> Parser<T> for Cx<I>
where
    I: Parser<T>,
{
    fn eval(&self, args: &mut State) -> Result<T, Error> {
        self.0.eval(args)
    }

    fn meta(&self) -> Meta {
        self.0.meta()
    }
}

/// An erased inner parser, used by [`cx`] to lift an arbitrary [`Parser`] into the [`Cx`] family.
#[doc(hidden)]
pub struct Boxed<T>(pub(crate) Box<dyn Parser<T>>);

impl<T> Parser<T> for Boxed<T> {
    fn eval(&self, args: &mut State) -> Result<T, Error> {
        self.0.eval(args)
    }

    fn meta(&self) -> Meta {
        self.0.meta()
    }
}

/// Lift any [`Parser`] into the [`Cx`] family.
///
/// Most parsers already are `Cx<…>` (every constructor and combinator returns one, and both
/// [`construct!`](crate::construct!) and the derive macro produce them), so you rarely need this.
/// It is an escape hatch for the occasional hand-rolled [`Parser`] implementation that you want to
/// keep inside the family.
pub fn cx<T>(parser: impl Parser<T> + 'static) -> Cx<Boxed<T>> {
    Cx(Boxed(Box::new(parser)))
}
