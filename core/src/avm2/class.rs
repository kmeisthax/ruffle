//! AVM2 classes

use crate::avm2::activation::Activation;
use crate::avm2::method::{Method, NativeMethodImpl};
use crate::avm2::names::{Multiname, Namespace, QName};
use crate::avm2::object::{Object, ScriptObject, TObject};
use crate::avm2::script::TranslationUnit;
use crate::avm2::string::AvmString;
use crate::avm2::traits::{Trait, TraitKind};
use crate::avm2::value::Value;
use crate::avm2::Error;
use bitflags::bitflags;
use gc_arena::{Collect, GcCell, MutationContext};
use std::fmt;
use swf::avm2::types::{
    Class as AbcClass, Instance as AbcInstance, Method as AbcMethod, MethodBody as AbcMethodBody,
};

bitflags! {
    /// All possible attributes for a given class.
    pub struct ClassAttributes: u8 {
        /// Class is sealed, attempts to set or init dynamic properties on an
        /// object will generate a runtime error.
        const SEALED    = 1 << 0;

        /// Class is final, attempts to construct child classes from it will
        /// generate a verification error.
        const FINAL     = 1 << 1;

        /// Class is an interface.
        const INTERFACE = 1 << 2;
    }
}

/// A function that can be used to allocate instances of a class.
///
/// By default, the `implicit_deriver` is used, which attempts to use the base
/// class's deriver, and defaults to `ScriptObject` otherwise. Custom derivers
/// anywhere in the class inheritance chain can change the representation of
/// all subtypes that use the implicit deriver.
///
/// Parameters for the deriver are:
///
///  * `constr` - The class constructor that was called (or will be called) to
///  construct this object. This must be the current class (using a base class
///  will cause the wrong class to be read for traits).
///  * `proto` - The prototype attached to the class constructor.
///  * `activation` - This is the current AVM2 activation.
pub type DeriverFn = for<'gc> fn(
    Object<'gc>,
    Object<'gc>,
    &mut Activation<'_, 'gc, '_>,
) -> Result<Object<'gc>, Error>;

#[derive(Clone, Collect)]
#[collect(require_static)]
pub struct Deriver(pub DeriverFn);

impl fmt::Debug for Deriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Deriver")
            .field(&"<native code>".to_string())
            .finish()
    }
}

/// The implicit deriver for new classes.
///
/// This attempts to use the parent type's deriver, and if such a deriver does
/// not exist, we default to `ScriptObject`.
pub fn implicit_deriver<'gc>(
    mut constr: Object<'gc>,
    proto: Object<'gc>,
    activation: &mut Activation<'_, 'gc, '_>,
) -> Result<Object<'gc>, Error> {
    let mut base_constr = Some(constr);
    let mut base_class = constr.as_class();
    let mut instance_deriver = None;

    while let (Some(b_constr), Some(b_class)) = (base_constr, base_class) {
        let base_deriver = b_class.read().instance_deriver();

        if base_deriver as usize != implicit_deriver as usize {
            instance_deriver = Some(base_deriver);
            break;
        }

        base_constr = b_constr.base_class_constr();
        base_class = base_constr.and_then(|c| c.as_class());
    }

    if let Some(base_deriver) = instance_deriver {
        base_deriver(constr, proto, activation)
    } else {
        let base_proto = constr
            .get_property(
                constr,
                &QName::new(Namespace::public(), "prototype"),
                activation,
            )?
            .coerce_to_object(activation)?;

        Ok(ScriptObject::instance(
            activation.context.gc_context,
            constr,
            base_proto,
        ))
    }
}

