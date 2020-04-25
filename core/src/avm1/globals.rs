use crate::avm1::fscommand;
use crate::avm1::function::Executable;
use crate::avm1::listeners::SystemListeners;
use crate::avm1::return_value::ReturnValue;
use crate::avm1::{Avm1, Error, Object, ScriptObject, TObject, UpdateContext, Value};
use crate::backend::navigator::NavigationMethod;
use enumset::EnumSet;
use gc_arena::Collect;
use gc_arena::MutationContext;
use rand::Rng;
use std::f64;

mod array;
pub(crate) mod boolean;
pub(crate) mod button;
mod color;
pub(crate) mod display_object;
mod function;
mod key;
mod math;
pub(crate) mod mouse;
pub(crate) mod movie_clip;
mod movie_clip_loader;
pub(crate) mod number;
mod object;
mod sound;
mod stage;
pub(crate) mod string;
pub(crate) mod text_field;
mod text_format;
mod xml;

#[allow(non_snake_case, unused_must_use)] //can't use errors yet
pub fn getURL<'a, 'gc>(
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'a, 'gc, '_>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    //TODO: Error behavior if no arguments are present
    if let Some(url_val) = args.get(0) {
        let swf_version = avm.current_swf_version();
        let url = url_val.clone().into_string(swf_version);
        if let Some(fscommand) = fscommand::parse(&url) {
            fscommand::handle(fscommand, avm, context);
            return Ok(Value::Undefined.into());
        }

        let window = args.get(1).map(|v| v.clone().into_string(swf_version));
        let method = match args.get(2) {
            Some(Value::String(s)) if s == "GET" => Some(NavigationMethod::GET),
            Some(Value::String(s)) if s == "POST" => Some(NavigationMethod::POST),
            _ => None,
        };
        let vars_method = method.map(|m| (m, avm.locals_into_form_values(context)));

        context.navigator.navigate_to_url(url, window, vars_method);
    }

    Ok(Value::Undefined.into())
}

pub fn random<'gc>(
    _avm: &mut Avm1<'gc>,
    action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    match args.get(0) {
        Some(Value::Number(max)) => Ok(action_context.rng.gen_range(0.0f64, max).floor().into()),
        _ => Ok(Value::Undefined.into()), //TODO: Shouldn't this be an error condition?
    }
}

pub fn is_nan<'gc>(
    avm: &mut Avm1<'gc>,
    action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    if let Some(val) = args.get(0) {
        Ok(val.as_number(avm, action_context)?.is_nan().into())
    } else {
        Ok(true.into())
    }
}

pub fn get_infinity<'gc>(
    avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    if avm.current_swf_version() > 4 {
        Ok(f64::INFINITY.into())
    } else {
        Ok(Value::Undefined.into())
    }
}

pub fn get_nan<'gc>(
    avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    if avm.current_swf_version() > 4 {
        Ok(f64::NAN.into())
    } else {
        Ok(Value::Undefined.into())
    }
}

/// This structure represents all system builtins that are used regardless of
/// whatever the hell happens to `_global`. These are, of course,
/// user-modifiable.
#[derive(Collect, Clone)]
#[collect(no_drop)]
pub struct SystemPrototypes<'gc> {
    pub button: Object<'gc>,
    pub object: Object<'gc>,
    pub function: Object<'gc>,
    pub movie_clip: Object<'gc>,
    pub sound: Object<'gc>,
    pub text_field: Object<'gc>,
    pub text_format: Object<'gc>,
    pub array: Object<'gc>,
    pub xml_node: Object<'gc>,
    pub string: Object<'gc>,
    pub number: Object<'gc>,
    pub boolean: Object<'gc>,
}

