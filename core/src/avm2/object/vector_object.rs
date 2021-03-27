//! Vector storage object

use crate::avm2::activation::Activation;
use crate::avm2::class::Class;
use crate::avm2::names::{Namespace, QName};
use crate::avm2::object::script_object::{ScriptObjectClass, ScriptObjectData};
use crate::avm2::object::{Object, ObjectPtr, TObject};
use crate::avm2::scope::Scope;
use crate::avm2::string::AvmString;
use crate::avm2::traits::Trait;
use crate::avm2::value::Value;
use crate::avm2::vector::VectorStorage;
use crate::avm2::Error;
use crate::impl_avm2_custom_object;
use gc_arena::{Collect, GcCell, MutationContext};
use std::cell::{Ref, RefMut};

/// An Object which stores typed properties in vector storage
#[derive(Collect, Debug, Clone, Copy)]
#[collect(no_drop)]
pub struct VectorObject<'gc>(GcCell<'gc, VectorObjectData<'gc>>);

#[derive(Collect, Debug, Clone)]
#[collect(no_drop)]
pub struct VectorObjectData<'gc> {
    /// Base script object
    base: ScriptObjectData<'gc>,

    /// Vector-structured properties
    vector: VectorStorage<'gc>,
}

impl<'gc> VectorObject<'gc> {
    pub fn derive(
        base_proto: Object<'gc>,
        mc: MutationContext<'gc, '_>,
        class: GcCell<'gc, Class<'gc>>,
        scope: Option<GcCell<'gc, Scope<'gc>>>,
    ) -> Result<Object<'gc>, Error> {
        let base = ScriptObjectData::base_new(
            Some(base_proto),
            ScriptObjectClass::InstancePrototype(class, scope),
        );

        Ok(VectorObject(GcCell::allocate(
            mc,
            VectorObjectData {
                base,
                vector: VectorStorage::new(0, false, class),
            },
        ))
        .into())
    }
}

