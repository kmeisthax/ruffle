#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DecoderStatus {
    Continue,
    Stop,
}

pub struct DecoderContext<'a, 'gc> {
    pub background_color: &'a mut crate::prelude::Color,
    pub library: std::cell::RefMut<'a, crate::library::Library<'gc>>,
    pub reader: &'a mut swf::read::Reader<std::io::Cursor<&'a [u8]>>
    pub renderer: &'a mut crate::backend::render::RenderBackend,
    pub swf_version: u8,
    pub tag_len: usize,
}

pub fn decode_tags<'a, 'gc, 'gc_context, T: TagDecoder<'a, 'gc>>(context: &'a mut crate::player::UpdateContext<'a, 'gc, 'gc_context>, reader: &'a mut swf::read::Reader<std::io::Cursor<&'a [u8]>>, decoder: T) -> Result<(), Box<std::error::Error>> {
    let mut decoder_context = DecoderContext {
        reader,
        swf_version: context.swf_version,
        library: context.library,
        background_color: context.background_color,
        renderer: context.renderer,
        tag_len: 0,
    };
    while let decode_tag(&mut decoder_context)? == DecoderStats::Continue {}
}

fn decode_tag<'a, 'gc, T: TagDecoder<'a, 'gc>>(context: &'a mut DecoderContext<'a, 'gc>, decoder: T) -> Result<(), Box<std::error::Error>> {
    let (tag_code, tag_len) = context.reader.read_tag_code_and_length()?;

    let end_pos = context.reader.get_ref().position() + tag_len as u64;

    use swf::TagCode;
    let tag = TagCode::from_u16(tag_code);
    context.tag_len = tag_len;
    let mut ret = DecoderStatus::Continue;
    if let Some(tag) = tag {
        let result = match tag {
            TagCode::DefineButton => decoder.define_button_1(context),
            TagCode::DefineButton2 => decoder.define_button_2(context),
            TagCode::DefineBits => decoder.define_bits(context),
            TagCode::DefineBitsJpeg2 => decoder.define_bits_jpeg_2(context),
            TagCode::DefineBitsLossless => decoder.define_bits_lossless_1(context),
            TagCode::DefineBitsLossless2 => decoder.define_bits_lossless_2(context),
            TagCode::DefineFont => decoder.define_font_1(context),
            TagCode::DefineFont2 => decoder.define_font_2(context),
            TagCode::DefineFont3 => decoder.define_font_3(context),
            TagCode::DefineMorphShape => decoder.define_morph_shape_1(context),
            TagCode::DefineMorphShape2 => decoder.define_morph_shape_2(context),
            TagCode::DefineShape => decoder.define_shape_1(context),
            TagCode::DefineShape2 => decoder.define_shape_2(context),
            TagCode::DefineShape3 => decoder.define_shape_3(context),
            TagCode::DefineShape4 => decoder.define_shape_4(context),
            TagCode::DefineSound => decoder.define_sound(context),
            TagCode::DefineSprite => decoder.define_sprite(context),
            TagCode::DefineText => decoder.define_text(context),
            TagCode::DoAction => decoder.do_action(context),
            TagCode::End => decoder.end(context),
            TagCode::JpegTables => decoder.jpeg_tables(context),
            TagCode::PlaceObject => decoder.place_object_1(context),
            TagCode::PlaceObject2 => decoder.place_object_2(context),
            TagCode::PlaceObject3 => decoder.place_object_3(context),
            TagCode::PlaceObject4 => decoder.place_object_4(context),
            TagCode::RemoveObject => decoder.remove_object_1(context),
            TagCode::RemoveObject2 => decoder.remove_object_2(context),
            TagCode::SetBackgroundColor => decoder.set_background_color(context),
            TagCode::ShowFrame => decoder.show_frame(context),
            TagCode::SoundStreamBlock => decoder.sound_stream_block(context),
            TagCode::SoundStreamHead => decoder.sound_stream_head_1(context),
            TagCode::SoundStreamHead2 => decoder.sound_stream_head_2(context),
            TagCode::StartSound => decoder.start_sound_1(context),
            _ => Ok(()), // TODO
        };

        if let Err(e) = result {
            log::error!("Error running tag: {:?}", tag);
        }

        if decoder.stop_at_tag(tag) {
            ret = DecoderStatus::Stop;
        }
    } else {
        log::warn!("Unknown tag code {}", tag_code);
    }

    use std::io::{Seek, SeekFrom};
    context.reader.get_mut().seek(SeekFrom::Start(end_pos));

    Ok(())
}

pub type DecoderResult = std::result::Result<(), Box<std::error::Error>>;
pub trait TagDecoder<'a, 'gc> {
    fn define_bits(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }
    fn define_bits_jpeg_2(&mut self, context: &mut DecoderContext<'a, 'gc>)  -> DecoderResult {  }
    fn define_bits_lossless_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }
    fn define_bits_lossless_2(&mut self, context: &mut DecoderContext<'a, 'gc>)  -> DecoderResult {  }
    fn define_button_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  } 
    fn define_button_2(&mut self, context: &mut DecoderContext<'a, 'gc>)  -> DecoderResult {  }  
    fn define_font_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_font_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_font_3(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }
    fn define_morph_shape_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }
    fn define_morph_shape_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }
    fn define_shape_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_shape_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_shape_3(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_shape_4(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_sound(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_sprite(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn define_text(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn end(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn jpeg_tables(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn do_action(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn place_object_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn place_object_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn place_object_3(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn place_object_4(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn remove_object_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn remove_object_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn set_background_color(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn show_frame(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn sound_stream_block(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn sound_stream_head_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn sound_stream_head_2(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn start_sound_1(&mut self, context: &mut DecoderContext<'a, 'gc>) -> DecoderResult {  }  
    fn unknown(&mut self, context: &mut DecoderContext<'a, 'gc>, tag_code: u16) -> DecoderResult {  }

    fn stop_at_tag(&self, tag: swf::TagCode) -> bool {
        tag == swf::TagCode::End
    }
}