/// A loaded ABC Class which can be used to construct objects with.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub struct Class<'gc> {
    /// The name of the class.
    name: QName<'gc>,

    /// The name of this class's superclass.
    super_class: Option<Multiname<'gc>>,

    /// Attributes of the given class.
    #[collect(require_static)]
    attributes: ClassAttributes,

    /// The namespace that protected traits of this class are stored into.
    protected_namespace: Option<Namespace<'gc>>,

    /// The list of interfaces this class implements.
    interfaces: Vec<Multiname<'gc>>,

    /// The instance deriver for this class.
    instance_deriver: Deriver,

    /// The instance initializer for this class.
    ///
    /// Must be called each time a new class instance is constructed.
    instance_init: Method<'gc>,

    /// The native instance initializer for this class.
    ///
    /// This may be provided to allow natively-constructed classes to
    /// initialize themselves in a different manner from user-constructed ones.
    /// For example, the user-accessible constructor may error out (as it's not
    /// a valid class to construct for users), but native code may still call
    /// it's constructor stack.
    ///
    /// By default, a class's `native_instance_init` will be initialized to the
    /// same method as the regular one. You must specify a separate native
    /// initializer to change initialization behavior based on what code is
    /// constructing the class.
    native_instance_init: Method<'gc>,

    /// Instance traits for a given class.
    ///
    /// These are accessed as normal instance properties; they should not be
    /// present on prototypes, but instead should shadow any prototype
    /// properties that would match.
    instance_traits: Vec<Trait<'gc>>,

    /// The class initializer for this class.
    ///
    /// Must be called once and only once prior to any use of this class.
    class_init: Method<'gc>,

    /// Whether or not the class initializer has already been called.
    class_initializer_called: bool,

    /// Static traits for a given class.
    ///
    /// These are accessed as constructor properties.
    class_traits: Vec<Trait<'gc>>,

    /// Whether or not this `Class` has loaded its traits or not.
    traits_loaded: bool,
}

/// Find traits in a list of traits matching a name.
///
/// This function also enforces final/override bits on the traits, and will
/// raise `VerifyError`s as needed.
///
/// TODO: This is an O(n^2) algorithm, it sucks.
fn do_trait_lookup<'gc>(
    name: &QName<'gc>,
    known_traits: &mut Vec<Trait<'gc>>,
    all_traits: &[Trait<'gc>],
) -> Result<(), Error> {
    for trait_entry in all_traits {
        if name == trait_entry.name() {
            for known_trait in known_traits.iter() {
                match (&trait_entry.kind(), &known_trait.kind()) {
                    (TraitKind::Getter { .. }, TraitKind::Setter { .. }) => continue,
                    (TraitKind::Setter { .. }, TraitKind::Getter { .. }) => continue,
                    _ => {}
                };

                if known_trait.is_final() {
                    return Err("Attempting to override a final definition".into());
                }

                if !trait_entry.is_override() {
                    return Err("Definition override is not marked as override".into());
                }
            }

            known_traits.push(trait_entry.clone());
        }
    }

    Ok(())
}

/// Find traits in a list of traits matching a slot ID.
fn do_trait_lookup_by_slot<'gc>(
    id: u32,
    all_traits: &[Trait<'gc>],
) -> Result<Option<Trait<'gc>>, Error> {
    for trait_entry in all_traits {
        let trait_id = match trait_entry.kind() {
            TraitKind::Slot { slot_id, .. } => slot_id,
            TraitKind::Const { slot_id, .. } => slot_id,
            TraitKind::Class { slot_id, .. } => slot_id,
            TraitKind::Function { slot_id, .. } => slot_id,
            _ => continue,
        };

        if id == *trait_id {
            return Ok(Some(trait_entry.clone()));
        }
    }

    Ok(None)
}

impl<'gc> Class<'gc> {
    /// Create a new class.
    ///
    /// This function is primarily intended for use by native code to define
    /// builtin classes. The absolute minimum necessary to define a class is
    /// required here; further methods allow further changes to the class.
    ///
    /// Classes created in this way cannot have traits loaded from an ABC file
    /// using `load_traits`.
    pub fn new(
        name: QName<'gc>,
        super_class: Option<Multiname<'gc>>,
        instance_init: Method<'gc>,
        class_init: Method<'gc>,
        mc: MutationContext<'gc, '_>,
    ) -> GcCell<'gc, Self> {
        let native_instance_init = instance_init.clone();

