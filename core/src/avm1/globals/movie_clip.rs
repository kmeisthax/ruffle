//! MovieClip prototype

use crate::avm1::function::{Executable, FunctionObject};
use crate::avm1::globals::display_object::{self, AVM_DEPTH_BIAS, AVM_MAX_DEPTH};
use crate::avm1::property::Attribute::*;
use crate::avm1::return_value::ReturnValue;
use crate::avm1::{Avm1, Error, Object, ScriptObject, TObject, UpdateContext, Value};
use crate::backend::navigator::NavigationMethod;
use crate::display_object::{DisplayObject, EditText, MovieClip, TDisplayObject};
use crate::prelude::*;
use crate::tag_utils::SwfSlice;
use gc_arena::MutationContext;
use swf::Twips;

/// Implements `MovieClip`
pub fn constructor<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    Ok(Value::Undefined.into())
}

macro_rules! with_movie_clip {
    ( $gc_context: ident, $object:ident, $fn_proto: expr, $($name:expr => $fn:expr),* ) => {{
        $(
            $object.force_set_function(
                $name,
                |avm, context: &mut UpdateContext<'_, 'gc, '_>, this, args| -> Result<ReturnValue<'gc>, Error> {
                    if let Some(display_object) = this.as_display_object() {
                        if let Some(movie_clip) = display_object.as_movie_clip() {
                            return $fn(movie_clip, avm, context, args);
                        }
                    }
                    Ok(Value::Undefined.into())
                } as crate::avm1::function::NativeFunction<'gc>,
                $gc_context,
                DontDelete | ReadOnly | DontEnum,
                $fn_proto
            );
        )*
    }};
}

#[allow(clippy::comparison_chain)]
pub fn hit_test<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    if args.len() > 1 {
        let x = args.get(0).unwrap().as_number(avm, context)?;
        let y = args.get(1).unwrap().as_number(avm, context)?;
        let shape = args
            .get(2)
            .map(|v| v.as_bool(avm.current_swf_version()))
            .unwrap_or(false);
        if shape {
            log::warn!("Ignoring shape hittest and using bounding box instead. Shape based hit detection is not yet implemented. See https://github.com/ruffle-rs/ruffle/issues/177");
        }
        if x.is_finite() && y.is_finite() {
            // The docs say the point is in "Stage coordinates", but actually they are in root coordinates.
            // root can be moved via _root._x etc., so we actually have to transform from root to world space.
            let point = movie_clip
                .root()
                .local_to_global((Twips::from_pixels(x), Twips::from_pixels(y)));
            return Ok(movie_clip.hit_test(point).into());
        }
    } else if args.len() == 1 {
        let other = args
            .get(0)
            .unwrap()
            .as_object()
            .ok()
            .and_then(|o| o.as_display_object());
        if let Some(other) = other {
            return Ok(other
                .world_bounds()
                .intersects(&movie_clip.world_bounds())
                .into());
        }
    }

    Ok(false.into())
}

pub fn create_proto<'gc>(
    gc_context: MutationContext<'gc, '_>,
    proto: Object<'gc>,
    constr: Object<'gc>,
    fn_proto: Object<'gc>,
    fn_constr: Object<'gc>,
) -> (Object<'gc>, Object<'gc>) {
    let mut movie_clip_proto = ScriptObject::object(gc_context, Some(proto), Some(constr));

    display_object::define_display_object_proto(gc_context, movie_clip_proto, fn_proto);

    with_movie_clip!(
        gc_context,
        movie_clip_proto,
        Some(fn_proto),
        "attachMovie" => attach_movie,
        "createEmptyMovieClip" => create_empty_movie_clip,
        "createTextField" => create_text_field,
        "duplicateMovieClip" => duplicate_movie_clip,
        "getBounds" => get_bounds,
        "getBytesLoaded" => get_bytes_loaded,
        "getBytesTotal" => get_bytes_total,
        "getNextHighestDepth" => get_next_highest_depth,
        "getRect" => get_rect,
        "globalToLocal" => global_to_local,
        "gotoAndPlay" => goto_and_play,
        "gotoAndStop" => goto_and_stop,
        "hitTest" => hit_test,
        "loadMovie" => load_movie,
        "loadVariables" => load_variables,
        "localToGlobal" => local_to_global,
        "nextFrame" => next_frame,
        "play" => play,
        "prevFrame" => prev_frame,
        "removeMovieClip" => remove_movie_clip,
        "startDrag" => start_drag,
        "stop" => stop,
        "stopDrag" => stop_drag,
        "swapDepths" => swap_depths,
        "toString" => to_string,
        "unloadMovie" => unload_movie
    );

    let movie_clip = FunctionObject::function(
        gc_context,
        Executable::Native(constructor),
        Some(fn_proto),
        Some(fn_constr),
        Some(movie_clip_proto.into()),
    );

    (movie_clip, movie_clip_proto.into())
}

