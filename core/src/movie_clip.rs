use crate::audio::AudioStreamHandle;
use crate::character::Character;
use crate::color_transform::ColorTransform;
use crate::display_object::{DisplayObject, DisplayObjectBase};
use crate::font::Font;
use crate::graphic::Graphic;
use crate::matrix::Matrix;
use crate::morph_shape::MorphShape;
use crate::player::{RenderContext, UpdateContext};
use crate::prelude::*;
use crate::tag_decoder::{TagDecoder, DecoderContext, DecoderResult, DecoderStatus};
use crate::text::Text;
use std::collections::BTreeMap;
use swf::read::SwfRead;

type Depth = i16;
type FrameNumber = u16;

#[derive(Clone)]
pub struct MovieClip<'gc> {
    base: DisplayObjectBase,
    tag_stream_start: u64,
    tag_stream_pos: u64,
    tag_stream_len: usize,
    is_playing: bool,
    action: Option<(usize, usize)>,
    goto_queue: Vec<FrameNumber>,
    current_frame: FrameNumber,
    total_frames: FrameNumber,

    audio_stream: Option<AudioStreamHandle>,
    stream_started: bool,

    children: BTreeMap<Depth, DisplayNode<'gc>>,
}

impl<'gc> MovieClip<'gc> {
    pub fn new() -> Self {
        Self {
            base: Default::default(),
            tag_stream_start: 0,
            tag_stream_pos: 0,
            tag_stream_len: 0,
            is_playing: true,
            action: None,
            goto_queue: Vec::new(),
            current_frame: 0,
            total_frames: 1,
            audio_stream: None,
            stream_started: false,
            children: BTreeMap::new(),
        }
    }

    pub fn new_with_data(tag_stream_start: u64, tag_stream_len: usize, num_frames: u16) -> Self {
        Self {
            base: Default::default(),
            tag_stream_start,
            tag_stream_pos: 0,
            tag_stream_len,
            is_playing: true,
            action: None,
            goto_queue: Vec::new(),
            current_frame: 0,
            audio_stream: None,
            stream_started: false,
            total_frames: num_frames,
            children: BTreeMap::new(),
        }
    }

    pub fn playing(&self) -> bool {
        self.is_playing
    }

    pub fn next_frame(&mut self) {
        if self.current_frame < self.total_frames {
            self.goto_frame(self.current_frame + 1, true);
        }
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn prev_frame(&mut self) {
        if self.current_frame > 1 {
            self.goto_frame(self.current_frame - 1, true);
        }
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
    }

    pub fn goto_frame(&mut self, frame: FrameNumber, stop: bool) {
        self.goto_queue.push(frame);

        if stop {
            self.stop();
        } else {
            self.play();
        }
    }

    pub fn x(&self) -> f32 {
        self.get_matrix().tx
    }

    pub fn y(&self) -> f32 {
        self.get_matrix().ty
    }

    pub fn x_scale(&self) -> f32 {
        self.get_matrix().a * 100.0
    }

    pub fn y_scale(&self) -> f32 {
        self.get_matrix().d * 100.0
    }

    pub fn current_frame(&self) -> FrameNumber {
        self.current_frame
    }

    pub fn total_frames(&self) -> FrameNumber {
        self.total_frames
    }

    pub fn frames_loaded(&self) -> FrameNumber {
        // TODO(Herschel): root needs to progressively stream in frames.
        self.total_frames
    }

    pub fn get_child_by_name(&self, name: &str) -> Option<&DisplayNode<'gc>> {
        self.children
            .values()
            .find(|child| child.read().name() == name)
    }

    pub fn frame_label_to_number(
        &self,
        frame_label: &str,
        context: &mut UpdateContext<'_, '_, '_>,
    ) -> Option<FrameNumber> {
        // TODO(Herschel): We should cache the labels in the preload step.
        let mut reader = self.reader(context);
        use swf::Tag;
        let mut frame_num = 1;
        while let Ok(Some(tag)) = reader.read_tag() {
            match tag {
                Tag::FrameLabel { label, .. } => {
                    if label == frame_label {
                        return Some(frame_num);
                    }
                }
                Tag::ShowFrame => frame_num += 1,
                _ => (),
            }
        }
        None
    }

