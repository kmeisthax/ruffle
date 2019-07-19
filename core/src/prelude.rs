pub use crate::color_transform::ColorTransform;
pub use crate::display_object::DisplayObject;
pub use crate::matrix::Matrix;
pub use log::{error, info, trace, warn};
pub use swf::{CharacterId, Color, Depth, Twips};

use gc_arena::{Gc, GcCell};

pub type DisplayNode<'gc> = Gc<'gc, GcCell<'gc, Box<dyn DisplayObject<'gc>>>>;
