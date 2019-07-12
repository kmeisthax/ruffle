pub enum Character<'a> {
    Graphic(Box<crate::graphic::Graphic>),
    MovieClip(Box<crate::movie_clip::MovieClip<'a>>),
    Bitmap(crate::backend::render::BitmapHandle),
    Button(Box<crate::button::Button<'a>>),
    Font(Box<crate::font::Font>),
    MorphShape(Box<crate::morph_shape::MorphShape>),
    Text(Box<crate::text::Text>),
    Sound(crate::backend::audio::SoundHandle),
}