    pub fn action(&self) -> Option<(usize, usize)> {
        self.action
    }

    pub fn run_goto_queue(&mut self, context: &mut UpdateContext<'_, 'gc, '_>) {
        let mut i = 0;
        while i < self.goto_queue.len() {
            let frame = self.goto_queue[i];
            if frame >= self.current_frame {
                // Advancing
                while self.current_frame + 1 < frame {
                    self.run_frame_internal(context, true);
                }
                self.run_frame_internal(context, false);
            } else {
                // Rewind
                // Reset everything to blank, start from frame 1,
                // and advance forward
                self.children.clear();
                self.tag_stream_pos = 0;
                self.current_frame = 0;
                while self.current_frame + 1 < frame {
                    self.run_frame_internal(context, true);
                }
                self.run_frame_internal(context, false);
            }

            i += 1;
        }

        self.goto_queue.clear();
    }

    pub fn place_object(
        &mut self,
        place_object: &swf::PlaceObject,
        context: &mut UpdateContext<'_, 'gc, '_>,
    ) {
        use swf::PlaceObjectAction;
        let character = match place_object.action {
            PlaceObjectAction::Place(id) => {
                // TODO(Herschel): Behavior when character doesn't exist/isn't a DisplayObject?
                let character = if let Ok(character) = context
                    .library
                    .instantiate_display_object(id, context.gc_context)
                {
                    character
                } else {
                    return;
                };

                // TODO(Herschel): Behavior when depth is occupied? (I think it replaces)
                self.children.insert(place_object.depth, character);
                self.children.get_mut(&place_object.depth).unwrap()
            }
            PlaceObjectAction::Modify => {
                if let Some(child) = self.children.get_mut(&place_object.depth) {
                    child
                } else {
                    return;
                }
            }
            PlaceObjectAction::Replace(id) => {
                let character = if let Ok(character) = context
                    .library
                    .instantiate_display_object(id, context.gc_context)
                {
                    character
                } else {
                    return;
                };

                let prev_character = self.children.insert(place_object.depth, character);
                let character = self.children.get_mut(&place_object.depth).unwrap();
                if let Some(prev_character) = prev_character {
                    character
                        .write(context.gc_context)
                        .set_matrix(prev_character.read().get_matrix());
                    character
                        .write(context.gc_context)
                        .set_color_transform(prev_character.read().get_color_transform());
                }
                character
            }
        };

        if let Some(matrix) = &place_object.matrix {
            let m = matrix.clone();
            character
                .write(context.gc_context)
                .set_matrix(&Matrix::from(m));
        }

        if let Some(color_transform) = &place_object.color_transform {
            character
                .write(context.gc_context)
                .set_color_transform(&ColorTransform::from(color_transform.clone()));
        }

        if let Some(name) = &place_object.name {
            character.write(context.gc_context).set_name(name);
        }

        if let Some(ratio) = &place_object.ratio {
            if let Some(morph_shape) = character.write(context.gc_context).as_morph_shape_mut() {
                morph_shape.set_ratio(*ratio);
            }
        }

        if let Some(clip_depth) = &place_object.clip_depth {
            character
                .write(context.gc_context)
                .set_clip_depth(*clip_depth);
        }
    }

