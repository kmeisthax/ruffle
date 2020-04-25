//! Represents AVM1 scope chain resolution.

use crate::avm1::return_value::ReturnValue;
use crate::avm1::{Avm1, Error, Object, ScriptObject, TObject, UpdateContext, Value};
use enumset::EnumSet;
use gc_arena::{GcCell, MutationContext};
use std::cell::Ref;

/// Indicates what kind of scope a scope is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ScopeClass {
    /// Scope represents global scope.
    Global,

    /// Target represents timeline scope. All timeline actions execute with
    /// the current clip object in lieu of a local scope, and the timeline scope
    /// can be changed via `tellTarget`.
    Target,

    /// Scope represents local scope and is inherited when a closure is defined.
    Local,

    /// Scope represents an object added to the scope chain with `with`.
    /// It is not inherited when closures are defined.
    With,
}

/// Represents a scope chain for an AVM1 activation.
#[derive(Debug)]
pub struct Scope<'gc> {
    parent: Option<GcCell<'gc, Scope<'gc>>>,
    class: ScopeClass,
    values: Object<'gc>,
}

unsafe impl<'gc> gc_arena::Collect for Scope<'gc> {
    #[inline]
    fn trace(&self, cc: gc_arena::CollectionContext) {
        self.parent.trace(cc);
        self.values.trace(cc);
    }
}

impl<'gc> Scope<'gc> {
    /// Construct a global scope (one without a parent).
    pub fn from_global_object(globals: Object<'gc>) -> Scope<'gc> {
        Scope {
            parent: None,
            class: ScopeClass::Global,
            values: globals,
        }
    }

    /// Construct a child scope of another scope.
    pub fn new_local_scope(parent: GcCell<'gc, Self>, mc: MutationContext<'gc, '_>) -> Scope<'gc> {
        Scope {
            parent: Some(parent),
            class: ScopeClass::Local,
            values: ScriptObject::object_cell(mc, None, None),
        }
    }

    /// Construct a closure scope to be used as the parent of all local scopes
    /// when invoking a function.
    ///
    /// This function filters With scopes from the scope chain. If all scopes
    /// are filtered (somehow), this function constructs and returns a new,
    /// single global scope with a bare object.
    pub fn new_closure_scope(
        mut parent: GcCell<'gc, Self>,
        mc: MutationContext<'gc, '_>,
    ) -> GcCell<'gc, Self> {
        let mut bottom_scope = None;
        let mut top_scope: Option<GcCell<'gc, Self>> = None;

        loop {
            if parent.read().class != ScopeClass::With {
                let next_scope = GcCell::allocate(
                    mc,
                    Self {
                        parent: None,
                        class: parent.read().class,
                        values: parent.read().values,
                    },
                );

                if bottom_scope.is_none() {
                    bottom_scope = Some(next_scope);
                }

                if let Some(ref scope) = top_scope {
                    scope.write(mc).parent = Some(next_scope);
                }

                top_scope = Some(next_scope);
            }

            let grandparent = parent.read().parent;
            if let Some(grandparent) = grandparent {
                parent = grandparent;
            } else {
                break;
            }
        }

        bottom_scope.unwrap_or_else(|| {
            GcCell::allocate(
                mc,
                Self {
                    parent: None,
                    class: ScopeClass::Global,
                    values: ScriptObject::object_cell(mc, None, None),
                },
            )
        })
    }

    /// Construct a scope for use with `tellTarget` code where the timeline
    /// scope has been replaced with another given object.
    pub fn new_target_scope(
        mut parent: GcCell<'gc, Self>,
        clip: Object<'gc>,
        mc: MutationContext<'gc, '_>,
    ) -> GcCell<'gc, Self> {
        let mut bottom_scope = None;
        let mut top_scope: Option<GcCell<'gc, Self>> = None;

        loop {
            let next_scope = GcCell::allocate(
                mc,
                Self {
                    parent: None,
                    class: parent.read().class,
                    values: parent.read().values,
                },
            );

            if parent.read().class == ScopeClass::Target {
                next_scope.write(mc).values = clip;
            }

            if bottom_scope.is_none() {
                bottom_scope = Some(next_scope);
            }

            if let Some(ref scope) = top_scope {
                scope.write(mc).parent = Some(next_scope);
            }

            top_scope = Some(next_scope);

            let grandparent = parent.read().parent;
            if let Some(grandparent) = grandparent {
                parent = grandparent;
            } else {
                break;
            }
        }

        bottom_scope.unwrap_or_else(|| {
            GcCell::allocate(
                mc,
                Self {
                    parent: None,
                    class: ScopeClass::Global,
                    values: ScriptObject::object_cell(mc, None, None),
                },
            )
        })
    }

