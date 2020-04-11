//! CSS combinators

use crate::css::specificity::Specificity;
use crate::xml::{XMLName, XMLNode};
use gc_arena::Collect;

/// A CSS selector consists of a particular set of combinators; this lists all
/// of the combinators supported by the layout engine.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub enum Combinator {
    /// This combinator matches any element.
    Any,

    /// This combinator matches descendents.
    Descendent,

    /// This combinator matches children.
    Child,

    /// This combinator matches elements by tag name.
    IsElement(String),

    /// This combinator matches a particular element class.
    HasClass(String),

    /// This combinator matches an element's ID.
    HasId(String),
}

impl Combinator {
    /// Evaluate the combinator.
    ///
    /// A combinator is evaluated against a candidate set: a list of elements
    /// which are being searched to determine if the selector matches a
    /// particular element. The set will be mutated by the combinator to store
    /// the state of selector matching and to signal if the element does not
    /// match.
    ///
    /// To use `eval`, do the following:
    ///
    /// 1. Start with a candidate set consisting of the element to be tested.
    /// 2. Call `eval` on the next combinator in the list (starting from the
    ///    end of the selector) with the candidate set.
    /// 3. If the candidate set is now empty, the element does not match.
    /// 4. If the candidate set contains elements, and all combinators have
    ///    been evaluated, then the element matches.
    /// 5. If there are more combinators to evaluate, repeat this process from
    ///    step 2.
    pub fn eval<'gc>(&self, candidates: &mut Vec<XMLNode<'gc>>) {
        let mut new_candidates = vec![];

        match self {
            Self::Any => return,
            Self::Descendent => {
                for child in candidates.iter() {
                    if let Some(ancestors) = child.ancestors() {
                        for ancestor in ancestors {
                            new_candidates.push(ancestor);
                        }
                    }
                }
            }
            Self::Child => {
                for child in candidates.iter() {
                    if let Ok(Some(parent)) = child.parent() {
                        new_candidates.push(parent);
                    }
                }
            }
            Self::IsElement(filter_tag_name) => {
                for child in candidates.iter() {
                    if let Some(tag_name) = child.tag_name() {
                        if tag_name == XMLName::from_str(filter_tag_name) {
                            new_candidates.push(*child);
                        }
                    }
                }
            }
            Self::HasClass(target_class) => {
                for child in candidates.iter() {
                    if let Some(class_list) = child.attribute_value(&XMLName::from_str("class")) {
                        for class in class_list.split(' ') {
                            if class == target_class {
                                new_candidates.push(*child);
                                break;
                            }
                        }
                    }
                }
            }
            Self::HasId(target_id) => {
                for child in candidates.iter() {
                    if let Some(id) = child.attribute_value(&XMLName::from_str("id")) {
                        if id == *target_id {
                            new_candidates.push(*child);
                        }
                    }
                }
            }
        }

        *candidates = new_candidates;
    }

    /// Determine how much specificity this combinator contributes to rules
    /// that include it.
    pub fn specificity(&self) -> Specificity {
        match self {
            Self::Any => Default::default(),
            Self::Descendent => Default::default(),
            Self::Child => Default::default(),
            Self::IsElement(_) => Specificity::element(),
            Self::HasClass(_) => Specificity::class(),
            Self::HasId(_) => Specificity::id(),
        }
    }
}
