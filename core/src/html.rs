//! HTML layout engine
//!
//! This module allows the computation of HTML layout trees from an XML node
//! hierarchy. It is primarily intended to serve as the backing representation
//! of an `EditText`'s visual appearance.

mod css_types;
mod dimensions;
mod iterators;
mod layout;
mod text_format;

pub use css_types::CSSStylesheet;
pub use dimensions::BoxBounds;
pub use dimensions::Position;
pub use dimensions::Size;
pub use layout::LayoutBox;
pub use text_format::{FormatSpans, TextFormat, TextSpan};

#[cfg(test)]
mod test;
