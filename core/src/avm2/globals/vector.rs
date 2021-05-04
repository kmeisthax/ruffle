//! `Vector` builtin/prototype

use crate::avm2::activation::Activation;
use crate::avm2::class::{Class, ClassAttributes};
use crate::avm2::globals::array::ArrayIter;
use crate::avm2::globals::NS_VECTOR;
use crate::avm2::method::Method;
use crate::avm2::names::{Namespace, QName};
use crate::avm2::object::{Object, TObject, VectorObject};
use crate::avm2::scope::Scope;
use crate::avm2::string::AvmString;
use crate::avm2::traits::Trait;
use crate::avm2::value::Value;
use crate::avm2::vector::VectorStorage;
use crate::avm2::Error;
use gc_arena::{GcCell, MutationContext};

/// Implements `Vector`'s instance constructor.
pub fn instance_init<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        activation.super_init(this, &[])?;

        if let Some(mut vector) = this.as_vector_storage_mut(activation.context.gc_context) {
            let length = args
                .get(0)
                .cloned()
                .unwrap_or(Value::Unsigned(0))
                .coerce_to_u32(activation)? as usize;
            let is_fixed = args
                .get(1)
                .cloned()
                .unwrap_or(Value::Bool(false))
                .coerce_to_boolean();

            vector.resize(length)?;
            vector.set_is_fixed(is_fixed);
        }
    }

    Ok(Value::Undefined)
}

/// Implements `Vector`'s class constructor.
pub fn class_init<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    _this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    Ok(Value::Undefined)
}

/// `Vector.length` getter
pub fn length<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        if let Some(vector) = this.as_vector_storage() {
            return Ok(vector.length().into());
        }
    }

    Ok(Value::Undefined)
}

/// `Vector.length` setter
pub fn set_length<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        if let Some(mut vector) = this.as_vector_storage_mut(activation.context.gc_context) {
            let new_length = args
                .get(0)
                .cloned()
                .unwrap_or(Value::Unsigned(0))
                .coerce_to_u32(activation)? as usize;

            vector.resize(new_length)?;
        }
    }

    Ok(Value::Undefined)
}

/// `Vector.fixed` getter
pub fn fixed<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        if let Some(vector) = this.as_vector_storage() {
            return Ok(vector.is_fixed().into());
        }
    }

    Ok(Value::Undefined)
}

/// `Vector.fixed` setter
pub fn set_fixed<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        if let Some(mut vector) = this.as_vector_storage_mut(activation.context.gc_context) {
            let new_fixed = args
                .get(0)
                .cloned()
                .unwrap_or(Value::Bool(false))
                .coerce_to_boolean();

            vector.set_is_fixed(new_fixed);
        }
    }

    Ok(Value::Undefined)
}

/// `Vector.concat` impl
pub fn concat<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        let mut new_vector_storage = if let Some(vector) = this.as_vector_storage() {
            vector.clone()
        } else {
            return Err("Not a vector-structured object".into());
        };

        let my_proto = this
            .proto()
            .ok_or("TypeError: Tried to concat into a bare object")?;
        let my_class = this
            .as_proto_class()
            .ok_or("TypeError: Tried to concat into a bare object")?;
        let my_param = new_vector_storage.value_proto();

        for arg in args.iter().map(|a| a.clone()) {
            let arg_obj = arg.coerce_to_object(activation)?;
            let arg_class = arg_obj
                .as_proto_class()
                .ok_or("TypeError: Tried to concat from a bare object")?;
            if !arg_obj.is_coercible_to(my_proto)? {
                return Err(format!(
                    "TypeError: Cannot coerce argument of type {:?} to argument of type {:?}",
                    arg_class.read().name(),
                    my_class.read().name()
                )
                .into());
            }

            let old_vec = arg_obj.as_vector_storage();
            let old_vec: Vec<Option<Value<'gc>>> = if let Some(old_vec) = old_vec {
                old_vec.iter().collect()
            } else {
                continue;
            };

            for val in old_vec {
                if let Some(val) = val {
                    let coerced_val = VectorStorage::coerce(val, my_param, activation)?;
                    new_vector_storage.push(coerced_val);
                } else {
                    new_vector_storage.push(None);
                }
            }
        }

        let vector_proto = activation.context.avm2.prototypes().vector;
        return Ok(VectorObject::from_vector(
            new_vector_storage,
            vector_proto,
            activation.context.gc_context,
        )
        .into());
    }

    Ok(Value::Undefined)
}

