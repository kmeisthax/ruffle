//! CSS Tests

use crate::css::combinators::Combinator;
use crate::css::property::{Property, PropertyName};
use crate::css::stylesheet::{ComputedStyle, Rule, Stylesheet};
use crate::css::values::Value;
use crate::xml::{XMLDocument, XMLName, XMLNode};
use gc_arena::rootless_arena;

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

#[test]
fn css_combinator_any() {
    rootless_arena(|mc| {
        let combinator = Combinator::Any;

        let document = XMLDocument::new(mc);
        let root_node = XMLNode::new_element(mc, "html", document).unwrap();

        let mut candidates = vec![root_node];

        combinator.eval(&mut candidates);

        assert!(!candidates.is_empty());
    });
}

#[test]
fn css_combinator_is_element() {
    rootless_arena(|mc| {
        let combinator = Combinator::IsElement("html".to_string());

        let document = XMLDocument::new(mc);
        let root_node = XMLNode::new_element(mc, "html", document).unwrap();

        let mut candidates = vec![root_node];

        combinator.eval(&mut candidates);

        assert!(!candidates.is_empty());
    });
}

#[test]
fn css_combinator_has_class() {
    rootless_arena(|mc| {
        let combinator = Combinator::HasClass("my-class".to_string());

        let document = XMLDocument::new(mc);
        let root_node = XMLNode::new_element(mc, "html", document).unwrap();

        root_node.set_attribute_value(mc, &XMLName::from_str("class"), "my-other-class my-class");

        let mut candidates = vec![root_node];

        combinator.eval(&mut candidates);

        assert!(!candidates.is_empty());
    });
}

#[test]
fn css_combinator_has_id() {
    rootless_arena(|mc| {
        let combinator = Combinator::HasId("my-id".to_string());

        let document = XMLDocument::new(mc);
        let root_node = XMLNode::new_element(mc, "html", document).unwrap();

        root_node.set_attribute_value(mc, &XMLName::from_str("id"), "my-id");

        let mut candidates = vec![root_node];

        combinator.eval(&mut candidates);

        assert!(!candidates.is_empty());
    });
}

#[test]
fn css_rule_any() {
    rootless_arena(|mc| {
        let rule: Rule<TestCSSProperty, TestCSSKeyword> =
            Rule::from_combinators(vec![Combinator::Any]);

        let document = XMLDocument::new(mc);
        let root_node = XMLNode::new_element(mc, "html", document).unwrap();

        document.as_node().append_child(mc, root_node).unwrap();

        assert!(rule.applies_to(root_node));
    });
}

#[test]
fn css_rule_descendent() {
    rootless_arena(|mc| {
        let rule: Rule<TestCSSProperty, TestCSSKeyword> = Rule::from_combinators(vec![
            Combinator::IsElement("html".to_string()),
            Combinator::Descendent,
            Combinator::IsElement("body".to_string()),
        ]);

        let document = XMLDocument::new(mc);
        let mut root_node = XMLNode::new_element(mc, "html", document).unwrap();
        let child_node = XMLNode::new_element(mc, "body", document).unwrap();

        document.as_node().append_child(mc, root_node).unwrap();
        root_node.append_child(mc, child_node).unwrap();

        assert!(!rule.applies_to(root_node));
        assert!(rule.applies_to(child_node));
    });
}

#[test]
fn css_rule_child() {
    rootless_arena(|mc| {
        let rule: Rule<TestCSSProperty, TestCSSKeyword> = Rule::from_combinators(vec![
            Combinator::IsElement("body".to_string()),
            Combinator::Child,
            Combinator::IsElement("p".to_string()),
        ]);

        let document = XMLDocument::new(mc);
        let mut root_node = XMLNode::new_element(mc, "body", document).unwrap();
        let mut child_node1 = XMLNode::new_element(mc, "div", document).unwrap();
        let child_node2 = XMLNode::new_element(mc, "p", document).unwrap();
        let grandchild_node = XMLNode::new_element(mc, "p", document).unwrap();

        document.as_node().append_child(mc, root_node).unwrap();
        root_node.append_child(mc, child_node1).unwrap();
        root_node.append_child(mc, child_node2).unwrap();
        child_node1.append_child(mc, grandchild_node).unwrap();

        assert!(!rule.applies_to(root_node));
        assert!(!rule.applies_to(child_node1));
        assert!(rule.applies_to(child_node2));
        assert!(!rule.applies_to(grandchild_node));
    });
}

#[test]
fn css_stylesheet_any() {
    rootless_arena(|mc| {
        let mut stylesheet = Stylesheet::new();

        let mut rule = Rule::from_combinators(vec![Combinator::Any]);
        rule.add_property(
            Property::new(
                TestCSSProperty::PropertyInherited,
                TestCSSKeyword::KeywordB.into(),
            ),
            false,
        );
        rule.add_property(
            Property::new(
                TestCSSProperty::PropertyNonInherited,
                TestCSSKeyword::KeywordA.into(),
            ),
            false,
        );

        stylesheet.append_rule(rule);

        let document = XMLDocument::new(mc);
        let root_node = XMLNode::new_element(mc, "html", document).unwrap();

        document.as_node().append_child(mc, root_node).unwrap();

        let cs = stylesheet.compute_styles(root_node);

        assert_eq!(
            cs.get_defined(&TestCSSProperty::PropertyInherited)
                .into_owned(),
            TestCSSKeyword::KeywordB.into()
        );
        assert_eq!(
            cs.get_defined(&TestCSSProperty::PropertyNonInherited)
                .into_owned(),
            TestCSSKeyword::KeywordA.into()
        );
    });
}
