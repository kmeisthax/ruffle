//! CSS combinators

use crate::css::specificity::Specificity;
use gc_arena::Collect;

/// Trait which represents the target tree structure being styled.
///
/// In practice, the primary target of the `StyleNode` trait is our own
/// `XMLNode<'gc>`, but any tree structure with unique IDs and class lists may
/// implement `StyleNode` to allow styling that structure with CSS. For
/// example, it may be desired to use a different HTML representation than an
/// XML tree; or we may wish to attach CSS to non-HTML styling hierarchies.
///
/// `StyleNode`s are intended to be opaque references into a particular tree
/// structure. They will be copied frequently during the combinator evaluation
/// process, and whatever object implements `StyleNode` must hold the pointer
/// type, not the actual data. This has implications for various memory
/// management strategies. Assuming `T` is an inner data structure holding the
/// underlying node's data:
///
///  * Reference-counted `StyleNode`s must hold an `Rc<T>`
///  * Garbage-collected `StyleNode`s (using `gc_arena`) must hold a
///    `GcCell<'gc, T>`
///  * Array-managed `StyleNode`s (e.g. `generational_arena`) must hold an
///    `Index` and a `&mut 'a Arena<T>`.
///
/// The `'gc` lifetime parameter refers to the lifetime of whatever storage
/// holds the node data. For the first case, it would be `'static`; the second
/// would be `'gc`, and the third would be `'a`.
pub trait StyleNode<'gc>: Copy {
    /// Determine if a particular style node is a particular element type.
    fn is_element(&self, tag_name: &str) -> bool;

    /// Determine if a particular style node possesses a particular CSS class.
    fn has_class(&self, class: &str) -> bool;

    /// Determine if a particular style node possesses a particular ID.
    ///
    /// It is assumed that IDs are unique across the style tree, but this is
    /// not a hard requirement of this API.
    fn has_id(&self, id: &str) -> bool;

    /// Retrieve the parent of this node, or `None` if this is the root node.
    fn parent(&self) -> Option<Self>;
}

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
    pub fn eval<'gc, S>(&self, candidates: &mut Vec<S>)
    where
        S: StyleNode<'gc>,
    {
        let mut new_candidates = vec![];

        match self {
            Self::Any => return,
            Self::Descendent => {
                for child in candidates.iter() {
                    let mut parent = child.parent();
                    while let Some(ancestor) = parent {
                        new_candidates.push(ancestor);
                        parent = ancestor.parent();
                    }
                }
            }
            Self::Child => {
                for child in candidates.iter() {
                    if let Some(parent) = child.parent() {
                        new_candidates.push(parent);
                    }
                }
            }
            Self::IsElement(filter_tag_name) => {
                for child in candidates.iter() {
                    if child.is_element(filter_tag_name) {
                        new_candidates.push(*child);
                    }
                }
            }
            Self::HasClass(target_class) => {
                for child in candidates.iter() {
                    if child.has_class(target_class) {
                        new_candidates.push(*child);
                    }
                }
            }
            Self::HasId(target_id) => {
                for child in candidates.iter() {
                    if child.has_id(target_id) {
                        new_candidates.push(*child);
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
