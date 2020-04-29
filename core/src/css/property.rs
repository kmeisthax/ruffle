//! Property trait

use crate::css::values::Value;
use gc_arena::Collect;
use std::hash::Hash;

/// Defines an enumeration of CSS properties.
///
/// In order to use the CSS engine to parse CSS, you must provide a list of
/// properties that the layout technology is interested in. That enumeration
/// must also implement this trait.
///
/// The `K` property is the keyword type that this property name type generates
/// default values for.
pub trait PropertyName<K>: Clone + Eq + Hash {
    /// Determine if a particular property is inherited.
    ///
    /// Any property can be set as `initial` or `inherit`. However, if a
    /// property is *not* set, or if it is deliberately `unset`, then it needs
    /// to default to either it's initial value or inheritance.
    fn is_inherited(&self) -> bool;

    /// Determine the initial value of a particular property.
    ///
    /// Every property must provide an initial value, even those which are
    /// expected to be inherited, as a property may be `unset` for the entire
    /// layout hierarchy including the root element.
    ///
    /// This function must not return `Initial`, `Inherit`, or `Unset`, or CSS
    /// operations will panic.
    fn initial_value(&self) -> Value<K>;
}

/// A CSS property is a combination of a name and the value the property should
/// be set to.
///
/// Multiple properties may apply for a particular name on a particular
/// element. Each name needs to be resolved to a single value before that value
/// can affect a particular element.
///
/// The `N` parameter is the enumerated type which constitutes all property
/// names we care about. The `K` parameter enumerates all CSS keywords we
/// recognize. Any CSS properties or values outside those two ranges will be
/// silently ignored by CSS parsing.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct Property<N, K>(pub N, pub Value<K>)
where
    N: PropertyName<K>;

impl<N, K> Property<N, K>
where
    N: PropertyName<K>,
{
    ///Create a new property declaration.
    pub fn new(name: N, value: Value<K>) -> Self {
        Self(name, value)
    }
}
