//! `uint` impl

use crate::avm2::activation::Activation;
use crate::avm2::class::Class;
use crate::avm2::method::{Method, ParamConfig};
use crate::avm2::names::{Namespace, QName};
use crate::avm2::object::{primitive_allocator, Object};
use crate::avm2::value::Value;
use crate::avm2::Error;
use gc_arena::{GcCell, MutationContext};

/// Implements `uint`'s instance initializer.
pub fn instance_init<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    _this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    Err("uint constructor is a stub.".into())
}

/// Implements `uint`'s native instance initializer.
pub fn native_instance_init<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        activation.super_init(this, args)?;
    }

    Ok(Value::Undefined)
}

/// Implements `uint`'s class initializer.
pub fn class_init<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    _this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    Ok(Value::Undefined)
}

/// Construct `uint`'s class.
pub fn create_class<'gc>(mc: MutationContext<'gc, '_>) -> GcCell<'gc, Class<'gc>> {
    let class = Class::new(
        QName::new(Namespace::public(), "uint"),
        Some(QName::new(Namespace::public(), "Object").into()),
        Method::from_builtin_and_params(
            instance_init,
            "<uint instance initializer>",
            vec![ParamConfig::of_type(
                "num",
                QName::new(Namespace::public(), "Object").into(),
            )],
            false,
            mc,
        ),
        Method::from_builtin(class_init, "<uint class initializer>", mc),
        mc,
    );

    let mut write = class.write(mc);
    write.set_instance_allocator(primitive_allocator);
    write.set_native_instance_init(Method::from_builtin_and_params(
        native_instance_init,
        "<uint native instance initializer>",
        vec![ParamConfig::of_type(
            "num",
            QName::new(Namespace::public(), "Object").into(),
        )],
        false,
        mc,
    ));

    class
}
