//! CSS value types

use gc_arena::Collect;

/// All valid CSS unit types.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub enum Unit {
    /// Units of pixels.
    ///
    /// This is interpreted as virtual pixels on the Flash stage relative to
    /// the hosting display object's current transform. This is equivalent to
    /// stating the quantity in terms of 20 twips.
    Px,

    /// Units of em-height
    Em,

    /// Units of ex-height
    Ex,

    /// Units of 1 percent.
    ///
    /// Percentages are expected to be stored in the range of [0, 1].
    Percent,
}

/// All possible value types a property can be assigned.
///
/// The `K` parameter indicates a type which constitutes a CSS keyword that we
/// care about.
#[derive(Clone, Debug, Collect)]
#[collect(no_drop)]
pub enum Value<K> {
    /// The CSS value is not specified here and should be taken from the
    /// inheritied properties of the parent's computed style.
    Inherit,

    /// The CSS value is not specified here and should be taken from the
    /// default value that would be present on this element had this property
    /// not been specified in any applicable CSS rule.
    Initial,

    /// A valid CSS keyword.
    Keyword(K),

    /// The CSS value is a quoted string.
    Str(String),

    /// The CSS value is a URL to some external resource.
    Url(String),

    /// The CSS value is a dimensionless integer.
    Integer(i32),

    /// The CSS value is a dimensionless real (approximated with floats).
    Number(f32),

    /// The CSS value is a dimentioned real quantity of one of the unit types
    /// provided in `Unit`.
    Dimension(f32, Unit),

    /// The CSS value is an RGBA color.
    ///
    /// While CSS allows specifying colors in other systems, such as HSL, the
    /// internal representation of color must always be in RGBA.
    ///
    /// All components are in the range of [0, 1] and are not premultiplied.
    Color(f32, f32, f32, f32),

    /// The CSS value is a list of font families.
    ///
    /// The font this actually represents is the first font name that resolves.
    /// Fonts may be provided by the SWF file itself, or be provided by "device
    /// fonts".
    Font(Vec<String>),
}
