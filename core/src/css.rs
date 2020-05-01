//! CSS parsing and evaluation engine
//!
//! This module is generic with respects to the embedding environment that
//! wishes to use CSS. Most types provided require two parameters: an
//! enumeration of all valid property names, and an enumeration of all valid
//! CSS keywords. Builtin keywords and literal formats are handled by the
//! `Value` enumeration in this module.
//!
//! The general process of working with CSS is as follows:
//!
//! 1. Parse the CSS stylesheet you wish to use, or construct one in-memory
//! 2. Compute styles for a given `XMLNode`
//! 3. Cascade computed styles to children from parent
//! 4. Inspect the result for property values you care about and take
//! appropriate layout action.

mod combinators;
mod property;
mod specificity;
mod stylesheet;
mod values;

#[cfg(test)]
mod tests;

pub use combinators::{Combinator, StyleNode};
pub use property::{Property, PropertyName};
pub use stylesheet::{ComputedStyle, Rule, Stylesheet};
pub use values::Value;
