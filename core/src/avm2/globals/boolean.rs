//! `Boolean` impl

use crate::avm2::activation::Activation;
use crate::avm2::class::Class;
use crate::avm2::method::Method;
use crate::avm2::names::{Namespace, QName};
use crate::avm2::object::{primitive_deriver, Object};
use crate::avm2::value::Value;
use crate::avm2::Error;
use gc_arena::{GcCell, MutationContext};

/// Implements `Boolean`'s instance initializer.
pub fn instance_init<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    _this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    Err("Boolean constructor is a stub.".into())
}

/// Implements `Boolean`'s native instance initializer.
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

/// Implements `Boolean`'s class initializer.
pub fn class_init<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    _this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    Ok(Value::Undefined)
}

/// Construct `Boolean`'s class.
pub fn create_class<'gc>(mc: MutationContext<'gc, '_>) -> GcCell<'gc, Class<'gc>> {
    let class = Class::new(
        QName::new(Namespace::public(), "Boolean"),
        Some(QName::new(Namespace::public(), "Object").into()),
        Method::from_builtin_only(instance_init, "<Boolean instance initializer>", mc),
        Method::from_builtin_only(class_init, "<Boolean class initializer>", mc),
        mc,
    );

    let mut write = class.write(mc);
    write.set_instance_deriver(primitive_deriver);
    write.set_native_instance_init(Method::from_builtin_only(
        native_instance_init,
        "<Boolean native instance initializer>",
        mc,
    ));

    class
}
