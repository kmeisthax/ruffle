//! CSS Tests

use crate::css::property::PropertyName;
use crate::css::stylesheet::ComputedStyle;
use crate::css::values::Value;

#[derive(Clone, PartialEq, Eq, Hash)]
enum TestCSSProperty {
    PropertyInherited,
    PropertyNonInherited,
}

impl PropertyName<TestCSSKeyword> for TestCSSProperty {
    fn is_inherited(&self) -> bool {
        match self {
            Self::PropertyInherited => true,
            Self::PropertyNonInherited => false,
        }
    }

    fn initial_value(&self) -> Value<TestCSSKeyword> {
        match self {
            Self::PropertyInherited => TestCSSKeyword::KeywordA.into(),
            Self::PropertyNonInherited => TestCSSKeyword::KeywordB.into(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
enum TestCSSKeyword {
    KeywordA,
    KeywordB,
}

#[test]
fn css_root_initial() {
    let mut root = ComputedStyle::default();
    root.cascade(None);

    assert_eq!(
        root.get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        root.get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
}