    fn reader<'a>(
        &self,
        context: &UpdateContext<'a, '_, '_>,
    ) -> swf::read::Reader<std::io::Cursor<&'a [u8]>> {
        let mut cursor = std::io::Cursor::new(
            &context.swf_data[self.tag_stream_start as usize
                ..self.tag_stream_start as usize + self.tag_stream_len],
        );
        cursor.set_position(self.tag_stream_pos);
        swf::read::Reader::new(cursor, context.swf_version)
    }
    fn run_frame_internal(
        &mut self,
        context: &mut UpdateContext<'_, 'gc, '_>,
        only_display_actions: bool,
    ) {
        // Advance frame number.
        if self.current_frame < self.total_frames {
            self.current_frame += 1;
        } else {
            self.current_frame = 1;
            self.children.clear();
            self.tag_stream_pos = 0;
        }

        let mut tag_pos = self.tag_stream_pos;
        let mut reader = self.reader(context);

        loop {
            let frame_complete = if only_display_actions {
                self.run_goto_tag(context, &mut reader)
            } else {
                self.run_tag(context, &mut reader)
            };

            if frame_complete {
                break;
            }
        }

        self.tag_stream_pos = reader.get_ref().position();
    }

    fn run_tag(
        &mut self,
        context: &mut UpdateContext<'_, 'gc, '_>,
        reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>
    )-> bool {
        let (tag_code, tag_len) = if let Ok((tag_code, tag_len)) = reader.read_tag_code_and_length() {
            (tag_code, tag_len)
        } else {
            log::error!("ERROR");
            panic!();
        };
    
        let end_pos = reader.get_ref().position() + tag_len as u64;

        use swf::TagCode;
        let tag = TagCode::from_u16(tag_code);
        context.tag_len = tag_len;
        let mut ret = false;
        if let Some(tag) = tag {
            let result = match tag {
                TagCode::DoAction => self.do_action(context, reader),
                TagCode::PlaceObject => self.place_object_1(context, reader),
                TagCode::PlaceObject2 => self.place_object_2(context, reader),
                TagCode::PlaceObject3 => self.place_object_3(context, reader),
                TagCode::PlaceObject4 => self.place_object_4(context, reader),
                TagCode::RemoveObject => self.remove_object_1(context, reader),
                TagCode::RemoveObject2 => self.remove_object_2(context, reader),
                TagCode::SetBackgroundColor => self.set_background_color(context, reader),
                TagCode::SoundStreamBlock => self.sound_stream_block(context, reader),
                TagCode::SoundStreamHead => self.sound_stream_head_1(context, reader),
                TagCode::SoundStreamHead2 => self.sound_stream_head_2(context, reader),
                TagCode::StartSound => self.start_sound_1(context, reader),

                TagCode::End => {
                    reader.get_mut().set_position(0);
                    Ok(())
                },

                TagCode::ShowFrame => {
                    ret = true;
                    Ok(())
                },

                _ => Ok(()),
            };

            if let Err(e) = result {
                log::error!("Error running tag: {:?}", tag);
            }
        } else {
            log::warn!("Unknown tag code {}", tag_code);
        }

        use std::io::{Seek, SeekFrom};
        reader.get_mut().seek(SeekFrom::Start(end_pos));

        ret
    }

    fn run_goto_tag(
        &mut self,
        context: &mut UpdateContext<'_, 'gc, '_>,
        reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>
    ) -> bool {
        let (tag_code, tag_len) = reader.read_tag_code_and_length().unwrap();
        let end_pos = reader.get_ref().position() + tag_len as u64;

        use swf::TagCode;
        let tag = TagCode::from_u16(tag_code);
        context.tag_len = tag_len;
        let mut ret = false;
        if let Some(tag) = tag {
            let result = match tag {
                TagCode::DoAction => self.do_action(context, reader),
                TagCode::PlaceObject => self.place_object_1(context, reader),
                TagCode::PlaceObject2 => self.place_object_2(context, reader),
                TagCode::PlaceObject3 => self.place_object_3(context, reader),
                TagCode::PlaceObject4 => self.place_object_4(context, reader),
                TagCode::RemoveObject => self.remove_object_1(context, reader),
                TagCode::RemoveObject2 => self.remove_object_2(context, reader),

                TagCode::ShowFrame => {
                    ret = true;
                    Ok(())
                }

                _ => Ok(()),
            };

            if let Err(e) = result {
                log::error!("Error running tag: {:?}", tag);
            }
        } else {
            log::warn!("Unknown tag code {}", tag_code);
        }

        use std::io::{Seek, SeekFrom};
        reader.get_mut().seek(SeekFrom::Start(end_pos));

        ret
    }

    fn run_preload_tag(
        &mut self,
        context: &mut UpdateContext<'_, 'gc, '_>,
        reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>
    ) -> bool {
        let (tag_code, tag_len) = reader.read_tag_code_and_length().unwrap();
        let end_pos = reader.get_ref().position() + tag_len as u64;

        use swf::TagCode;
        let tag = TagCode::from_u16(tag_code);
        context.tag_len = tag_len;
        let mut ret = false;
        if let Some(tag) = tag {
            let result = match tag {
                TagCode::DefineButton => self.define_button_1(context, reader),
                TagCode::DefineButton2 => self.define_button_2(context, reader),
                TagCode::DefineBits => self.define_bits(context, reader),
                TagCode::DefineBitsJpeg2 => self.define_bits_jpeg_2(context, reader),
                TagCode::DefineBitsLossless => self.define_bits_lossless_1(context, reader),
                TagCode::DefineBitsLossless2 => self.define_bits_lossless_2(context, reader),
                TagCode::DefineFont => self.define_font_1(context, reader),
                TagCode::DefineFont2 => self.define_font_2(context, reader),
                TagCode::DefineFont3 => self.define_font_3(context, reader),
                TagCode::DefineMorphShape => self.define_morph_shape_1(context, reader),
                TagCode::DefineMorphShape2 => self.define_morph_shape_2(context, reader),
                TagCode::DefineShape => self.define_shape_1(context, reader),
                TagCode::DefineShape2 => self.define_shape_2(context, reader),
                TagCode::DefineShape3 => self.define_shape_3(context, reader),
                TagCode::DefineShape4 => self.define_shape_4(context, reader),
                TagCode::DefineSound => self.define_sound(context, reader),
                TagCode::DefineSprite => self.define_sprite(context, reader),
                TagCode::DefineText => self.define_text(context, reader),
                TagCode::JpegTables => self.jpeg_tables(context, reader),

                TagCode::End => {
                    ret = true;
                    Ok(())
                },
                _ => Ok(()),
            };

            if let Err(e) = result {
                log::error!("Error running tag: {:?}", tag);
            }
        } else {
            log::warn!("Unknown tag code {}", tag_code);
        }

        use std::io::{Seek, SeekFrom};
        reader.get_mut().seek(SeekFrom::Start(end_pos));

        ret
    }

    // Definition tag codes (preloading)
    fn define_bits(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        use std::io::Read;
        let id = reader.read_u16()?;
        if !context.library.contains_character(id) {
            let mut jpeg_data = Vec::with_capacity(context.tag_len - 2);
            reader.get_mut().take(context.tag_len as u64 - 2).read_to_end(&mut jpeg_data)?;
            let handle = context.renderer.register_bitmap_jpeg(
                id,
                &jpeg_data,
                context.library.jpeg_tables().unwrap(),
            );
            context
                .library
                .register_character(id, Character::Bitmap(handle));
        }
        Ok(())
    }

    fn define_bits_jpeg_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        use std::io::Read;
        let id = reader.read_u16()?;
        if !context.library.contains_character(id) {
            let mut jpeg_data = Vec::with_capacity(context.tag_len - 2);
            reader.get_mut().take(context.tag_len as u64 - 2).read_to_end(&mut jpeg_data)?;
            let handle = context.renderer.register_bitmap_jpeg_2(
                id,
                &jpeg_data
            );
            context
                .library
                .register_character(id, Character::Bitmap(handle));
        }
        Ok(())
    }

    fn define_bits_lossless(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>, version: u8)  -> Result<(), Box<std::error::Error>> {
        let define_bits_lossless = reader.read_define_bits_lossless(version)?;
        if !context.library.contains_character(define_bits_lossless.id) {
            let handle = context.renderer.register_bitmap_png(&define_bits_lossless);
            context
                .library
                .register_character(define_bits_lossless.id, Character::Bitmap(handle));
        }
        Ok(())
    }

    fn define_bits_lossless_1(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_bits_lossless(context, reader, 1)
    }

    fn define_bits_lossless_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_bits_lossless(context, reader, 2)
    }

    fn define_button_1(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let swf_button = reader.read_define_button_1()?;
        if !context.library.contains_character(swf_button.id) {
            let button = crate::button::Button::from_swf_tag(
                &swf_button,
                &context.library,
                context.gc_context,
            );
            context
                .library
                .register_character(swf_button.id, Character::Button(Box::new(button)));
        }
        Ok(())
    }

    fn define_button_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let swf_button = reader.read_define_button_2()?;
        if !context.library.contains_character(swf_button.id) {
            let button = crate::button::Button::from_swf_tag(
                &swf_button,
                &context.library,
                context.gc_context,
            );
            context
                .library
                .register_character(swf_button.id, Character::Button(Box::new(button)));
        }
        Ok(())
    }

    fn define_font_1(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let font = reader.read_define_font_1()?;
        if !context.library.contains_character(font.id) {
            let glyphs = font
                .glyphs
                .into_iter()
                .map(|g| swf::Glyph {
                    shape_records: g,
                    code: 0,
                    advance: None,
                    bounds: None,
                })
                .collect::<Vec<_>>();

            let font = swf::Font {
                id: font.id,
                version: 0,
                name: "".to_string(),
                glyphs,
                language: swf::Language::Unknown,
                layout: None,
                is_small_text: false,
                is_shift_jis: false,
                is_ansi: false,
                is_bold: false,
                is_italic: false,
            };
            let font_object = Font::from_swf_tag(context, &font).unwrap();
            context
                .library
                .register_character(font.id, Character::Font(Box::new(font_object)));
        }

        Ok(())
    }

    fn define_font_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let font = reader.read_define_font_2(2)?;
        if !context.library.contains_character(font.id) {
            let font_object = Font::from_swf_tag(context, &font).unwrap();
            context
                .library
                .register_character(font.id, Character::Font(Box::new(font_object)));
        }

        Ok(())
    }

    fn define_font_3(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let font = reader.read_define_font_2(3)?;
        if !context.library.contains_character(font.id) {
            let font_object = Font::from_swf_tag(context, &font).unwrap();
            context
                .library
                .register_character(font.id, Character::Font(Box::new(font_object)));
        }

        Ok(())
    }

    fn define_morph_shape(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>, version: u8)  -> Result<(), Box<std::error::Error>> {
        let swf_shape = reader.read_define_morph_shape(version)?;
        if !context.library.contains_character(swf_shape.id) {
            let morph_shape = MorphShape::from_swf_tag(&swf_shape, context);
            context.library.register_character(
                swf_shape.id,
                Character::MorphShape(Box::new(morph_shape)),
            );
        }
        Ok(())
    }

    fn define_morph_shape_1(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_morph_shape(context, reader, 1)
    }

    fn define_morph_shape_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_morph_shape(context, reader, 2)
    }

    fn define_shape(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>, version: u8)  -> Result<(), Box<std::error::Error>> {
        let swf_shape = reader.read_define_shape(version)?;
        if !context.library.contains_character(swf_shape.id) {
            let graphic = Graphic::from_swf_tag(&swf_shape, context);
            context.library.register_character(
                swf_shape.id,
                Character::Graphic(Box::new(graphic)),
            );
        }
        Ok(())
    }

    fn define_shape_1(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_shape(context, reader, 1)
    }

    fn define_shape_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_shape(context, reader, 2)
    }

    fn define_shape_3(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_shape(context, reader, 3)
    }

    fn define_shape_4(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        self.define_shape(context, reader, 4)
    }

    fn define_sound(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        // TODO(Herschel): Can we use a slice of the sound data instead of copying the data?
        let sound = reader.read_define_sound()?;
        if !context.library.contains_character(sound.id) {
            let handle = context.audio.register_sound(&sound).unwrap();
            context
                .library
                .register_character(sound.id, Character::Sound(handle));
        }
        Ok(())
    }

    fn define_sprite(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let id = reader.read_character_id()?;
        let num_frames = reader.read_u16()?;
        if !context.library.contains_character(id) {
            let mut movie_clip =
                MovieClip::new_with_data(reader.get_ref().position(), context.tag_len - 4, num_frames);

            movie_clip.preload(context);

            context.library.register_character(
                id,
                Character::MovieClip(Box::new(movie_clip)),
            );
        }

        Ok(())
    }

    fn define_text(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let text = reader.read_define_text()?;
        if !context.library.contains_character(text.id) {
            let text_object = Text::from_swf_tag(&text);
            context
                .library
                .register_character(text.id, Character::Text(Box::new(text_object)));
        }
        Ok(())
    }

    fn jpeg_tables(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        use std::io::Read;
        // TODO(Herschel): Can we use a slice instead of copying?
        let mut jpeg_data = Vec::with_capacity(context.tag_len);
        reader.get_mut().take(context.tag_len as u64).read_to_end(&mut jpeg_data)?;
        context.library.set_jpeg_tables(jpeg_data);
        Ok(())
    }

    fn preload_place_object(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        // use swf::PlaceObjectAction;
        // match place_object.action {
        //     PlaceObjectAction::Place(id) | PlaceObjectAction::Replace(id) => {
        //         ids.insert(place_object.depth, id);
        //     }
        //     _ => (),
        // }
        // if let Some(ratio) = place_object.ratio {
        //     if let Some(&id) = ids.get(&place_object.depth) {
        //         if let Some(Character::MorphShape(morph_shape)) =
        //             context.library.get_character_mut(id)
        //         {
        //             morph_shape.register_ratio(context.renderer, ratio);
        //         }
        //     }
        // }
        Ok(())
    }

    // Control tag codes
    fn do_action(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        Ok(())
    }

    fn place_object_1(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let place_object = reader.read_place_object()?;
        self.place_object(&place_object, context);
        Ok(())
    }

    fn place_object_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let place_object = reader.read_place_object_2_or_3(2)?;
        self.place_object(&place_object, context);
        Ok(())
    }

    fn place_object_3(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let place_object = reader.read_place_object_2_or_3(3)?;
        self.place_object(&place_object, context);
        Ok(())
    }

    fn place_object_4(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let place_object = reader.read_place_object_2_or_3(4)?;
        self.place_object(&place_object, context);
        Ok(())
    }

    fn remove_object_1(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let remove_object = reader.read_remove_object_1()?;
        // TODO(Herschel): How does the character ID work for RemoveObject1?
        // Verify what happens when the character ID doesn't match.
        self.children.remove(&remove_object.depth);
        Ok(())
    }

    fn remove_object_2(&mut self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>)  -> Result<(), Box<std::error::Error>> {
        let remove_object = reader.read_remove_object_2()?;
        self.children.remove(&remove_object.depth);
        Ok(())
    }

    fn set_background_color(&self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>) -> Result<(), Box<std::error::Error>> {
        *context.background_color = reader.read_rgb()?;
        Ok(())
    }

    fn sound_stream_block(&self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>) -> Result<(), Box<std::error::Error>> {
        // TODO
        Ok(())
    }

    fn sound_stream_head_1(&self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>) -> Result<(), Box<std::error::Error>> {
        // TODO
        Ok(())
    }

    fn sound_stream_head_2(&self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>) -> Result<(), Box<std::error::Error>> {
        // TODO
        Ok(())
    }

    fn start_sound_1(&self, context: &mut UpdateContext<'_, 'gc, '_>, reader: &mut swf::read::Reader<std::io::Cursor<&'_ [u8]>>) -> Result<(), Box<std::error::Error>> {
        let start_sound = reader.read_start_sound_1()?;
        if let Some(handle) = context.library.get_sound(start_sound.id) {
            context.audio.play_sound(handle);
        }
        Ok(())
    }
}

