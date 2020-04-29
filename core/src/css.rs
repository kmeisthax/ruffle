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
#[derive(Clone, Debug, Collect, PartialEq, Eq, Hash)]
#[collect(no_drop)]
pub enum Name {
    Display,
    FontFamily,
    FontSize,
    FontVariant,
    FontWeight,
}

/// The list of keyword properties we consider.
#[derive(Clone, Debug, Collect, PartialEq, Eq, Hash)]
#[collect(no_drop)]
pub enum Keyword {
    Block,
    Inline,
    InlineBlock,
}

pub type CSSStylesheet = stylesheet::Stylesheet<Name, Keyword>;

pub type CSSRule = stylesheet::Rule<Name, Keyword>;

pub type CSSProperty = stylesheet::Property<Name, Keyword>;

pub type CSSValue = values::Value<Keyword>;

pub type CSSComputedStyle = stylesheet::ComputedStyle<Name, Keyword>;

pub use combinators::Combinator;
pub use user_agent::ua_stylesheet;
