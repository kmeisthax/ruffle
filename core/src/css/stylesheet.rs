//! Stylesheet object model... objects

use crate::css::combinators::Combinator;
use crate::css::specificity::Specificity;
use crate::css::values::Value;
use crate::xml::XMLNode;
use gc_arena::Collect;

/// A CSS property is a combination of a name and the value the property should
/// be set to.
///
/// Multiple properties may apply for a particular name on a particular
/// element. Each name needs to be resolved to a single value before that value
/// can affect a particular element.
///
/// The `N` parameter is the enumerated type which constitutes all property
/// names we care about.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct Property<N>(N, Value);

/// A CSS Rule consists of a series of properties applied to elements matching
/// a particular selector.
///
/// The `N` parameter is the enumerated type which constitutes all property
/// names we care about.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct Rule<N> {
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
    properties: Vec<(Property<N>, bool)>,

    /// The index of this rule within it's contained stylesheet.
    rule_index: u32,
}

impl<N> Rule<N> {
    /// Determine if a rule applies to a node.
    ///
    /// A node matching a rule does not in and of itself determine if the
    /// properties in the rule apply to an element. Multiple rules can apply to
    /// a given element at one time, carrying conflicting property values. To
    /// resolve this, you must determine the specificity of the rule's selector
    /// and use it to determine priority order.
    fn applies_to<'gc>(&self, node: XMLNode<'gc>) -> bool {
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
}

/// A stylesheet consists of all rules declared in the stylesheet.
///
/// The `N` parameter is the enumerated type which constitutes all property
/// names we care about.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct Stylesheet<N> {
    rules: Vec<Rule<N>>,
}