/// This structure represents all system builtin constructors that are used
/// regardless of whatever the hell happens to `_global`. These are, of course,
/// user-modifiable.
#[derive(Collect, Clone)]
#[collect(no_drop)]
pub struct SystemConstructors<'gc> {
    pub button: Object<'gc>,
    pub object: Object<'gc>,
    pub function: Object<'gc>,
    pub movie_clip: Object<'gc>,
    pub sound: Object<'gc>,
    pub text_field: Object<'gc>,
    pub text_format: Object<'gc>,
    pub array: Object<'gc>,
    pub xml_node: Object<'gc>,
    pub string: Object<'gc>,
    pub number: Object<'gc>,
    pub boolean: Object<'gc>,
}

/// Initialize default global scope and builtins for an AVM1 instance.
pub fn create_globals<'gc>(
    gc_context: MutationContext<'gc, '_>,
) -> (
    SystemPrototypes<'gc>,
    SystemConstructors<'gc>,
    Object<'gc>,
    SystemListeners<'gc>,
) {
    // `Object` and `Function are tricky to create, because `Function` is a
    // subclass of `Object`, but `Object` is an instance of `Function`. Thus, we
    // have to dance around half-constructed objects until both are initialized.
    let object_proto = ScriptObject::object_cell(gc_context, None, None);
    let function_proto = function::create_proto(gc_context, object_proto);

    object::fill_proto(gc_context, object_proto, function_proto);
    let object = object::create_object_object(gc_context, object_proto, function_proto);
    let function = function::finish_function_object(gc_context, function_proto, object);

    // Prototypes and constructors have to be created in a very specific order,
    // to ensure prototypes are linked to the constructor that would have been
    // called to create them via `new`. Now that `Object` and `Function` both
    // exist, we can create all the other immediate subclasses. We'll call
    // these "second-generation" builtins.
    let (button, button_proto) =
        button::create_proto(gc_context, object_proto, object, function_proto, function);

    let (movie_clip, movie_clip_proto) =
        movie_clip::create_proto(gc_context, object_proto, object, function_proto, function);

    let (movie_clip_loader, _movie_clip_loader_proto) =
        movie_clip_loader::create_proto(gc_context, object_proto, object, function_proto, function);

    let (sound, sound_proto) =
        sound::create_proto(gc_context, object_proto, object, function_proto, function);

    let (text_field, text_field_proto) =
        text_field::create_proto(gc_context, object_proto, object, function_proto, function);
    let (text_format, text_format_proto) =
        text_format::create_proto(gc_context, object_proto, object, function_proto, function);

    let (array, array_proto) =
        array::create_proto(gc_context, object_proto, object, function_proto, function);

    let (color, _color_proto) =
        color::create_proto(gc_context, object_proto, object, function_proto, function);
    let (xml_node, xml_node_proto) =
        xml::create_xmlnode_proto(gc_context, object_proto, object, function_proto, function);

    let (string, string_proto) =
        string::create_proto(gc_context, object_proto, object, function_proto, function);
    let (number, number_proto) =
        number::create_proto(gc_context, object_proto, object, function_proto, function);
    let (boolean, boolean_proto) =
        boolean::create_proto(gc_context, object_proto, object, function_proto, function);

    // These builtins are subclasses of one of the above classes, and thus
    // become third-generation builtins.
    let (xml, _xml_proto) = xml::create_xml_proto(
        gc_context,
        xml_node_proto,
        xml_node,
        function_proto,
        function,
    );

    // Now, we hook everything we just built up to the root scope, and hand it
    // off to the AVM that called us.
    let listeners = SystemListeners::new(gc_context, Some(array_proto), Some(array));

    let mut globals = ScriptObject::bare_object(gc_context);
    globals.define_value(gc_context, "Array", array.into(), EnumSet::empty());
    globals.define_value(gc_context, "Button", button.into(), EnumSet::empty());
    globals.define_value(gc_context, "Color", color.into(), EnumSet::empty());
    globals.define_value(gc_context, "Object", object.into(), EnumSet::empty());
    globals.define_value(gc_context, "Function", function.into(), EnumSet::empty());
    globals.define_value(gc_context, "MovieClip", movie_clip.into(), EnumSet::empty());
    globals.define_value(
        gc_context,
        "MovieClipLoader",
        movie_clip_loader.into(),
        EnumSet::empty(),
    );
    globals.define_value(gc_context, "Sound", sound.into(), EnumSet::empty());
    globals.define_value(gc_context, "TextField", text_field.into(), EnumSet::empty());
    globals.define_value(
        gc_context,
        "TextFormat",
        text_format.into(),
        EnumSet::empty(),
    );
    globals.define_value(gc_context, "XMLNode", xml_node.into(), EnumSet::empty());
    globals.define_value(gc_context, "XML", xml.into(), EnumSet::empty());
    globals.define_value(gc_context, "String", string.into(), EnumSet::empty());
    globals.define_value(gc_context, "Number", number.into(), EnumSet::empty());
    globals.define_value(gc_context, "Boolean", boolean.into(), EnumSet::empty());

    globals.define_value(
        gc_context,
        "Math",
        Value::Object(math::create(
            gc_context,
            Some(object_proto),
            Some(object),
            Some(function_proto),
        )),
        EnumSet::empty(),
    );
    globals.define_value(
        gc_context,
        "Mouse",
        Value::Object(mouse::create_mouse_object(
            gc_context,
            Some(object_proto),
            Some(object),
            Some(function_proto),
            &listeners.mouse,
        )),
        EnumSet::empty(),
    );
    globals.define_value(
        gc_context,
        "Key",
        Value::Object(key::create_key_object(
            gc_context,
            Some(object_proto),
            Some(object),
            Some(function_proto),
        )),
        EnumSet::empty(),
    );
    globals.define_value(
        gc_context,
        "Stage",
        Value::Object(stage::create_stage_object(
            gc_context,
            Some(object_proto),
            Some(object),
            Some(array_proto),
            Some(function_proto),
        )),
        EnumSet::empty(),
    );
    globals.force_set_function(
        "isNaN",
        is_nan,
        gc_context,
        EnumSet::empty(),
        Some(function_proto),
    );
    globals.force_set_function(
        "getURL",
        getURL,
        gc_context,
        EnumSet::empty(),
        Some(function_proto),
    );
    globals.force_set_function(
        "random",
        random,
        gc_context,
        EnumSet::empty(),
        Some(function_proto),
    );
    globals.force_set_function(
        "ASSetPropFlags",
        object::as_set_prop_flags,
        gc_context,
        EnumSet::empty(),
        Some(function_proto),
    );
    globals.add_property(
        gc_context,
        "NaN",
        Executable::Native(get_nan),
        None,
        EnumSet::empty(),
    );
    globals.add_property(
        gc_context,
        "Infinity",
        Executable::Native(get_infinity),
        None,
        EnumSet::empty(),
    );

    (
        SystemPrototypes {
            button: button_proto,
            object: object_proto,
            function: function_proto,
            movie_clip: movie_clip_proto,
            sound: sound_proto,
            text_field: text_field_proto,
            text_format: text_format_proto,
            array: array_proto,
            xml_node: xml_node_proto,
            string: string_proto,
            number: number_proto,
            boolean: boolean_proto,
        },
        SystemConstructors {
            button,
            object,
            function,
            movie_clip,
            sound,
            text_field,
            text_format,
            array,
            xml_node,
            string,
            number,
            boolean,
        },
        globals.into(),
        listeners,
    )
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)]
mod tests {
    use super::*;