impl<'gc> DisplayObject<'gc> for MovieClip<'gc> {
    impl_display_object!(base);

    fn preload(&mut self, context: &mut UpdateContext<'_, 'gc, '_>) {
        let mut reader = self.reader(context);
        loop {
            let complete = self.run_preload_tag(context, &mut reader);
            if complete {
                break;
            }
        }
    }

    fn run_frame(&mut self, context: &mut UpdateContext<'_, 'gc, '_>) {
        self.action = None;

        if self.is_playing {
            self.run_frame_internal(context, false);
        }

        // TODO(Herschel): Verify order of execution for parent/children.
        // Parent first? Children first? Sorted by depth?
        for child in self.children.values_mut() {
            child.write(context.gc_context).run_frame(context);
        }
    }

    fn run_post_frame(&mut self, context: &mut UpdateContext<'_, 'gc, '_>) {
        self.run_goto_queue(context);

        for child in self.children.values() {
            child.write(context.gc_context).run_post_frame(context);
        }
    }

    fn render(&self, context: &mut RenderContext<'_, 'gc>) {
        context.transform_stack.push(self.transform());

        for child in self.children.values() {
            child.read().render(context);
        }

        context.transform_stack.pop();
    }

    fn handle_click(&mut self, _pos: (f32, f32)) {
        // for child in self.children.values_mut() {
        //     child.handle_click(pos);
        // }
    }
    fn as_movie_clip(&self) -> Option<&crate::movie_clip::MovieClip<'gc>> {
        Some(self)
    }

    fn as_movie_clip_mut(&mut self) -> Option<&mut crate::movie_clip::MovieClip<'gc>> {
        Some(self)
    }
}