fn attach_movie<'gc>(
    mut movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    let (export_name, new_instance_name, depth) = match &args[0..3] {
        [export_name, new_instance_name, depth] => (
            export_name.clone().coerce_to_string(avm, context)?,
            new_instance_name.clone().coerce_to_string(avm, context)?,
            depth.as_i32().unwrap_or(0).wrapping_add(AVM_DEPTH_BIAS),
        ),
        _ => {
            log::error!("MovieClip.attachMovie: Too few parameters");
            return Ok(Value::Undefined.into());
        }
    };
    let init_object = args.get(3);

    // TODO: What is the derivation of this max value? It shows up a few times in the AVM...
    // 2^31 - 16777220
    if depth < 0 || depth > AVM_MAX_DEPTH {
        return Ok(Value::Undefined.into());
    }

    if let Ok(mut new_clip) = context
        .library
        .library_for_movie(movie_clip.movie().unwrap())
        .ok_or_else(|| "Movie is missing!".into())
        .and_then(|l| l.instantiate_by_export_name(&export_name, context.gc_context))
    {
        // Set name and attach to parent.
        new_clip.set_name(context.gc_context, &new_instance_name);
        movie_clip.add_child_from_avm(context, new_clip, depth);
        new_clip.post_instantiation(avm, context, new_clip);
        new_clip.run_frame(avm, context);

        // Copy properties from init_object to the movieclip.
        let new_clip = new_clip.object().as_object().unwrap();
        if let Some(Value::Object(o)) = init_object {
            for k in o.get_keys(avm) {
                let value = o.get(&k, avm, context)?.resolve(avm, context)?;
                new_clip.set(&k, value, avm, context)?;
            }
        }
        Ok(new_clip.into())
    } else {
        log::warn!("Unable to attach '{}'", export_name);
        Ok(Value::Undefined.into())
    }
}

fn create_empty_movie_clip<'gc>(
    mut movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    let (new_instance_name, depth) = match &args[0..2] {
        [new_instance_name, depth] => (
            new_instance_name.clone().coerce_to_string(avm, context)?,
            depth.as_i32().unwrap_or(0).wrapping_add(AVM_DEPTH_BIAS),
        ),
        _ => {
            log::error!("MovieClip.attachMovie: Too few parameters");
            return Ok(Value::Undefined.into());
        }
    };

    // Create empty movie clip.
    let swf_movie = movie_clip
        .movie()
        .or_else(|| avm.base_clip().movie())
        .unwrap();
    let mut new_clip = MovieClip::new(SwfSlice::empty(swf_movie), context.gc_context);

    // Set name and attach to parent.
    new_clip.set_name(context.gc_context, &new_instance_name);
    movie_clip.add_child_from_avm(context, new_clip.into(), depth);
    new_clip.post_instantiation(avm, context, new_clip.into());
    new_clip.run_frame(avm, context);

    Ok(new_clip.object().into())
}

fn create_text_field<'gc>(
    mut movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    let movie = avm.base_clip().movie().unwrap();
    let instance_name = args
        .get(0)
        .cloned()
        .unwrap_or(Value::Undefined)
        .coerce_to_string(avm, context)?;
    let depth = args
        .get(1)
        .cloned()
        .unwrap_or(Value::Undefined)
        .as_number(avm, context)?;
    let x = args
        .get(2)
        .cloned()
        .unwrap_or(Value::Undefined)
        .as_number(avm, context)?;
    let y = args
        .get(3)
        .cloned()
        .unwrap_or(Value::Undefined)
        .as_number(avm, context)?;
    let width = args
        .get(4)
        .cloned()
        .unwrap_or(Value::Undefined)
        .as_number(avm, context)?;
    let height = args
        .get(5)
        .cloned()
        .unwrap_or(Value::Undefined)
        .as_number(avm, context)?;

    let mut text_field: DisplayObject<'gc> =
        EditText::new(context, movie, x, y, width, height).into();
    text_field.set_name(context.gc_context, &instance_name);
    movie_clip.add_child_from_avm(context, text_field, depth as Depth);
    text_field.post_instantiation(avm, context, text_field);

    if avm.current_swf_version() >= 8 {
        //SWF8+ returns the `TextField` instance here
        Ok(text_field.object().into())
    } else {
        Ok(Value::Undefined.into())
    }
}

