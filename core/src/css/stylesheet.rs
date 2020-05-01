//! Stylesheet object model... objects

use crate::css::combinators::{Combinator, StyleNode};
use crate::css::property::{Property, PropertyName};
use crate::css::specificity::Specificity;
use crate::css::values::Value;
use gc_arena::Collect;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

/// A CSS Rule consists of a series of properties applied to elements matching
/// a particular selector.
///
/// The `N` parameter is the enumerated type which constitutes all property
/// names we care about. The `K` parameter enumerates all CSS keywords we
/// recognize. Any CSS properties or values outside those two ranges will be
/// silently ignored by CSS parsing.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct Rule<N, K>
where
    N: PropertyName<K>,
{
    /// The selector that determines if this rule matches an element.
    ///
    /// Combinators are evaluated right-to-left against a candidate matching
    /// set that initially contains only the element being checked. Certain
    /// combinators will either alter or expand the candidate set, and others
    /// will shrink the set. A rule is considered to have been matched if at
    /// least one element remains in the candidate set after execution.
    selector: Vec<Combinator>,

    /// A list of properties, in source order, to be applied to the calculated
    /// style.
    ///
    /// The `bool` indicates if the property is `!important`. Important
    /// properties override non-important properties regardless of specificity.
    properties: Vec<(Property<N, K>, bool)>,

    /// The index of this rule within it's contained stylesheet.
    rule_index: u32,
}

impl<N, K> Rule<N, K>
where
    N: PropertyName<K>,
{
    /// Construct a rule from a list of combinators.
    pub fn from_combinators(combinators: Vec<Combinator>) -> Self {
        Rule {
            selector: combinators,
            properties: Vec::new(),
            rule_index: 0,
        }
    }

    /// Set the rule index of the rule.
    ///
    /// The rule index is used for specificity calculations.
    fn set_rule_index(&mut self, index: u32) {
        self.rule_index = index;
    }

    pub fn add_property(&mut self, property: Property<N, K>, is_important: bool) {
        self.properties.push((property, is_important));
    }

    /// Determine if a rule applies to a node.
    ///
    /// A node matching a rule does not in and of itself determine if the
    /// properties in the rule apply to an element. Multiple rules can apply to
    /// a given element at one time, carrying conflicting property values. To
    /// resolve this, you must determine the specificity of the rule's selector
    /// and use it to determine priority order.
    fn applies_to<'gc, S>(&self, node: S) -> bool
    where
        S: StyleNode<'gc>,
    {
        let mut candidates = vec![node];

        for combi in self.selector.iter().rev() {
            combi.eval(&mut candidates);

            if candidates.is_empty() {
                return false;
            }
        }

        candidates.is_empty()
    }

    /// Determine the specificity of the given rule.
    ///
    /// Specificity is a quantity used to sort applicable rules. For more
    /// information, read the documentation of the `Specificity` struct.
    fn specificity(&self) -> Specificity {
        let mut specificity = Specificity::from((0, 0, 0, self.rule_index));

        for combi in self.selector.iter() {
            specificity += combi.specificity();
        }

        specificity
    }

    fn iter_properties(&self) -> impl Iterator<Item = &(Property<N, K>, bool)> {
        self.properties.iter()
    }
}

/// A stylesheet consists of all rules declared in the stylesheet.
///
/// The `N` parameter is the enumerated type which constitutes all property
/// names we care about. The `K` parameter enumerates all CSS keywords we
/// recognize. Any CSS properties or values outside those two ranges will be
/// silently ignored by CSS parsing.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct Stylesheet<N, K>
where
    N: PropertyName<K>,
{
    rules: Vec<Rule<N, K>>,
}

impl<N, K> Default for Stylesheet<N, K>
where
    N: PropertyName<K>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<N, K> Stylesheet<N, K>
where
    N: PropertyName<K>,
{
    pub fn new() -> Self {
        Stylesheet { rules: Vec::new() }
    }

    /// Add a rule to the stylesheet.
    pub fn append_rule(&mut self, mut rule: Rule<N, K>) {
        rule.set_rule_index(self.rules.len() as u32);

        self.rules.push(rule);
    }
}

