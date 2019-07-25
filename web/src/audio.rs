use generational_arena::Arena;
use ruffle_core::backend::audio::decoders::{stream_tag_reader, AdpcmDecoder, Mp3Decoder};
use ruffle_core::backend::audio::{swf, AudioBackend, AudioStreamHandle, SoundHandle};
use std::cell::RefCell;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::AudioContext;

thread_local! {
    static STREAMS: RefCell<Arena<AudioStream>> = RefCell::new(Arena::new());
}

struct Sound {
    format: swf::SoundFormat,
    data: Vec<u8>,
}

pub struct WebAudioBackend {
    context: AudioContext,
    sounds: Arena<Sound>,
}

type Decoder = Box<dyn Iterator<Item=i16>>;
struct AudioStream {
    decoder: Decoder,
    is_stereo: bool,
    left_samples: Vec<f32>,
    right_samples: Vec<f32>,
}

type Error = Box<std::error::Error>;

impl WebAudioBackend {
    pub fn new() -> Result<Self, Error> {
        let context = AudioContext::new().map_err(|_| "Unable to create AudioContext")?;
        log::info!("sample rate: {}", context.sample_rate());
        Ok(Self {
            context,
            sounds: Arena::new(),
        })
    }

    fn update_script_processor(
        audio_stream: &mut AudioStream,
        event: web_sys::AudioProcessingEvent,
    ) -> bool {
        let output_buffer = event.output_buffer().unwrap();
        let num_frames = output_buffer.length() as usize;
        let mut complete = false;
        audio_stream.left_samples.clear();
        audio_stream.right_samples.clear();
        for _ in 0..num_frames {
            if let (Some(l), Some(r)) = (audio_stream.decoder.next(), audio_stream.decoder.next()) {
                audio_stream.left_samples.push(f32::from(l) / 32767.0);
                if audio_stream.is_stereo {
                    audio_stream.right_samples.push(f32::from(r) / 32767.0);
                }
            } else {
                complete = true;
                break;
            }
        }
        output_buffer.copy_to_channel(&mut audio_stream.left_samples[..], 0).unwrap();
        if audio_stream.is_stereo {
            output_buffer.copy_to_channel(&mut audio_stream.right_samples[..], 1).unwrap();
        }
        complete
    }
}

impl AudioBackend for WebAudioBackend {
    fn register_sound(&mut self, sound: &swf::Sound) -> Result<SoundHandle, Error> {
        Ok(self.sounds.insert(Sound {
            format: sound.format.clone(),
            data: sound.data.clone(),
        }))
    }

    fn play_sound(&mut self, sound: SoundHandle) {
        if let Some(sound) = self.sounds.get(sound) {
            use swf::AudioCompression;
            let decoder: Decoder = match sound.format.compression {
                AudioCompression::Adpcm => Box::new(AdpcmDecoder::new(
                    std::io::Cursor::new(sound.data.to_vec()),
                        sound.format.is_stereo,
                        sound.format.sample_rate
                ).unwrap()),
                AudioCompression::Mp3 => Box::new(Mp3Decoder::new(
                    if sound.format.is_stereo {
                        2
                    } else {
                        1
                    },
                    sound.format.sample_rate.into(),
                    std::io::Cursor::new(sound.data.to_vec())//&sound.data[..]
                )),
                _ => unimplemented!()
            };

            log::info!("{} {}", sound.format.sample_rate, self.context.sample_rate());
            let decoder: Decoder = if sound.format.sample_rate != self.context.sample_rate() as u16 {
                Box::new(resample(decoder, sound.format.sample_rate, self.context.sample_rate() as u16, sound.format.is_stereo))
            } else {
                decoder
            };

            let audio_stream = AudioStream {
                decoder,
                left_samples: vec![],
                right_samples: vec![],
                is_stereo: sound.format.is_stereo,
            };

            let stream_handle = STREAMS.with(|streams| {
                let mut streams = streams.borrow_mut();
                streams.insert(audio_stream)
            });

            let script_processor_node = self.context.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(4096, 0, if sound.format.is_stereo { 2 } else { 1 }).unwrap();
            let script_node = script_processor_node.clone();
            let f = Closure::wrap(Box::new(move |event| {
                STREAMS.with(|streams| {
                    let mut streams = streams.borrow_mut();
                    let complete = if let Some(audio_stream) = streams.get_mut(stream_handle) {
                        WebAudioBackend::update_script_processor(audio_stream, event)
                    } else {
                        false
                    };
                    if complete {
                        streams.remove(stream_handle);
                        script_node.disconnect().unwrap();
                    }
                })
            }) as Box<FnMut(web_sys::AudioProcessingEvent)>);
            script_processor_node.set_onaudioprocess(Some(f.as_ref().unchecked_ref()));
            f.forget();

            script_processor_node
                .connect_with_audio_node(&self.context.destination())
                .unwrap();
        }
    }