fn duplicate_movie_clip<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    // duplicateMovieClip method uses biased depth compared to CloneSprite
    duplicate_movie_clip_with_bias(movie_clip, avm, context, args, AVM_DEPTH_BIAS)
}

pub fn duplicate_movie_clip_with_bias<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
    depth_bias: i32,
) -> Result<ReturnValue<'gc>, Error> {
    let (new_instance_name, depth) = match &args[0..2] {
        [new_instance_name, depth] => (
            new_instance_name.clone().coerce_to_string(avm, context)?,
            depth.as_i32().unwrap_or(0).wrapping_add(depth_bias),
        ),
        _ => {
            log::error!("MovieClip.attachMovie: Too few parameters");
            return Ok(Value::Undefined.into());
        }
    };
    let init_object = args.get(2);

    // Can't duplicate the root!
    let mut parent = if let Some(parent) = movie_clip.parent().and_then(|o| o.as_movie_clip()) {
        parent
    } else {
        return Ok(Value::Undefined.into());
    };

    // TODO: What is the derivation of this max value? It shows up a few times in the AVM...
    // 2^31 - 16777220
    if depth < 0 || depth > AVM_MAX_DEPTH {
        return Ok(Value::Undefined.into());
    }

    if let Ok(mut new_clip) = context
        .library
        .library_for_movie(movie_clip.movie().unwrap())
        .ok_or_else(|| "Movie is missing!".into())
        .and_then(|l| l.instantiate_by_id(movie_clip.id(), context.gc_context))
    {
        // Set name and attach to parent.
        new_clip.set_name(context.gc_context, &new_instance_name);
        parent.add_child_from_avm(context, new_clip, depth);

        // Copy display properties from previous clip to new clip.
        new_clip.set_matrix(context.gc_context, &*movie_clip.matrix());
        new_clip.set_color_transform(context.gc_context, &*movie_clip.color_transform());
        // TODO: Any other properties we should copy...?
        // Definitely not ScriptObject properties.

        new_clip.post_instantiation(avm, context, new_clip);
        new_clip.run_frame(avm, context);

        // Copy properties from init_object to the movieclip.
        let new_clip = new_clip.object().as_object().unwrap();
        if let Some(Value::Object(o)) = init_object {
            for k in o.get_keys(avm) {
                let value = o.get(&k, avm, context)?.resolve(avm, context)?;
                new_clip.set(&k, value, avm, context)?;
            }
        }
        Ok(new_clip.into())
    } else {
        log::warn!("Unable to duplicate clip '{}'", movie_clip.name());
        Ok(Value::Undefined.into())
    }
}

fn get_bytes_loaded<'gc>(
    _movie_clip: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    _context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    // TODO find a correct value
    Ok(1.0.into())
}

fn get_bytes_total<'gc>(
    _movie_clip: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    _context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    // TODO find a correct value
    Ok(1.0.into())
}

fn get_next_highest_depth<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    _context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    if avm.current_swf_version() >= 7 {
        let depth = std::cmp::max(
            movie_clip
                .highest_depth()
                .unwrap_or(0)
                .wrapping_sub(AVM_DEPTH_BIAS - 1),
            0,
        );
        Ok(depth.into())
    } else {
        Ok(Value::Undefined.into())
    }
}

fn goto_and_play<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    goto_frame(movie_clip, avm, context, args, false, 0)
}

fn goto_and_stop<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    goto_frame(movie_clip, avm, context, args, true, 0)
}

pub fn goto_frame<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
    stop: bool,
    scene_offset: u16,
) -> Result<ReturnValue<'gc>, Error> {
    match args.get(0).cloned().unwrap_or(Value::Undefined) {
        // Goto only runs if n is an integer
        Value::Number(n) if n.fract() == 0.0 => {
            // Frame #
            // Gotoing <= 0 has no effect.
            // Gotoing greater than _totalframes jumps to the last frame.
            // Wraps around as an i32.
            // TODO: -1 +1 here to match Flash's behavior.
            // We probably want to change our frame representation to 0-based.
            // Scene offset is only used by GotoFrame2 global opcode.
            let mut frame = crate::avm1::value::f64_to_wrapping_i32(n);
            frame = frame.wrapping_sub(1);
            frame = frame.wrapping_add(i32::from(scene_offset));
            if frame >= 0 {
                movie_clip.goto_frame(avm, context, frame.saturating_add(1) as u16, stop);
            }
        }
        val => {
            // Coerce to string and search for a frame label.
            let frame_label = val.clone().coerce_to_string(avm, context)?;
            if let Some(mut frame) = movie_clip.frame_label_to_number(&frame_label) {
                frame = frame.wrapping_add(scene_offset);
                movie_clip.goto_frame(avm, context, frame, stop);
            }
        }
    }
    Ok(Value::Undefined.into())
}