impl<N, K> Stylesheet<N, K>
where
    N: PropertyName<K>,
    K: Clone,
{
    /// Compute the styles that would apply to a given node with this
    /// stylesheet.
    pub fn compute_styles<'gc, S>(&self, node: S) -> ComputedStyle<N, K>
    where
        S: StyleNode<'gc>,
    {
        let mut computed_style = ComputedStyle::default();
        let mut sorted_rules = BTreeMap::new();

        for (index, rule) in self.rules.iter().enumerate() {
            if rule.applies_to(node) {
                sorted_rules.insert(rule.specificity(), index);
            }
        }

        for (_specificity, index) in sorted_rules.iter() {
            let rule = self.rules.get(*index).unwrap();
            for property in rule
                .iter_properties()
                .filter_map(|(p, imp)| if *imp { Some(p) } else { None })
            {
                computed_style.add_property(property.clone());
            }

            for property in rule
                .iter_properties()
                .filter_map(|(p, imp)| if !*imp { Some(p) } else { None })
            {
                computed_style.add_property(property.clone());
            }
        }

        computed_style
    }
}

/// An enumeration of all properties and values applied to a particular style.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct ComputedStyle<N, K>(HashMap<N, Value<K>>)
where
    N: PropertyName<K>;

impl<N, K> Default for ComputedStyle<N, K>
where
    N: PropertyName<K>,
{
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<N, K> ComputedStyle<N, K>
where
    N: PropertyName<K>,
    K: Clone,
{
    /// Retrieve the raw value of a particular property on this computed style.
    fn get(&self, name: &N) -> Cow<Value<K>> {
        match self.0.get(name) {
            Some(val) => Cow::Borrowed(val),
            None => Cow::Owned(Value::Unset),
        }
    }

    /// Retrieve the value of a particular property, forcing it to not be
    /// `unset`, `initial`, or `inherit`.
    ///
    /// You should use this function to retrieve property values from a
    /// computed style *after* cascading.
    pub fn get_defined(&self, name: &N) -> Cow<Value<K>> {
        let undef = self.get(name);
        match undef.as_ref() {
            Value::Unset | Value::Initial | Value::Inherit => Cow::Owned(name.initial_value()),
            _ => undef,
        }
    }

    /// Add a property to the computed style.
    ///
    /// If the property has already been set, it will be overridden.
    pub fn add_property(&mut self, property: Property<N, K>) {
        self.0.insert(property.0, property.1);
    }

    /// Given a name, check if the property in question is unresolved, and if
    /// so, attempt to resolve it.
    ///
    /// If the property is already resolved, or cannot be resolved with the
    /// current information, then this function returns `None`.
    fn resolve_unset(name: &N, value: &Value<K>, parent_value: &Value<K>) -> Option<Value<K>> {
        let parent_value = match parent_value {
            Value::Initial | Value::Inherit | Value::Unset => Cow::Owned(name.initial_value()),
            _ => Cow::Borrowed(parent_value),
        };

        match (value, name.is_inherited()) {
            (Value::Inherit, _) | (Value::Unset, true) => Some(parent_value.into_owned()),
            (Value::Initial, _) | (Value::Unset, false) => Some(name.initial_value()),
            _ => None,
        }
    }

    /// Cascade properties from a parent's computed styles into the child.
    ///
    /// This function will attempt to resolve any property mentioned in either
    /// the parent or the child. Properties not mentioned in either will remain
    /// `unset`. If the `parent` is `None`, indicating that we are at the root
    /// of the layout hierarchy, then all values will resolve as `initial`.
    /// This behavior also applies for properties which are not mentioned in
    /// either parent or child.
    pub fn cascade(&mut self, parent: Option<&Self>) {
        if let Some(parent) = parent {
            for (name, parent_value) in parent.0.iter() {
                if let Some(cascade) = Self::resolve_unset(name, &self.get(name), parent_value) {
                    self.0.insert(name.clone(), cascade);
                }
            }
        }

        for (name, value) in self.0.iter_mut() {
            if let Some(cascade) = Self::resolve_unset(
                name,
                value,
                &parent
                    .map(|p| p.get(name))
                    .unwrap_or(Cow::Owned(Value::Initial)),
            ) {
                *value = cascade;
            }
        }
    }
}
