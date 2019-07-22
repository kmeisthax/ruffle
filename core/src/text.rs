use crate::display_object::{DisplayObject, DisplayObjectBase};
use crate::player::{RenderContext, UpdateContext};
use crate::transform::Transform;

#[derive(Clone)]
pub struct Text {
    base: DisplayObjectBase,
    text_blocks: Vec<swf::TextRecord>,
}

impl Text {
    pub fn from_swf_tag(tag: &swf::Text) -> Self {
        Self {
            base: Default::default(),
            text_blocks: tag.records.clone(),
        }
    }
}

impl<'gc> DisplayObject<'gc> for Text {
    impl_display_object!(base);

    fn run_frame(&mut self, _context: &mut UpdateContext) {
        // Noop
    }

    fn render(&self, context: &mut RenderContext) {
        context.transform_stack.push(self.transform());

        let mut x = Default::default();
        let mut y = Default::default();
        let mut color = swf::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        let mut font_id = 0;
        let mut height = 0;
        let mut transform: Transform = Default::default();
        for block in &self.text_blocks {
            x = block.x_offset.unwrap_or(x);
            y = block.y_offset.unwrap_or(y);
            color = block.color.as_ref().unwrap_or_else(|| &color).clone();
            font_id = block.font_id.unwrap_or(font_id);
            height = block.height.unwrap_or(height);
            let scale = f32::from(height) / 1024.0;
            transform.matrix.a = scale;
            transform.matrix.d = scale;
            transform.matrix.tx = x.get() as f32;
            transform.matrix.ty = y.get() as f32;
            transform.color_transform.r_mult = f32::from(color.r) / 255.0;
            transform.color_transform.g_mult = f32::from(color.g) / 255.0;
            transform.color_transform.b_mult = f32::from(color.b) / 255.0;
            transform.color_transform.a_mult = f32::from(color.a) / 255.0;
            if let Some(font) = context.library.get_font(font_id) {
                for c in &block.glyphs {
                    if let Some(glyph) = font.get_glyph(c.index as usize) {
                        context.transform_stack.push(&transform);
                        context
                            .renderer
                            .render_shape(glyph, context.transform_stack.transform());
                        context.transform_stack.pop();
                        transform.matrix.tx += c.advance as f32;
                    }
                }
            }
        }
        context.transform_stack.pop();
    }
}

unsafe impl<'gc> gc_arena::Collect for Text {
    #[inline]
    fn needs_trace() -> bool {
        false
    }
}
