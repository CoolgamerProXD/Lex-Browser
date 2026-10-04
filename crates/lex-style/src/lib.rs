//! Lex-owned CSS style computation.
//!
//! `lex-css` parses stylesheets and matches selectors. This crate is the next
//! architectural boundary: it cascades property-valid declarations, applies a
//! small HTML user-agent style layer, resolves CSS-wide keywords and supported
//! relative values, performs inheritance, and exposes complete typed
//! [`ComputedStyle`] values by stable DOM [`lex_dom::NodeId`]. Layout never
//! needs to inspect selectors or raw declarations.
//!
//! M7 intentionally supports a practical subset rather than full CSS. Box
//! shorthands accept one to four simple values. Percentages whose meaning
//! depends on a containing block, plus `vw`/`vh`, remain typed and unresolved
//! for M8. Pseudo-classes, custom properties, `calc()`, logical properties,
//! per-side longhands, and the full user-agent stylesheet are not implemented.

mod engine;
mod ua;
mod values;

pub use engine::{InvalidationReason, StyleEngine, StyleInvalidation};
pub use lex_css::Color;
pub use values::{
    BorderStyle, ComputedStyle, Display, Edges, FontWeight, LengthPercentage,
    LengthPercentageAuto, LineHeight, Property, Size, SpecifiedStyle, SpecifiedValue, TextAlign,
    Visibility, BLACK, LINK_BLUE, TRANSPARENT,
};