fn next_frame<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    movie_clip.next_frame(avm, context);
    Ok(Value::Undefined.into())
}

fn play<'gc>(
    movie_clip: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    movie_clip.play(context);
    Ok(Value::Undefined.into())
}

fn prev_frame<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    movie_clip.prev_frame(avm, context);
    Ok(Value::Undefined.into())
}

fn remove_movie_clip<'gc>(
    movie_clip: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    // removeMovieClip method uses biased depth compared to RemoveSprite
    remove_movie_clip_with_bias(movie_clip, context, AVM_DEPTH_BIAS)
}

pub fn remove_movie_clip_with_bias<'gc>(
    movie_clip: MovieClip<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    depth_bias: i32,
) -> Result<ReturnValue<'gc>, Error> {
    let depth = movie_clip.depth().wrapping_add(depth_bias);
    // Can only remove positive depths (when offset by the AVM depth bias).
    // Generally this prevents you from removing non-dynamically created clips,
    // although you can get around it with swapDepths.
    // TODO: Figure out the derivation of this range.
    if depth >= AVM_DEPTH_BIAS && depth < 2_130_706_416 {
        // Need a parent to remove from.
        let mut parent = if let Some(parent) = movie_clip.parent().and_then(|o| o.as_movie_clip()) {
            parent
        } else {
            return Ok(Value::Undefined.into());
        };

        parent.remove_child_from_avm(context, movie_clip.into());
    }
    Ok(Value::Undefined.into())
}

fn start_drag<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    crate::avm1::start_drag(movie_clip.into(), avm, context, args);
    Ok(Value::Undefined.into())
}

fn stop<'gc>(
    movie_clip: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    movie_clip.stop(context);
    Ok(Value::Undefined.into())
}

fn stop_drag<'gc>(
    _movie_clip: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    // It doesn't matter which clip we call this on; it simply stops any active drag.
    *context.drag_object = None;
    Ok(Value::Undefined.into())
}

fn swap_depths<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    let arg = args.get(0).cloned().unwrap_or(Value::Undefined);

    let parent = if let Some(parent) = movie_clip.parent().and_then(|o| o.as_movie_clip()) {
        parent
    } else {
        return Ok(Value::Undefined.into());
    };

    let mut depth = None;
    if let Value::Number(n) = arg {
        depth = Some(crate::avm1::value::f64_to_wrapping_i32(n).wrapping_add(AVM_DEPTH_BIAS));
    } else if let Some(target) =
        avm.resolve_target_display_object(context, movie_clip.into(), arg)?
    {
        if let Some(target_parent) = target.parent() {
            if DisplayObject::ptr_eq(target_parent, parent.into()) {
                depth = Some(target.depth())
            } else {
                log::warn!("MovieClip.swapDepths: Objects do not have the same parent");
            }
        }
    } else {
        log::warn!("MovieClip.swapDepths: Invalid target");
    };

    if let Some(depth) = depth {
        if depth < 0 || depth > AVM_MAX_DEPTH {
            // Depth out of range; no action.
            return Ok(Value::Undefined.into());
        }

        if depth != movie_clip.depth() {
            parent.swap_child_to_depth(context, movie_clip.into(), depth);
        }
    }

    Ok(Value::Undefined.into())
}

fn to_string<'gc>(
    movie_clip: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    _context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    Ok(movie_clip.path().into())
}

fn local_to_global<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    if let Value::Object(point) = args.get(0).unwrap_or(&Value::Undefined) {
        // localToGlobal does no coercion; it fails if the properties are not numbers.
        // It does not search the prototype chain.
        if let (Value::Number(x), Value::Number(y)) = (
            point
                .get_local("x", avm, context, *point)?
                .resolve(avm, context)?,
            point
                .get_local("y", avm, context, *point)?
                .resolve(avm, context)?,
        ) {
            let x = Twips::from_pixels(x);
            let y = Twips::from_pixels(y);
            let (out_x, out_y) = movie_clip.local_to_global((x, y));
            point.set("x", out_x.to_pixels().into(), avm, context)?;
            point.set("y", out_y.to_pixels().into(), avm, context)?;
        } else {
            log::warn!("MovieClip.localToGlobal: Invalid x and y properties");
        }
    } else {
        log::warn!("MovieClip.localToGlobal: Missing point parameter");
    }

    Ok(Value::Undefined.into())
}