fn join_inner<'gc, 'a, 'ctxt, C>(
    activation: &mut Activation<'a, 'gc, 'ctxt>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
    mut conv: C,
) -> Result<Value<'gc>, Error>
where
    C: for<'b> FnMut(Value<'gc>, &'b mut Activation<'a, 'gc, 'ctxt>) -> Result<Value<'gc>, Error>,
{
    let mut separator = args.get(0).cloned().unwrap_or(Value::Undefined);
    if separator == Value::Undefined {
        separator = ",".into();
    }

    if let Some(this) = this {
        if let Some(vector) = this.as_vector_storage() {
            let string_separator = separator.coerce_to_string(activation)?;
            let mut accum = Vec::with_capacity(vector.length());

            for (_, item) in vector.iter().enumerate() {
                if matches!(item, Some(Value::Undefined))
                    || matches!(item, Some(Value::Null))
                    || item.is_none()
                {
                    accum.push("".into());
                } else {
                    accum.push(
                        conv(item.unwrap(), activation)?
                            .coerce_to_string(activation)?
                            .to_string(),
                    );
                }
            }

            return Ok(AvmString::new(
                activation.context.gc_context,
                accum.join(&string_separator),
            )
            .into());
        }
    }

    Ok(Value::Undefined)
}

/// Implements `Vector.join`
pub fn join<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    join_inner(activation, this, args, |v, _act| Ok(v))
}

/// Implements `Vector.toString`
pub fn to_string<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    join_inner(activation, this, &[",".into()], |v, _act| Ok(v))
}

/// Implements `Vector.every`
pub fn every<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        let callback = args
            .get(0)
            .cloned()
            .unwrap_or(Value::Undefined)
            .coerce_to_object(activation)?;
        let receiver = args
            .get(1)
            .cloned()
            .unwrap_or(Value::Null)
            .coerce_to_object(activation)
            .ok();
        let mut iter = ArrayIter::new(activation, this)?;

        while let Some(r) = iter.next(activation) {
            let (i, item) = r?;

            let result = callback
                .call(
                    receiver,
                    &[item, i.into(), this.into()],
                    activation,
                    receiver.and_then(|r| r.proto()),
                )?
                .coerce_to_boolean();

            if !result {
                return Ok(false.into());
            }
        }

        return Ok(true.into());
    }

    Ok(Value::Undefined)
}

/// Implements `Vector.some`
pub fn some<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        let callback = args
            .get(0)
            .cloned()
            .unwrap_or(Value::Undefined)
            .coerce_to_object(activation)?;
        let receiver = args
            .get(1)
            .cloned()
            .unwrap_or(Value::Null)
            .coerce_to_object(activation)
            .ok();
        let mut iter = ArrayIter::new(activation, this)?;

        while let Some(r) = iter.next(activation) {
            let (i, item) = r?;

            let result = callback
                .call(
                    receiver,
                    &[item, i.into(), this.into()],
                    activation,
                    receiver.and_then(|r| r.proto()),
                )?
                .coerce_to_boolean();

            if result {
                return Ok(true.into());
            }
        }

        return Ok(false.into());
    }

    Ok(Value::Undefined)
}

/// Implements `Vector.forEach`
pub fn for_each<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        let callback = args
            .get(0)
            .cloned()
            .unwrap_or(Value::Undefined)
            .coerce_to_object(activation)?;
        let receiver = args
            .get(1)
            .cloned()
            .unwrap_or(Value::Null)
            .coerce_to_object(activation)
            .ok();
        let mut iter = ArrayIter::new(activation, this)?;

        while let Some(r) = iter.next(activation) {
            let (i, item) = r?;

            callback.call(
                receiver,
                &[item, i.into(), this.into()],
                activation,
                receiver.and_then(|r| r.proto()),
            )?;
        }
    }

    Ok(Value::Undefined)
}

/// Vector deriver
pub fn vector_deriver<'gc>(
    base_proto: Object<'gc>,
    activation: &mut Activation<'_, 'gc, '_>,
    class: GcCell<'gc, Class<'gc>>,
    scope: Option<GcCell<'gc, Scope<'gc>>>,
) -> Result<Object<'gc>, Error> {
    VectorObject::derive(base_proto, activation.context.gc_context, class, scope)
}

/// Construct `Sprite`'s class.
pub fn create_class<'gc>(mc: MutationContext<'gc, '_>) -> GcCell<'gc, Class<'gc>> {
    let class = Class::new(
        QName::new(Namespace::package(NS_VECTOR), "Vector"),
        Some(QName::new(Namespace::public(), "Object").into()),
        Method::from_builtin(instance_init),
        Method::from_builtin(class_init),
        mc,
    );

    let mut write = class.write(mc);

    write.set_attributes(ClassAttributes::GENERIC | ClassAttributes::FINAL);

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "length"),
        Method::from_builtin(length),
    ));
    write.define_instance_trait(Trait::from_setter(
        QName::new(Namespace::public(), "length"),
        Method::from_builtin(set_length),
    ));
    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "fixed"),
        Method::from_builtin(fixed),
    ));
    write.define_instance_trait(Trait::from_setter(
        QName::new(Namespace::public(), "fixed"),
        Method::from_builtin(set_fixed),
    ));
    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "concat"),
        Method::from_builtin(concat),
    ));
    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "join"),
        Method::from_builtin(join),
    ));
    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "every"),
        Method::from_builtin(every),
    ));
    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "some"),
        Method::from_builtin(some),
    ));
    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "forEach"),
        Method::from_builtin(for_each),
    ));
    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "toString"),
        Method::from_builtin(to_string),
    ));

    class
}
