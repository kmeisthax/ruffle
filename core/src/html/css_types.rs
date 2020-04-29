//! HTML/CSS interface module.
//!
//! This module contains the definition of the property names and values we can
//! parse. It also publishes type specializations of all the CSS machinery for
//! use with HTML.

use crate::css::{ComputedStyle, Property, PropertyName, Rule, Stylesheet, Value};
use gc_arena::Collect;

/// The list of CSS property names that we care about.
///
/// Note that a couple of rules apply to what constitutes a property:
///
/// 1. Composite properties such as `font` do not exist separately. Instead,
/// they are broken into their individual properties at parse time and resolved
/// separately.
/// 2. Properties not enumerated here will be silently dropped at parse time.
/// 3. Any property that requres a specific value or set of values must be
/// mentioned in `CSSKeyword`.
#[derive(Clone, Debug, Collect, PartialEq, Eq, Hash)]
#[collect(no_drop)]
pub enum CSSName {
    Display,
    FontFamily,
    FontSize,
    FontVariant,
    FontWeight,
}

impl PropertyName<CSSKeyword> for CSSName {
    fn is_inherited(&self) -> bool {
        match self {
            Self::Display => false,
            Self::FontFamily => true,
            Self::FontSize => true,
            Self::FontVariant => true,
            Self::FontWeight => true,
        }
    }

    fn initial_value(&self) -> Value<CSSKeyword> {
        match self {
            Self::Display => CSSKeyword::Inline.into(),
            Self::FontFamily => Value::Font(vec![]),
            Self::FontSize => CSSKeyword::Medium.into(),
            Self::FontVariant => CSSKeyword::Normal.into(),
            Self::FontWeight => CSSKeyword::Normal.into(),
        }
    }
}

/// The list of keyword properties we consider.
#[derive(Clone, Debug, Collect, PartialEq, Eq, Hash)]
#[collect(no_drop)]
pub enum CSSKeyword {
    /// `display: block`
    Block,

    /// `display: inline`
    Inline,

    /// `display: inline-block`
    InlineBlock,

    /// `font-size: medium`
    Medium,

    /// `font-variant: normal`, `font-weight: normal` etc
    Normal,
}

pub type CSSStylesheet = Stylesheet<CSSName, CSSKeyword>;

pub type CSSRule = Rule<CSSName, CSSKeyword>;

pub type CSSProperty = Property<CSSName, CSSKeyword>;

pub type CSSValue = Value<CSSKeyword>;

pub type CSSComputedStyle = ComputedStyle<CSSName, CSSKeyword>;