    fn setup<'gc>(_avm: &mut Avm1<'gc>, context: &mut UpdateContext<'_, 'gc, '_>) -> Object<'gc> {
        create_globals(context.gc_context).2
    }

    test_method!(boolean_function, "Boolean", setup,
        [19] => {
            [true] => true,
            [false] => false,
            [10.0] => true,
            [-10.0] => true,
            [0.0] => false,
            [std::f64::INFINITY] => true,
            [std::f64::NAN] => false,
            [""] => false,
            ["Hello"] => true,
            [" "] => true,
            ["0"] => true,
            ["1"] => true,
            [Value::Undefined] => false,
            [Value::Null] => false,
            [] => Value::Undefined
        },
        [6] => {
            [true] => true,
            [false] => false,
            [10.0] => true,
            [-10.0] => true,
            [0.0] => false,
            [std::f64::INFINITY] => true,
            [std::f64::NAN] => false,
            [""] => false,
            ["Hello"] => false,
            [" "] => false,
            ["0"] => false,
            ["1"] => true,
            [Value::Undefined] => false,
            [Value::Null] => false,
            [] => Value::Undefined
        }
    );

    test_method!(is_nan_function, "isNaN", setup,
        [19] => {
            [true] => false,
            [false] => false,
            [10.0] => false,
            [-10.0] => false,
            [0.0] => false,
            [std::f64::INFINITY] => false,
            [std::f64::NAN] => true,
            [""] => true,
            ["Hello"] => true,
            [" "] => true,
            ["  5  "] => true,
            ["0"] => false,
            ["1"] => false,
            ["Infinity"] => true,
            ["100a"] => true,
            ["0x10"] => false,
            ["0xhello"] => true,
            ["0x1999999981ffffff"] => false,
            ["0xUIXUIDFKHJDF012345678"] => true,
            ["123e-1"] => false,
            [] => true
        }
    );

