//! CSS specificity sorting

use std::ops::{Add, AddAssign};

/// A measure of the sorting weight of individual CSS rules.
///
/// Rules can gain specificity by way of id, class, or element combinators.
/// These combinators are counted separately, with the resulting specificity
/// ordered by the id, class, and element components. Furthermore, the order
/// in which rules are encountered forms a fourth component to break ties
/// between otherwise equally-specific selectors.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity {
    id: u32,
    class: u32,
    element: u32,
    rule_index: u32,
}

impl Default for Specificity {
    fn default() -> Self {
        Self {
            id: 0,
            class: 0,
            element: 0,
            rule_index: 0,
        }
    }
}

impl From<(u32, u32, u32, u32)> for Specificity {
    fn from(components: (u32, u32, u32, u32)) -> Self {
        Self {
            id: components.0,
            class: components.1,
            element: components.2,
            rule_index: components.3,
        }
    }
}

impl Add for Specificity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            id: self.id + rhs.id,
            class: self.class + rhs.class,
            element: self.element + rhs.element,
            rule_index: self.rule_index + rhs.rule_index,
        }
    }
}

impl AddAssign for Specificity {
    fn add_assign(&mut self, rhs: Self) {
        self.id += rhs.id;
        self.class += rhs.class;
        self.element += rhs.element;
        self.rule_index += rhs.rule_index;
    }
}

impl Specificity {
    pub fn id() -> Self {
        Self {
            id: 1,
            class: 0,
            element: 0,
            rule_index: 0,
        }
    }

    pub fn class() -> Self {
        Self {
            id: 0,
            class: 1,
            element: 0,
            rule_index: 0,
        }
    }

    pub fn element() -> Self {
        Self {
            id: 0,
            class: 0,
            element: 1,
            rule_index: 0,
        }
    }
}