    /// Construct a with scope to be used as the scope during a with block.
    ///
    /// A with block adds an object to the top of the scope chain, so unqualified
    /// references will try to resolve on that object first.
    pub fn new_with_scope(
        parent_scope: GcCell<'gc, Self>,
        with_object: Object<'gc>,
        mc: MutationContext<'gc, '_>,
    ) -> GcCell<'gc, Self> {
        GcCell::allocate(
            mc,
            Scope {
                parent: Some(parent_scope),
                class: ScopeClass::With,
                values: with_object,
            },
        )
    }

    /// Construct an arbitrary scope
    pub fn new(
        parent: GcCell<'gc, Self>,
        class: ScopeClass,
        with_object: Object<'gc>,
    ) -> Scope<'gc> {
        Scope {
            parent: Some(parent),
            class,
            values: with_object,
        }
    }

    /// Returns a reference to the current local scope object.
    pub fn locals(&self) -> &Object<'gc> {
        &self.values
    }

    /// Returns a reference to the current local scope object for mutation.
    #[allow(dead_code)]
    pub fn locals_mut(&mut self) -> &mut Object<'gc> {
        &mut self.values
    }

    /// Returns a reference to the parent scope object.
    pub fn parent(&self) -> Option<Ref<Scope<'gc>>> {
        match self.parent {
            Some(ref p) => Some(p.read()),
            None => None,
        }
    }

    /// Returns a reference to the parent scope object.
    pub fn parent_cell(&self) -> Option<GcCell<'gc, Scope<'gc>>> {
        self.parent
    }

    /// Resolve a particular value in the scope chain.
    ///
    /// Because scopes are object chains, the same rules for `Object::get`
    /// still apply here. This function is allowed to yield `None` to indicate
    /// that the result will be calculated on the AVM stack.
    pub fn resolve(
        &self,
        name: &str,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
        this: Object<'gc>,
    ) -> Result<ReturnValue<'gc>, Error> {
        if self.locals().has_property(avm, context, name) {
            return self.locals().get(name, avm, context);
        }
        if let Some(scope) = self.parent() {
            return scope.resolve(name, avm, context, this);
        }

        //TODO: Should undefined variables halt execution?
        Ok(Value::Undefined.into())
    }

    /// Check if a particular property in the scope chain is defined.
    pub fn is_defined(
        &self,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
        name: &str,
    ) -> bool {
        if self.locals().has_property(avm, context, name) {
            return true;
        }

        if let Some(scope) = self.parent() {
            return scope.is_defined(avm, context, name);
        }

        false
    }

    /// Update a particular value in the scope chain.
    ///
    /// Traverses the scope chain in search of a value. If it's found, it's overwritten.
    /// The traversal stops at Target scopes, which represents the movie clip timeline
    /// the code is executing in.
    /// If the value is not found, it is defined on this Target scope.
    pub fn set(
        &self,
        name: &str,
        value: Value<'gc>,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
        this: Object<'gc>,
    ) -> Result<(), Error> {
        if self.class == ScopeClass::Target
            || (self.locals().has_property(avm, context, name)
                && self.locals().is_property_overwritable(avm, name))
        {
            // Value found on this object, so overwrite it.
            // Or we've hit the executing movie clip, so create it here.
            self.locals().set(name, value, avm, context)
        } else if let Some(scope) = self.parent() {
            // Traverse the scope chain in search of the value.
            scope.set(name, value, avm, context, this)
        } else {
            // This probably shouldn't happen -- all AVM1 code runs in reference to some movieclip,
            // so we should always have a movieclip scope.
            // Define on the top-level scope.
            debug_assert!(false, "Scope::set: No top-level movie clip scope");
            self.locals().set(name, value, avm, context)
        }
    }

    /// Set a particular value in the locals for this scope.
    ///
    /// By convention, the locals for a given function are always defined as
    /// stored (e.g. not virtual) properties on the lowest object in the scope
    /// chain. As a result, this function always force sets a property on the
    /// local object and does not traverse the scope chain.
    pub fn define(&self, name: &str, value: impl Into<Value<'gc>>, mc: MutationContext<'gc, '_>) {
        self.locals()
            .define_value(mc, name, value.into(), EnumSet::empty());
    }

    /// Delete a value from scope
    pub fn delete(
        &self,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
        name: &str,
        mc: MutationContext<'gc, '_>,
    ) -> bool {
        if self.locals().has_property(avm, context, name) {
            return self.locals().delete(avm, mc, name);
        }

        if let Some(scope) = self.parent() {
            return scope.delete(avm, context, name, mc);
        }

        false
    }
}