impl<'gc> TObject<'gc> for VectorObject<'gc> {
    impl_avm2_custom_object!(base);

    fn get_property_local(
        self,
        receiver: Object<'gc>,
        name: &QName<'gc>,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<Value<'gc>, Error> {
        let read = self.0.read();

        if name.namespace().is_package("") {
            if let Ok(index) = name.local_name().parse::<usize>() {
                return Ok(read.vector.get(index).unwrap_or(Value::Undefined));
            }
        }

        let rv = read.base.get_property_local(receiver, name, activation)?;

        drop(read);

        rv.resolve(activation)
    }

    fn set_property_local(
        self,
        receiver: Object<'gc>,
        name: &QName<'gc>,
        value: Value<'gc>,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<(), Error> {
        if name.namespace().is_package("") {
            if let Ok(index) = name.local_name().parse::<usize>() {
                let type_of = self.0.read().vector.value_type();
                let value = VectorStorage::coerce(value, type_of, activation)?;

                self.0
                    .write(activation.context.gc_context)
                    .vector
                    .set(index, value)?;

                return Ok(());
            }
        }

        let mut write = self.0.write(activation.context.gc_context);

        let rv = write
            .base
            .set_property_local(receiver, name, value, activation)?;

        drop(write);

        rv.resolve(activation)?;

        Ok(())
    }

    fn init_property_local(
        self,
        receiver: Object<'gc>,
        name: &QName<'gc>,
        value: Value<'gc>,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<(), Error> {
        if name.namespace().is_package("") {
            if let Ok(index) = name.local_name().parse::<usize>() {
                let type_of = self.0.read().vector.value_type();
                let value = VectorStorage::coerce(value, type_of, activation)?;

                self.0
                    .write(activation.context.gc_context)
                    .vector
                    .set(index, value)?;

                return Ok(());
            }
        }

        let mut write = self.0.write(activation.context.gc_context);

        let rv = write
            .base
            .init_property_local(receiver, name, value, activation)?;

        drop(write);

        rv.resolve(activation)?;

        Ok(())
    }

    fn is_property_overwritable(
        self,
        gc_context: MutationContext<'gc, '_>,
        name: &QName<'gc>,
    ) -> bool {
        self.0.write(gc_context).base.is_property_overwritable(name)
    }

    fn delete_property(&self, gc_context: MutationContext<'gc, '_>, name: &QName<'gc>) -> bool {
        if name.namespace().is_package("") && name.local_name().parse::<usize>().is_ok() {
            return true;
        }

        self.0.write(gc_context).base.delete_property(name)
    }

    fn has_own_property(self, name: &QName<'gc>) -> Result<bool, Error> {
        if name.namespace().is_package("") {
            if let Ok(index) = name.local_name().parse::<usize>() {
                return Ok(self.0.read().vector.get(index).is_ok());
            }
        }

        self.0.read().base.has_own_property(name)
    }

    fn resolve_any(self, local_name: AvmString<'gc>) -> Result<Option<Namespace<'gc>>, Error> {
        if let Ok(index) = local_name.parse::<usize>() {
            if self.0.read().vector.get(index).is_ok() {
                return Ok(Some(Namespace::package("")));
            }
        }

        self.0.read().base.resolve_any(local_name)
    }

    fn resolve_any_trait(
        self,
        local_name: AvmString<'gc>,
    ) -> Result<Option<Namespace<'gc>>, Error> {
        self.0.read().base.resolve_any_trait(local_name)
    }

    fn to_string(&self, _mc: MutationContext<'gc, '_>) -> Result<Value<'gc>, Error> {
        Ok(Value::Object(Object::from(*self)))
    }

    fn value_of(&self, _mc: MutationContext<'gc, '_>) -> Result<Value<'gc>, Error> {
        Ok(Value::Object(Object::from(*self)))
    }

    fn construct(
        &self,
        activation: &mut Activation<'_, 'gc, '_>,
        _args: &[Value<'gc>],
    ) -> Result<Object<'gc>, Error> {
        let class = self
            .as_class()
            .ok_or("Attempted to construct bare-object Vector")?;
        let vector_type = class
            .read()
            .params()
            .get(0)
            .copied()
            .ok_or("Attempted to construct Vector without type parameter!")?;
        let this: Object<'gc> = Object::VectorObject(*self);
        let base = ScriptObjectData::base_new(Some(this), ScriptObjectClass::NoClass);

        Ok(VectorObject(GcCell::allocate(
            activation.context.gc_context,
            VectorObjectData {
                base,
                vector: VectorStorage::new(0, false, vector_type),
            },
        ))
        .into())
    }

    fn derive(
        &self,
        activation: &mut Activation<'_, 'gc, '_>,
        class: GcCell<'gc, Class<'gc>>,
        scope: Option<GcCell<'gc, Scope<'gc>>>,
    ) -> Result<Object<'gc>, Error> {
        let this: Object<'gc> = Object::VectorObject(*self);
        let vector_type = class
            .read()
            .params()
            .get(0)
            .copied()
            .ok_or("Attempted to construct Vector without type parameter!")?;
        let base = ScriptObjectData::base_new(
            Some(this),
            ScriptObjectClass::InstancePrototype(class, scope),
        );

        Ok(VectorObject(GcCell::allocate(
            activation.context.gc_context,
            VectorObjectData {
                base,
                vector: VectorStorage::new(0, false, vector_type),
            },
        ))
        .into())
    }

    fn apply(
        &self,
        activation: &mut Activation<'_, 'gc, '_>,
        params: &[GcCell<'gc, Class<'gc>>],
    ) -> Result<Object<'gc>, Error> {
        if params.len() != 1 {
            return Err("Vector can only be parameterized with one type".into());
        }

        let param = params.get(0).cloned().unwrap();
        if let Some(o) = activation.context.avm2.vector_proto_of(param) {
            return Ok(o);
        }

        let self_class = self
            .as_class()
            .ok_or("Attempted to apply type arguments to non-class!")?;
        let parameterized_class = self_class
            .read()
            .with_type_params(params, activation.context.gc_context);

        let self_scope = self.get_scope();
        let self_proto = self
            .proto()
            .ok_or("Attempted to apply type arguments to bare object!")?;

        let concrete_proto = VectorObject::derive(
            self_proto,
            activation.context.gc_context,
            parameterized_class,
            self_scope,
        )?;

        activation
            .context
            .avm2
            .set_vector_proto_of(param, concrete_proto);

        Ok(concrete_proto)
    }

    fn as_vector_storage(&self) -> Option<Ref<VectorStorage<'gc>>> {
        Some(Ref::map(self.0.read(), |vod| &vod.vector))
    }

    fn as_vector_storage_mut(
        &self,
        mc: MutationContext<'gc, '_>,
    ) -> Option<RefMut<VectorStorage<'gc>>> {
        Some(RefMut::map(self.0.write(mc), |vod| &mut vod.vector))
    }
}