        GcCell::allocate(
            mc,
            Self {
                name,
                super_class,
                attributes: ClassAttributes::empty(),
                protected_namespace: None,
                interfaces: Vec::new(),
                instance_deriver: Deriver(implicit_deriver),
                instance_init,
                native_instance_init,
                instance_traits: Vec::new(),
                class_init,
                class_initializer_called: false,
                class_traits: Vec::new(),
                traits_loaded: true,
            },
        )
    }

    /// Set the attributes of the class (sealed/final/interface status).
    pub fn set_attributes(&mut self, attributes: ClassAttributes) {
        self.attributes = attributes;
    }

    /// Add a protected namespace to this class.
    pub fn set_protected_namespace(&mut self, ns: Namespace<'gc>) {
        self.protected_namespace = Some(ns)
    }

    /// Construct a class from a `TranslationUnit` and its class index.
    ///
    /// The returned class will be allocated, but no traits will be loaded. The
    /// caller is responsible for storing the class in the `TranslationUnit`
    /// and calling `load_traits` to complete the trait-loading process.
    pub fn from_abc_index(
        unit: TranslationUnit<'gc>,
        class_index: u32,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<GcCell<'gc, Self>, Error> {
        let abc = unit.abc();
        let abc_class: Result<&AbcClass, Error> = abc
            .classes
            .get(class_index as usize)
            .ok_or_else(|| "LoadError: Class index not valid".into());
        let abc_class = abc_class?;

        let abc_instance: Result<&AbcInstance, Error> = abc
            .instances
            .get(class_index as usize)
            .ok_or_else(|| "LoadError: Instance index not valid".into());
        let abc_instance = abc_instance?;

        let name = QName::from_abc_multiname(
            unit,
            abc_instance.name.clone(),
            activation.context.gc_context,
        )?;
        let super_class = if abc_instance.super_name.0 == 0 {
            None
        } else {
            Some(Multiname::from_abc_multiname_static(
                unit,
                abc_instance.super_name.clone(),
                activation.context.gc_context,
            )?)
        };

        let protected_namespace = if let Some(ns) = &abc_instance.protected_namespace {
            Some(Namespace::from_abc_namespace(
                unit,
                ns.clone(),
                activation.context.gc_context,
            )?)
        } else {
            None
        };

        let mut interfaces = Vec::new();
        for interface_name in abc_instance.interfaces.iter() {
            interfaces.push(Multiname::from_abc_multiname_static(
                unit,
                interface_name.clone(),
                activation.context.gc_context,
            )?);
        }

        let instance_init = unit.load_method(abc_instance.init_method.0, activation)?;
        let native_instance_init = instance_init.clone();
        let class_init = unit.load_method(abc_class.init_method.0, activation)?;

        let mut attributes = ClassAttributes::empty();
        attributes.set(ClassAttributes::SEALED, abc_instance.is_sealed);
        attributes.set(ClassAttributes::FINAL, abc_instance.is_final);
        attributes.set(ClassAttributes::INTERFACE, abc_instance.is_interface);

        Ok(GcCell::allocate(
            activation.context.gc_context,
            Self {
                name,
                super_class,
                attributes,
                protected_namespace,
                interfaces,
                instance_deriver: Deriver(implicit_deriver),
                instance_init,
                native_instance_init,
                instance_traits: Vec::new(),
                class_init,
                class_initializer_called: false,
                class_traits: Vec::new(),
                traits_loaded: false,
            },
        ))
    }

    /// Finish the class-loading process by loading traits.
    ///
    /// This process must be done after the `Class` has been stored in the
    /// `TranslationUnit`. Failing to do so runs the risk of runaway recursion
    /// or double-borrows. It should be done before the class is actually
    /// instantiated into an `Object`.
    pub fn load_traits(
        &mut self,
        unit: TranslationUnit<'gc>,
        class_index: u32,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<(), Error> {
        if self.traits_loaded {
            return Ok(());
        }

        self.traits_loaded = true;

        let abc = unit.abc();
        let abc_class: Result<&AbcClass, Error> = abc
            .classes
            .get(class_index as usize)
            .ok_or_else(|| "LoadError: Class index not valid".into());
        let abc_class = abc_class?;

        let abc_instance: Result<&AbcInstance, Error> = abc
            .instances
            .get(class_index as usize)
            .ok_or_else(|| "LoadError: Instance index not valid".into());
        let abc_instance = abc_instance?;

        for abc_trait in abc_instance.traits.iter() {
            self.instance_traits
                .push(Trait::from_abc_trait(unit, abc_trait, activation)?);
        }

        for abc_trait in abc_class.traits.iter() {
            self.class_traits
                .push(Trait::from_abc_trait(unit, abc_trait, activation)?);
        }

        Ok(())
    }

    pub fn for_activation_constr(
        activation: &mut Activation<'_, 'gc, '_>,
        translation_unit: TranslationUnit<'gc>,
        method: &AbcMethod,
        body: &AbcMethodBody,
    ) -> Result<GcCell<'gc, Self>, Error> {
        let name =
            translation_unit.pool_string(method.name.as_u30(), activation.context.gc_context)?;
        let mut traits = Vec::new();

        for trait_entry in body.traits.iter() {
            traits.push(Trait::from_abc_trait(
                translation_unit,
                trait_entry,
                activation,
            )?);
        }

        Ok(GcCell::allocate(
            activation.context.gc_context,
            Self {
                name: QName::dynamic_name(name),
                super_class: None,
                attributes: ClassAttributes::empty(),
                protected_namespace: None,
                interfaces: Vec::new(),
                instance_deriver: Deriver(implicit_deriver),
                instance_init: Method::from_builtin_only(
                    |_, _, _| Ok(Value::Undefined),
                    "<Activation object constructor>",
                    activation.context.gc_context,
                ),
                native_instance_init: Method::from_builtin_only(
                    |_, _, _| Ok(Value::Undefined),
                    "<Activation object constructor>",
                    activation.context.gc_context,
                ),
                instance_traits: traits,
                class_init: Method::from_builtin_only(
                    |_, _, _| Ok(Value::Undefined),
                    "<Activation object class constructor>",
                    activation.context.gc_context,
                ),
                class_initializer_called: false,
                class_traits: Vec::new(),
                traits_loaded: true,
            },
        ))
    }

    pub fn name(&self) -> &QName<'gc> {
        &self.name
    }

    pub fn super_class_name(&self) -> &Option<Multiname<'gc>> {
        &self.super_class
    }

    #[inline(never)]
    pub fn define_public_constant_string_class_traits(
        &mut self,
        items: &[(&'static str, &'static str)],
    ) {
        for &(name, value) in items {
            self.define_class_trait(Trait::from_const(
                QName::new(Namespace::public(), name),
                QName::new(Namespace::public(), "String").into(),
                Some(value.into()),
            ));
        }
    }
    #[inline(never)]
    pub fn define_public_constant_number_class_traits(&mut self, items: &[(&'static str, f64)]) {
        for &(name, value) in items {
            self.define_class_trait(Trait::from_const(
                QName::new(Namespace::public(), name),
                QName::new(Namespace::public(), "Number").into(),
                Some(value.into()),
            ));
        }
    }
    #[inline(never)]
    pub fn define_public_constant_uint_class_traits(&mut self, items: &[(&'static str, u32)]) {
        for &(name, value) in items {
            self.define_class_trait(Trait::from_const(
                QName::new(Namespace::public(), name),
                QName::new(Namespace::public(), "uint").into(),
                Some(value.into()),
            ));
        }
    }
    #[inline(never)]
    pub fn define_public_builtin_instance_methods(
        &mut self,
        mc: MutationContext<'gc, '_>,
        items: &[(&'static str, NativeMethodImpl)],
    ) {
        for &(name, value) in items {
            self.define_instance_trait(Trait::from_method(
                QName::new(Namespace::public(), name),
                Method::from_builtin_only(value, name, mc),
            ));
        }
    }
    #[inline(never)]
    pub fn define_as3_builtin_instance_methods(
        &mut self,
        mc: MutationContext<'gc, '_>,
        items: &[(&'static str, NativeMethodImpl)],
    ) {
        for &(name, value) in items {
            self.define_instance_trait(Trait::from_method(
                QName::new(Namespace::as3_namespace(), name),
                Method::from_builtin_only(value, name, mc),
            ));
        }
    }
    #[inline(never)]
    pub fn define_public_builtin_class_methods(
        &mut self,
        mc: MutationContext<'gc, '_>,
        items: &[(&'static str, NativeMethodImpl)],
    ) {
        for &(name, value) in items {
            self.define_class_trait(Trait::from_method(
                QName::new(Namespace::public(), name),
                Method::from_builtin_only(value, name, mc),
            ));
        }
    }
    #[inline(never)]
    pub fn define_public_builtin_instance_properties(
        &mut self,
        mc: MutationContext<'gc, '_>,
        items: &[(
            &'static str,
            Option<NativeMethodImpl>,
            Option<NativeMethodImpl>,
        )],
    ) {
        for &(name, getter, setter) in items {
            if let Some(getter) = getter {
                self.define_instance_trait(Trait::from_getter(
                    QName::new(Namespace::public(), name),
                    Method::from_builtin_only(getter, name, mc),
                ));
            }
            if let Some(setter) = setter {
                self.define_instance_trait(Trait::from_setter(
                    QName::new(Namespace::public(), name),
                    Method::from_builtin_only(setter, name, mc),
                ));
            }
        }
    }

    /// Define a trait on the class.
    ///
    /// Class traits will be accessible as properties on the class constructor
    /// function.
    pub fn define_class_trait(&mut self, my_trait: Trait<'gc>) {
        self.class_traits.push(my_trait);
    }

    /// Given a name, append class traits matching the name to a list of known
    /// traits.
    ///
    /// This function adds its result onto the list of known traits, with the
    /// caveat that duplicate entries will be replaced (if allowed). As such, this
    /// function should be run on the class hierarchy from top to bottom.
    ///
    /// If a given trait has an invalid name, attempts to override a final trait,
    /// or overlaps an existing trait without being an override, then this function
    /// returns an error.
    pub fn lookup_class_traits(
        &self,
        name: &QName<'gc>,
        known_traits: &mut Vec<Trait<'gc>>,
    ) -> Result<(), Error> {
        do_trait_lookup(name, known_traits, &self.class_traits)
    }

    /// Given a slot ID, append class traits matching the slot to a list of
    /// known traits.
    ///
    /// This function adds its result onto the list of known traits, with the
    /// caveat that duplicate entries will be replaced (if allowed). As such, this
    /// function should be run on the class hierarchy from top to bottom.
    ///
    /// If a given trait has an invalid name, attempts to override a final trait,
    /// or overlaps an existing trait without being an override, then this function
    /// returns an error.
    pub fn lookup_class_traits_by_slot(&self, id: u32) -> Result<Option<Trait<'gc>>, Error> {
        do_trait_lookup_by_slot(id, &self.class_traits)
    }

    /// Determines if this class provides a given trait on itself.
    pub fn has_class_trait(&self, name: &QName<'gc>) -> bool {
        for trait_entry in self.class_traits.iter() {
            if name == trait_entry.name() {
                return true;
            }
        }

        false
    }

    /// Return class traits provided by this class.
    pub fn class_traits(&self) -> &[Trait<'gc>] {
        &self.class_traits[..]
    }

    /// Look for a class trait with a given local name, and return its
    /// namespace.
    ///
    /// TODO: Matching multiple namespaces with the same local name is at least
    /// claimed by the AVM2 specification to be a `VerifyError`.
    pub fn resolve_any_class_trait(&self, local_name: AvmString<'gc>) -> Option<Namespace<'gc>> {
        for trait_entry in self.class_traits.iter() {
            if local_name == trait_entry.name().local_name() {
                return Some(trait_entry.name().namespace().clone());
            }
        }

        None
    }

    /// Define a trait on instances of the class.
    ///
    /// Instance traits will be accessible as properties on instances of the
    /// class. They will not be accessible on the class prototype, and any
    /// properties defined on the prototype will be shadowed by these traits.
    pub fn define_instance_trait(&mut self, my_trait: Trait<'gc>) {
        self.instance_traits.push(my_trait);
    }

    /// Given a name, append instance traits matching the name to a list of
    /// known traits.
    ///
    /// This function adds its result onto the list of known traits, with the
    /// caveat that duplicate entries will be replaced (if allowed). As such, this
    /// function should be run on the class hierarchy from top to bottom.
    ///
    /// If a given trait has an invalid name, attempts to override a final trait,
    /// or overlaps an existing trait without being an override, then this function
    /// returns an error.
    pub fn lookup_instance_traits(
        &self,
        name: &QName<'gc>,
        known_traits: &mut Vec<Trait<'gc>>,
    ) -> Result<(), Error> {
        do_trait_lookup(name, known_traits, &self.instance_traits)
    }

    /// Given a slot ID, append instance traits matching the slot to a list of
    /// known traits.
    ///
    /// This function adds its result onto the list of known traits, with the
    /// caveat that duplicate entries will be replaced (if allowed). As such, this
    /// function should be run on the class hierarchy from top to bottom.
    ///
    /// If a given trait has an invalid name, attempts to override a final trait,
    /// or overlaps an existing trait without being an override, then this function
    /// returns an error.
    pub fn lookup_instance_traits_by_slot(&self, id: u32) -> Result<Option<Trait<'gc>>, Error> {
        do_trait_lookup_by_slot(id, &self.instance_traits)
    }

    /// Determines if this class provides a given trait on its instances.
    pub fn has_instance_trait(&self, name: &QName<'gc>) -> bool {
        for trait_entry in self.instance_traits.iter() {
            if name == trait_entry.name() {
                return true;
            }
        }

        false
    }

    /// Return instance traits provided by this class.
    pub fn instance_traits(&self) -> &[Trait<'gc>] {
        &self.instance_traits[..]
    }

    /// Look for an instance trait with a given local name, and return its
    /// namespace.
    ///
    /// TODO: Matching multiple namespaces with the same local name is at least
    /// claimed by the AVM2 specification to be a `VerifyError`.
    pub fn resolve_any_instance_trait(&self, local_name: AvmString<'gc>) -> Option<Namespace<'gc>> {
        for trait_entry in self.instance_traits.iter() {
            if local_name == trait_entry.name().local_name() {
                return Some(trait_entry.name().namespace().clone());
            }
        }

        None
    }

    /// Get this class's instance deriver.
    pub fn instance_deriver(&self) -> DeriverFn {
        self.instance_deriver.0
    }

    /// Set this class's instance deriver.
    pub fn set_instance_deriver(&mut self, deriver: DeriverFn) {
        self.instance_deriver.0 = deriver;
    }

    /// Get this class's instance initializer.
    pub fn instance_init(&self) -> Method<'gc> {
        self.instance_init.clone()
    }

    /// Get this class's native-code instance initializer.
    pub fn native_instance_init(&self) -> Method<'gc> {
        self.native_instance_init.clone()
    }

    /// Set a native-code instance initializer for this class.
    pub fn set_native_instance_init(&mut self, new_native_init: Method<'gc>) {
        self.native_instance_init = new_native_init;
    }

    /// Get this class's class initializer.
    pub fn class_init(&self) -> Method<'gc> {
        self.class_init.clone()
    }

    /// Check if the class has already been initialized.
    pub fn is_class_initialized(&self) -> bool {
        self.class_initializer_called
    }

    /// Mark the class as initialized.
    pub fn mark_class_initialized(&mut self) {
        self.class_initializer_called = true;
    }

    pub fn interfaces(&self) -> &[Multiname<'gc>] {
        &self.interfaces
    }

    pub fn implements(&mut self, iface: Multiname<'gc>) {
        self.interfaces.push(iface)
    }

    /// Determine if this class is sealed (no dynamic properties)
    pub fn is_sealed(&self) -> bool {
        self.attributes.contains(ClassAttributes::SEALED)
    }

    /// Determine if this class is final (cannot be subclassed)
    pub fn is_final(&self) -> bool {
        self.attributes.contains(ClassAttributes::FINAL)
    }

    /// Determine if this class is an interface
    pub fn is_interface(&self) -> bool {
        self.attributes.contains(ClassAttributes::INTERFACE)
    }
}