fn get_bounds<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    let target = match args.get(0) {
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::Object(o)) if o.as_display_object().is_some() => o.as_display_object(),
        Some(val) => {
            let path = val.clone().coerce_to_string(avm, context)?;
            avm.resolve_target_display_object(context, movie_clip.into(), path.into())?
        }
        None => Some(movie_clip.into()),
    };

    if let Some(target) = target {
        let bounds = movie_clip.bounds();
        let out_bounds = if DisplayObject::ptr_eq(movie_clip.into(), target) {
            // Getting the clips bounds in its own coordinate space; no AABB transform needed.
            bounds
        } else {
            // Transform AABB to target space.
            // Calculate the matrix to transform into the target coordinate space, and transform the above AABB.
            // Note that this doesn't produce as tight of an AABB as if we had used `bounds_with_transform` with
            // the final matrix, but this matches Flash's behavior.
            let to_global_matrix = movie_clip.local_to_global_matrix();
            let to_target_matrix = target.global_to_local_matrix();
            let bounds_transform = to_target_matrix * to_global_matrix;
            bounds.transform(&bounds_transform)
        };

        let out = ScriptObject::object(
            context.gc_context,
            Some(avm.prototypes.object),
            Some(avm.constructors.object),
        );
        out.set("xMin", out_bounds.x_min.to_pixels().into(), avm, context)?;
        out.set("yMin", out_bounds.y_min.to_pixels().into(), avm, context)?;
        out.set("xMax", out_bounds.x_max.to_pixels().into(), avm, context)?;
        out.set("yMax", out_bounds.y_max.to_pixels().into(), avm, context)?;
        Ok(out.into())
    } else {
        Ok(Value::Undefined.into())
    }
}

fn get_rect<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    // TODO: This should get the bounds ignoring strokes. Always equal to or smaller than getBounds.
    // Just defer to getBounds for now. Will have to store edge_bounds vs. shape_bounds in Graphic.
    get_bounds(movie_clip, avm, context, args)
}

fn global_to_local<'gc>(
    movie_clip: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    if let Value::Object(point) = args.get(0).unwrap_or(&Value::Undefined) {
        // globalToLocal does no coercion; it fails if the properties are not numbers.
        // It does not search the prototype chain.
        if let (Value::Number(x), Value::Number(y)) = (
            point
                .get_local("x", avm, context, *point)?
                .resolve(avm, context)?,
            point
                .get_local("y", avm, context, *point)?
                .resolve(avm, context)?,
        ) {
            let x = Twips::from_pixels(x);
            let y = Twips::from_pixels(y);
            let (out_x, out_y) = movie_clip.global_to_local((x, y));
            point.set("x", out_x.to_pixels().into(), avm, context)?;
            point.set("y", out_y.to_pixels().into(), avm, context)?;
        } else {
            log::warn!("MovieClip.globalToLocal: Invalid x and y properties");
        }
    } else {
        log::warn!("MovieClip.globalToLocal: Missing point parameter");
    }

    Ok(Value::Undefined.into())
}

fn load_movie<'gc>(
    target: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    let url = args
        .get(0)
        .cloned()
        .unwrap_or(Value::Undefined)
        .coerce_to_string(avm, context)?;
    let method = args.get(1).cloned().unwrap_or(Value::Undefined);
    let method = NavigationMethod::from_method_str(&method.coerce_to_string(avm, context)?);
    let (url, opts) = avm.locals_into_request_options(context, url, method);
    let fetch = context.navigator.fetch(url, opts);
    let process = context.load_manager.load_movie_into_clip(
        context.player.clone().unwrap(),
        DisplayObject::MovieClip(target),
        fetch,
        None,
    );

    context.navigator.spawn_future(process);

    Ok(Value::Undefined.into())
}

fn load_variables<'gc>(
    target: MovieClip<'gc>,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    let url = args
        .get(0)
        .cloned()
        .unwrap_or(Value::Undefined)
        .coerce_to_string(avm, context)?;
    let method = args.get(1).cloned().unwrap_or(Value::Undefined);
    let method = NavigationMethod::from_method_str(&method.coerce_to_string(avm, context)?);
    let (url, opts) = avm.locals_into_request_options(context, url, method);
    let fetch = context.navigator.fetch(url, opts);
    let process = context.load_manager.load_form_into_object(
        context.player.clone().unwrap(),
        target.object().as_object()?,
        fetch,
    );

    context.navigator.spawn_future(process);

    Ok(Value::Undefined.into())
}

fn unload_movie<'gc>(
    mut target: MovieClip<'gc>,
    _avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error> {
    target.unload(context);
    target.replace_with_movie(context.gc_context, None);

    Ok(Value::Undefined.into())
}