impl Default for MovieClip<'_> {
    fn default() -> Self {
        MovieClip::new()
    }
}

unsafe impl<'gc> gc_arena::Collect for MovieClip<'gc> {
    #[inline]
    fn trace(&self, cc: gc_arena::CollectionContext) {
        for child in self.children.values() {
            child.trace(cc);
        }
    }
}

struct PreloadTagDecoder {
    ids: fnv::FnvHashMap<Depth, CharacterId>,
}

impl<'a, 'gc> PreloadTagDecoder {
    fn define_bits_lossless(&mut self, context: &mut DecoderContext<'a, 'gc>, version: u8) -> DecoderResult {
        let define_bits_lossless = reader.read_define_bits_lossless(version)?;
        if !context.library.contains_character(define_bits_lossless.id) {
            let handle = context.renderer.register_bitmap_png(&define_bits_lossless);
            context
                .library
                .register_character(define_bits_lossless.id, Character::Bitmap(handle));
        }
        Ok(())
    }

    fn place_object<'a, 'gc>(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        use swf::PlaceObjectAction;
        match place_object.action {
            PlaceObjectAction::Place(id) | PlaceObjectAction::Replace(id) => {
                ids.insert(place_object.depth, id);
            }
            _ => (),
        }
        if let Some(ratio) = place_object.ratio {
            if let Some(&id) = ids.get(&place_object.depth) {
                if let Some(Character::MorphShape(morph_shape)) =
                    context.library.get_character_mut(id)
                {
                    morph_shape.register_ratio(context.renderer, ratio);
                }
            }
        }
        Ok(())
    }
}


