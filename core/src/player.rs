use crate::audio::Audio;
use crate::avm1::Avm1;
use crate::backend::{audio::AudioBackend, render::RenderBackend};
use crate::display_object::DisplayObject;
use crate::library::Library;
use crate::movie_clip::MovieClip;
use crate::prelude::*;
use crate::transform::TransformStack;
use log::info;
use std::io::Cursor;

type CharacterId = swf::CharacterId;

pub struct Player {
    tag_stream: swf::read::Reader<Cursor<Vec<u8>>>,

    //avm: Avm1,
    audio: Audio,
    renderer: Box<RenderBackend>,
    transform_stack: TransformStack,

    library: Library,
    stage: Box<DisplayObject>,
    background_color: Color,

    frame_rate: f64,
    frame_accumulator: f64,
    global_time: u64,

    movie_width: u32,
    movie_height: u32,

    mouse_pos: (f32, f32),
}

impl Player {
    pub fn new(
        mut renderer: Box<RenderBackend>,
        audio: Box<AudioBackend>,
        swf_data: Vec<u8>,
    ) -> Result<Player, Box<std::error::Error>> {
        let (header, mut reader) = swf::read::read_swf_header(&swf_data[..]).unwrap();
        // Decompress the entire SWF in memory.
        let mut data = Vec::new();
        reader.get_mut().read_to_end(&mut data)?;
        let tag_stream = swf::read::Reader::new(Cursor::new(data), header.version);

        info!("{}x{}", header.stage_size.x_max, header.stage_size.y_max);

        let stage = MovieClip::new_with_data(0, header.num_frames);
        let movie_width = (header.stage_size.x_max - header.stage_size.x_min).to_pixels() as u32;
        let movie_height = (header.stage_size.y_max - header.stage_size.y_min).to_pixels() as u32;
        renderer.set_dimensions(movie_width, movie_height);

        let mut player = Player {
            tag_stream,

            // avm: Avm1::new(header.version),
            renderer,
            audio: Audio::new(audio),

            background_color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            transform_stack: TransformStack::new(),

            library: Library::new(),
            stage: Box::new(stage),

            frame_rate: header.frame_rate.into(),
            frame_accumulator: 0.0,
            global_time: 0,

            movie_width,
            movie_height,

            mouse_pos: (0.0, 0.0),
        };

        player.preload();

        Ok(player)
    }

    pub fn tick(&mut self, dt: f64) {
        self.frame_accumulator += dt;
        self.global_time += dt as u64;
        let frame_time = 1000.0 / self.frame_rate;

        let needs_render = self.frame_accumulator >= frame_time;
        while self.frame_accumulator >= frame_time {
            self.frame_accumulator -= frame_time;
            self.run_frame();
        }

        if needs_render {
            self.render();
        }
    }

    pub fn movie_width(&self) -> u32 {
        self.movie_width
    }

    pub fn movie_height(&self) -> u32 {
        self.movie_height
    }

    pub fn mouse_move(&mut self, pos: (f32, f32)) {
        self.mouse_pos = pos;
    }

    pub fn mouse_down(&mut self) {}

    pub fn mouse_up(&mut self) {
        self.stage.handle_click(self.mouse_pos);
    }

    fn preload(&mut self) {
        let mut update_context = UpdateContext {
            global_time: self.global_time,
            mouse_pos: self.mouse_pos,
            tag_stream: &mut self.tag_stream,
            position_stack: vec![],
            library: &mut self.library,
            background_color: &mut self.background_color,
            //avm1: &mut self.avm,
            renderer: &mut *self.renderer,
            audio: &mut self.audio,
            action: None,
        };

        self.stage.preload(&mut update_context);
    }

    fn run_frame(&mut self) {
        let mut update_context = UpdateContext {
            global_time: self.global_time,
            mouse_pos: self.mouse_pos,
            tag_stream: &mut self.tag_stream,
            position_stack: vec![],
            library: &mut self.library,
            background_color: &mut self.background_color,
            //avm1: &mut self.avm,
            renderer: &mut *self.renderer,
            audio: &mut self.audio,
            action: None,
        };

        self.stage.run_frame(&mut update_context);
        self.stage.run_post_frame(&mut update_context)
    }

    fn render(&mut self) {
        self.renderer.begin_frame();

        self.renderer.clear(self.background_color.clone());

        {
            let mut render_context = RenderContext {
                renderer: &mut *self.renderer,
                library: &self.library,
                transform_stack: &mut self.transform_stack,
            };
            self.stage.render(&mut render_context);
        }

        self.renderer.end_frame();
    }
}

pub struct UpdateContext<'a> {
    pub global_time: u64,
    pub mouse_pos: (f32, f32),
    pub tag_stream: &'a mut swf::read::Reader<Cursor<Vec<u8>>>,
    pub position_stack: Vec<u64>,
    pub library: &'a mut Library,
    pub background_color: &'a mut Color,
    //pub avm1: &'a mut Avm1,
    pub renderer: &'a mut RenderBackend,
    pub audio: &'a mut Audio,
    pub action: Option<(usize, usize)>,
}

pub struct RenderContext<'a> {
    pub renderer: &'a mut RenderBackend,
    pub library: &'a Library,
    pub transform_stack: &'a mut TransformStack,
}
