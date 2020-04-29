//! Utility function that generates a user-agent stylesheet.

use crate::css::Combinator;
use crate::html::css_types::{CSSKeyword, CSSName, CSSProperty, CSSRule, CSSStylesheet, CSSValue};

/// Construct our user-agent stylesheet.
pub fn ua_stylesheet() -> CSSStylesheet {
    let mut stylesheet = CSSStylesheet::new();

    let mut paragraph_rule =
        CSSRule::from_combinators(vec![Combinator::IsElement("p".to_string())]);
    paragraph_rule.add_property(
        CSSProperty::new(CSSName::Display, CSSValue::Keyword(CSSKeyword::Block)),
        false,
    );
    stylesheet.append_rule(paragraph_rule);

    let mut img_rule = CSSRule::from_combinators(vec![Combinator::IsElement("img".to_string())]);
    img_rule.add_property(
        CSSProperty::new(CSSName::Display, CSSValue::Keyword(CSSKeyword::InlineBlock)),
        false,
    );
    stylesheet.append_rule(img_rule);

    stylesheet
}