impl<'a, 'gc> crate::tag_decoder::TagDecoder<'a, 'gc>  for PreloadTagDecoder {
    fn stop_at_tag(&self, tag_code: swf::TagCode) -> bool {
        tag_code == swf::TagCode::End
    }
    
    fn define_bits(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        use std::io::Read;
        let id = context.reader.read_u16()?;
        if !context.library.contains_character(id) {
            let data_len = context.tag_len - 2;
            let mut jpeg_data = Vec::with_capacity(data_len);
            context.reader.get_mut().take(data_len as u64).read_to_end(&mut jpeg_data)?;
            let handle = context.renderer.register_bitmap_jpeg(
                id,
                &jpeg_data,
                context.library.jpeg_tables().unwrap(),
            );
            context
                .library
                .register_character(id, Character::Bitmap(handle));
        }
        Ok(())
    }

    fn define_bits_jpeg_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        use std::io::Read;
        let id = context.reader.read_u16()?;
        if !context.library.contains_character(id) {
            let data_len = context.tag_len - 2;
            let mut jpeg_data = Vec::with_capacity(data_len);
            context.reader.get_mut().take(data_len as u64).read_to_end(&mut jpeg_data)?;
            let handle = context.renderer.register_bitmap_jpeg_2(
                id,
                &jpeg_data
            );
            context
                .library
                .register_character(id, Character::Bitmap(handle));
        }
        Ok(())
    }

    fn define_bits_lossless_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_bits_lossless(context, 1)
    }

    fn define_bits_lossless_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_bits_lossless(context, 2)
    }

    fn define_button_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        let swf_button = context.reader.read_define_button_1()?;
        if !context.library.contains_character(swf_button.id) {
            let button = crate::button::Button::from_swf_tag(
                &swf_button,
                &context.library,
                context.gc_context,
            );
            context
                .library
                .register_character(swf_button.id, Character::Button(Box::new(button)));
        }
        Ok(())
    }

    fn define_button_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        let swf_button = context.reader.read_define_button_2()?;
        if !context.library.contains_character(swf_button.id) {
            let button = crate::button::Button::from_swf_tag(
                &swf_button,
                &context.library,
                context.gc_context,
            );
            context
                .library
                .register_character(swf_button.id, Character::Button(Box::new(button)));
        }
        Ok(())
    }

    fn define_font_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        let font = context.reader.read_define_font_1()?;
        if !context.library.contains_character(font.id) {
            let glyphs = font
                .glyphs
                .into_iter()
                .map(|g| swf::Glyph {
                    shape_records: g,
                    code: 0,
                    advance: None,
                    bounds: None,
                })
                .collect::<Vec<_>>();

            let font = swf::Font {
                id: font.id,
                version: 0,
                name: "".to_string(),
                glyphs,
                language: swf::Language::Unknown,
                layout: None,
                is_small_text: false,
                is_shift_jis: false,
                is_ansi: false,
                is_bold: false,
                is_italic: false,
            };
            let font_object = Font::from_swf_tag(context, &font).unwrap();
            context
                .library
                .register_character(font.id, Character::Font(Box::new(font_object)));
        }
        Ok(())
    }

    fn define_font_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        let font = context.reader.read_define_font_2(2)?;
        if !context.library.contains_character(font.id) {
            let font_object = Font::from_swf_tag(context, &font).unwrap();
            context
                .library
                .register_character(font.id, Character::Font(Box::new(font_object)));
        }
        Ok(())
    }

    fn define_font_3(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        let font = context.reader.read_define_font_2(3)?;
        if !self.context.library.contains_character(font.id) {
            let font_object = Font::from_swf_tag(self.context, &font).unwrap();
            self.context
                .library
                .register_character(font.id, Character::Font(Box::new(font_object)));
        }

        Ok(())
    }

    fn define_morph_shape(&mut self, version: u8) -> DecoderResult {
        let swf_shape = self.reader.read_define_morph_shape(version)?;
        if !self.context.library.contains_character(swf_shape.id) {
            let morph_shape = MorphShape::from_swf_tag(&swf_shape, self.context);
            self.context.library.register_character(
                swf_shape.id,
                Character::MorphShape(Box::new(morph_shape)),
            );
        }
        Ok(())
    }

    fn define_morph_shape_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_morph_shape(self.context, self.reader, 1)
    }

    fn define_morph_shape_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_morph_shape(self.context, self.reader, 2)
    }

    // fn define_shape(&mut self, version: u8) -> DecoderResult {
    //     let swf_shape = reader.read_define_shape(version)?;
    //     if !self.context.library.contains_character(swf_shape.id) {
    //         let graphic = Graphic::from_swf_tag(&swf_shape, self.context);
    //         self.context.library.register_character(
    //             swf_shape.id,
    //             Character::Graphic(Box::new(graphic)),
    //         );
    //     }
    //     Ok(())
    // }

    fn define_shape_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_shape(self.context, self.reader, 1)
    }

    fn define_shape_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_shape(self.context, self.reader, 2)
    }

    fn define_shape_3(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_shape(self.context, self.reader, 3)
    }

    fn define_shape_4(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        self.define_shape(self.context, self.reader, 4)
    }

    fn define_sound(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        // TODO(Herschel): Can we use a slice of the sound data instead of copying the data?
        let sound = self.reader.read_define_sound()?;
        if !self.context.library.contains_character(sound.id) {
            let handle = self.context.audio.register_sound(&sound).unwrap();
            self.context
                .library
                .register_character(sound.id, Character::Sound(handle));
        }
        Ok(())
    }

    fn define_sprite(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        let id = self.reader.read_character_id()?;
        let num_frames = self.reader.read_u16()?;
        if !self.context.library.contains_character(id) {
            let mut movie_clip =
                MovieClip::new_with_data(self.reader.get_ref().position(), self.context.tag_len - 4, num_frames);

            movie_clip.preload(self.context);

            self.context.library.register_character(
                id,
                Character::MovieClip(Box::new(movie_clip)),
            );
        }

        Ok(())
    }

    fn define_text(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        let text = self.reader.read_define_text()?;
        if !self.context.library.contains_character(text.id) {
            let text_object = Text::from_swf_tag(&text);
            self.context
                .library
                .register_character(text.id, Character::Text(Box::new(text_object)));
        }
        Ok(())
    }

    fn jpeg_tables(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {
        use std::io::Read;
        // TODO(Herschel): Can we use a slice instead of copying?
        let mut jpeg_data = Vec::with_capacity(self.context.tag_len);
        self.reader.get_mut().take(self.context.tag_len as u64).read_to_end(&mut jpeg_data)?;
        self.context.library.set_jpeg_tables(jpeg_data);
        Ok(())
    }
}