    fn start_stream(
        &mut self,
        _clip_id: swf::CharacterId,
        clip_data: ruffle_core::tag_utils::SwfSlice,
        stream_info: &swf::SoundStreamHead,
    ) -> AudioStreamHandle {
        let decoder = Mp3Decoder::new(
            if stream_info.stream_format.is_stereo {
                2
            } else {
                1
            },
            stream_info.stream_format.sample_rate.into(),
            stream_tag_reader(clip_data),
        );

        let decoder: Decoder = if stream_info.stream_format.sample_rate != self.context.sample_rate() as u16 {
            Box::new(resample(decoder, stream_info.stream_format.sample_rate, self.context.sample_rate() as u16, stream_info.stream_format.is_stereo))
        } else {
            Box::new(decoder)
        };

        let audio_stream = AudioStream {
            decoder: Box::new(decoder),
            left_samples: vec![],
            right_samples: vec![],
            is_stereo: stream_info.stream_format.is_stereo,
        };

        let stream_handle = STREAMS.with(|streams| {
            let mut streams = streams.borrow_mut();
            streams.insert(audio_stream)
        });

        let script_processor_node = self.context.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(4096, 0, if stream_info.stream_format.is_stereo { 2 } else { 1 }).unwrap();
        let script_node = script_processor_node.clone();
        let f = Closure::wrap(Box::new(move |event| {
            STREAMS.with(|streams| {
                let mut streams = streams.borrow_mut();
                let complete = if let Some(audio_stream) = streams.get_mut(stream_handle) {
                    WebAudioBackend::update_script_processor(audio_stream, event)
                } else {
                    false
                };
                if complete {
                    streams.remove(stream_handle);
                    script_node.disconnect().unwrap();
                }
            })
        }) as Box<FnMut(web_sys::AudioProcessingEvent)>);
        script_processor_node.set_onaudioprocess(Some(f.as_ref().unchecked_ref()));
        f.forget();

        script_processor_node
            .connect_with_audio_node(&self.context.destination())
            .unwrap();

        stream_handle
    }
}

fn resample(mut input: impl Iterator<Item=i16>, input_sample_rate: u16, output_sample_rate: u16, is_stereo: bool) -> impl Iterator<Item=i16> {
    let (mut left0, mut right0) = if is_stereo {
        (input.next(), input.next())
    } else {
        let sample = input.next();
        (sample, sample)
    };
    let (mut left1, mut right1) = if is_stereo {
        (input.next(), input.next())
    } else {
        let sample = input.next();
        (sample, sample)
    };
    let (mut left, mut right) = (left0.unwrap(), right0.unwrap());
    let dt_input = 1.0 / f64::from(input_sample_rate);
    let dt_output = 1.0 / f64::from(output_sample_rate);
    let mut t = 0.0;
    let mut cur_channel = 0;
    std::iter::from_fn(move || {
        if cur_channel == 1 {
            cur_channel = 0;
            return Some(right);
        }
        if let (Some(l0), Some(r0), Some(l1), Some(r1)) = (left0, right0, left1, right1) {
            let a = t / dt_input;
            let l0 = f64::from(l0);
            let l1 = f64::from(l1);
            let r0 = f64::from(r0);
            let r1 = f64::from(r1);
            left = (l0 + (l1 - l0) * a) as i16;
            right = (r0 + (r1 - r0) * a) as i16;
            t += dt_output;
            while t >= dt_input {
                t -= dt_input;
                left0 = left1;
                right0 = right1;
                left1 = input.next();
                if is_stereo {
                    right1 = input.next();
                } else {
                    right1 = left1;
                }
            }
            cur_channel = 1;
            Some(left)
        } else {
            None
        }
    })
}
