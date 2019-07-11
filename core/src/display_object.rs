use crate::player::{RenderContext, UpdateContext};
use crate::prelude::*;
use crate::transform::Transform;
use std::collections::VecDeque;

#[derive(Clone)]
pub struct DisplayObjectBase {
    depth: Depth,
    transform: Transform,
    name: String,
    clip_depth: Depth,
}

impl Default for DisplayObjectBase {
    fn default() -> Self {
        Self {
            depth: Default::default(),
            transform: Default::default(),
            name: Default::default(),
            clip_depth: Default::default(),
        }
    }
}

impl DisplayObject for DisplayObjectBase {
    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn get_matrix(&self) -> &Matrix {
        &self.transform.matrix
    }
    fn set_matrix(&mut self, matrix: &Matrix) {
        self.transform.matrix = *matrix;
    }
    fn get_color_transform(&self) -> &ColorTransform {
        &self.transform.color_transform
    }
    fn set_color_transform(&mut self, color_transform: &ColorTransform) {
        self.transform.color_transform = *color_transform;
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
    fn clip_depth(&self) -> Depth {
        self.clip_depth
    }
    fn set_clip_depth(&mut self, depth: Depth) {
        self.clip_depth = depth;
    }
    fn box_clone(&self) -> Box<DisplayObject> {
        Box::new(self.clone())
    }
}

pub trait DisplayObject: std::any::Any {
    fn transform(&self) -> &Transform;
    fn get_matrix(&self) -> &Matrix;
    fn set_matrix(&mut self, matrix: &Matrix);
    fn get_color_transform(&self) -> &ColorTransform;
    fn set_color_transform(&mut self, color_transform: &ColorTransform);
    fn name(&self) -> &str;
    fn set_name(&mut self, name: &str);
    fn clip_depth(&self) -> Depth;
    fn set_clip_depth(&mut self, depth: Depth);

    fn preload(&mut self, _context: &mut UpdateContext) {}
    fn run_frame(&mut self, _context: &mut UpdateContext) {}
    fn run_post_frame(&mut self, _context: &mut UpdateContext) {}
    fn render(&self, _context: &mut RenderContext) {}

    fn handle_click(&mut self, _pos: (f32, f32)) {}
    fn visit_children(&self, queue: &mut VecDeque<Box<DisplayObject>>) {}
    fn as_movie_clip(&self) -> Option<&crate::movie_clip::MovieClip> {
        None
    }
    fn as_movie_clip_mut(&mut self) -> Option<&mut crate::movie_clip::MovieClip> {
        None
    }
    fn as_morph_shape(&self) -> Option<&crate::morph_shape::MorphShape> {
        None
    }
    fn as_morph_shape_mut(&mut self) -> Option<&mut crate::morph_shape::MorphShape> {
        None
    }
    fn box_clone(&self) -> Box<DisplayObject>;
}

impl Clone for Box<DisplayObject> {
    fn clone(&self) -> Box<DisplayObject> {
        self.box_clone()
    }
}

macro_rules! impl_display_object {
    ($field:ident) => {
        fn transform(&self) -> &crate::transform::Transform {
            self.$field.transform()
        }
        fn get_matrix(&self) -> &Matrix {
            self.$field.get_matrix()
        }
        fn set_matrix(&mut self, matrix: &Matrix) {
            self.$field.set_matrix(matrix)
        }
        fn get_color_transform(&self) -> &ColorTransform {
            self.$field.get_color_transform()
        }
        fn set_color_transform(&mut self, color_transform: &ColorTransform) {
            self.$field.set_color_transform(color_transform)
        }
        fn name(&self) -> &str {
            self.$field.name()
        }
        fn set_name(&mut self, name: &str) {
            self.$field.set_name(name)
        }
        fn clip_depth(&self) -> $crate::prelude::Depth {
            self.$field.clip_depth()
        }
        fn set_clip_depth(&mut self, depth: $crate::prelude::Depth) {
            self.$field.set_clip_depth(depth)
        }
        fn box_clone(&self) -> Box<$crate::display_object::DisplayObject> {
            Box::new(self.clone())
        }
    };
}
