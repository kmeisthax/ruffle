//! CSS Tests

use crate::css::property::{Property, PropertyName};
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
fn css_root_unset() {
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

#[test]
fn css_root_swapped_initial() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        Value::Initial,
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        Value::Inherit,
    ));
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

#[test]
fn css_root_set() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        TestCSSKeyword::KeywordB.into(),
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        TestCSSKeyword::KeywordA.into(),
    ));
    root.cascade(None);

    assert_eq!(
        root.get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
    assert_eq!(
        root.get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
}

#[test]
fn css_cascade_set_on_unset() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        TestCSSKeyword::KeywordB.into(),
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        TestCSSKeyword::KeywordA.into(),
    ));
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
}

#[test]
fn css_cascade_set_on_swapped_initial() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        TestCSSKeyword::KeywordB.into(),
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        TestCSSKeyword::KeywordA.into(),
    ));
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        Value::Initial,
    ));
    child.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        Value::Inherit,
    ));
    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
}

#[test]
fn css_cascade_set_on_set() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        TestCSSKeyword::KeywordB.into(),
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        TestCSSKeyword::KeywordB.into(),
    ));
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        TestCSSKeyword::KeywordA.into(),
    ));
    child.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        TestCSSKeyword::KeywordA.into(),
    ));

    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
}

#[test]
fn css_cascade_unset_on_unset() {
    let mut root = ComputedStyle::default();
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
}

#[test]
fn css_cascade_unset_on_swapped_initial() {
    let mut root = ComputedStyle::default();
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        Value::Initial,
    ));
    child.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        Value::Inherit,
    ));
    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
}

#[test]
fn css_cascade_unset_on_set() {
    let mut root = ComputedStyle::default();
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        TestCSSKeyword::KeywordA.into(),
    ));
    child.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        TestCSSKeyword::KeywordA.into(),
    ));

    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
}

#[test]
fn css_cascade_swapped_initial_on_unset() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        Value::Initial,
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        Value::Inherit,
    ));
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
}

#[test]
fn css_cascade_swapped_initial_on_swapped_initial() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        Value::Initial,
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        Value::Inherit,
    ));
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        Value::Initial,
    ));
    child.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        Value::Inherit,
    ));
    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordB.into()
    );
}

#[test]
fn css_cascade_swapped_initial_on_set() {
    let mut root = ComputedStyle::default();
    root.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        Value::Initial,
    ));
    root.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        Value::Inherit,
    ));
    root.cascade(None);

    let mut child = ComputedStyle::default();
    child.add_property(Property::new(
        TestCSSProperty::PropertyInherited,
        TestCSSKeyword::KeywordA.into(),
    ));
    child.add_property(Property::new(
        TestCSSProperty::PropertyNonInherited,
        TestCSSKeyword::KeywordA.into(),
    ));

    child.cascade(Some(&root));

    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
    assert_eq!(
        child
            .get_defined(&TestCSSProperty::PropertyNonInherited)
            .into_owned(),
        TestCSSKeyword::KeywordA.into()
    );
}
