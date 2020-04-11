//! CSS parsing and evaluation

use gc_arena::Collect;

mod combinators;
mod specificity;
mod stylesheet;
mod values;

/// The list of CSS property names that we care about.
///
/// Note that a couple of rules apply to what constitutes a property:
///
/// 1. Composite properties such as `font` do not exist separately. Instead,
/// they are broken into their individual properties at parse time and resolved
/// separately.
/// 2. Properties not enumerated here will be silently dropped at parse time.
/// 3. The values of these properties are not stored here. See `Value`.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub enum Name {
    FontFamily,
    FontSize,
    FontVariant,
    FontWeight,
}

pub type CSSStylesheet = stylesheet::Stylesheet<Name>;