    test_method!(number_function, "Number", setup,
        [5, 6] => {
            [true] => 1.0,
            [false] => 0.0,
            [10.0] => 10.0,
            [-10.0] => -10.0,
            ["true"] => std::f64::NAN,
            ["false"] => std::f64::NAN,
            [1.0] => 1.0,
            [0.0] => 0.0,
            [0.000] => 0.0,
            ["0.000"] => 0.0,
            ["True"] => std::f64::NAN,
            ["False"] => std::f64::NAN,
            [std::f64::NAN] => std::f64::NAN,
            [std::f64::INFINITY] => std::f64::INFINITY,
            [std::f64::NEG_INFINITY] => std::f64::NEG_INFINITY,
            [" 12"] => 12.0,
            [" \t\r\n12"] => 12.0,
            ["\u{A0}12"] => std::f64::NAN,
            [" 0x12"] => std::f64::NAN,
            ["01.2"] => 1.2,
            [""] => std::f64::NAN,
            ["Hello"] => std::f64::NAN,
            [" "] => std::f64::NAN,
            ["  5  "] => std::f64::NAN,
            ["0"] => 0.0,
            ["1"] => 1.0,
            ["Infinity"] => std::f64::NAN,
            ["100a"] => std::f64::NAN,
            ["0xhello"] => std::f64::NAN,
            ["123e-1"] => 12.3,
            ["0xUIXUIDFKHJDF012345678"] => std::f64::NAN,
            [] => 0.0
        },
        [5] => {
            ["0x12"] => std::f64::NAN,
            ["0x10"] => std::f64::NAN,
            ["0x1999999981ffffff"] => std::f64::NAN,
            ["010"] => 10,
            ["-010"] => -10,
            ["+010"] => 10,
            [" 010"] => 10,
            [" -010"] => -10,
            [" +010"] => 10,
            ["037777777777"] => 37777777777.0,
            ["-037777777777"] => -37777777777.0
        },
        [6, 7] => {
            ["0x12"] => 18.0,
            ["0x10"] => 16.0,
            ["-0x10"] => std::f64::NAN,
            ["0x1999999981ffffff"] => -2113929217.0,
            ["010"] => 8,
            ["-010"] => -8,
            ["+010"] => 8,
            [" 010"] => 10,
            [" -010"] => -10,
            [" +010"] => 10,
            ["037777777777"] => -1,
            ["-037777777777"] => 1
        },
        [5, 6] => {
            [Value::Undefined] => 0.0,
            [Value::Null] => 0.0
        },
        [7] => {
            [Value::Undefined] => std::f64::NAN,
            [Value::Null] => std::f64::NAN
        }
    );
}
