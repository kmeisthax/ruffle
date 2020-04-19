//! Button/SimpleButton prototype

use crate::avm1::function::{Executable, FunctionObject};
use crate::avm1::globals::display_object;
use crate::avm1::return_value::ReturnValue;
use crate::avm1::{Avm1, Error, Object, ScriptObject, UpdateContext, Value};
use gc_arena::MutationContext;

pub fn create_proto<'gc>(
    gc_context: MutationContext<'gc, '_>,
    proto: Object<'gc>,
    constr: Object<'gc>,
    fn_proto: Object<'gc>,
    fn_constr: Object<'gc>,
) -> (Object<'gc>, Object<'gc>) {
    let button_proto = ScriptObject::object(gc_context, Some(proto), Some(constr));

    display_object::define_display_object_proto(gc_context, button_proto, fn_proto);

    let button = FunctionObject::function(
        gc_context,
        Executable::Native(constructor),
        Some(fn_proto),
        Some(fn_constr),
        Some(button_proto.into()),
    );

    (button, button_proto.into())
}

/// Implements `Button` constructor.
pub fn constructor<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    Ok(Value::Undefined.into())
}
