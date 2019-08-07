use fnv::FnvHashMap;
use generational_arena::Arena;
use ruffle_core::backend::audio::decoders::{stream_tag_reader, AdpcmDecoder, Mp3Decoder};
use ruffle_core::backend::audio::{AudioBackend, AudioStreamHandle, SoundHandle};
use ruffle_core::backend::audio::swf::{self, AudioCompression};
use std::cell::RefCell;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::AudioContext;

pub struct WebAudioBackend {
    context: AudioContext,
    sounds: Arena<Sound>,
    stream_data: FnvHashMap<swf::CharacterId, StreamData>,
    id_to_sound: FnvHashMap<swf::CharacterId, SoundHandle>,
    left_samples: Vec<f32>,
    right_samples: Vec<f32>,
}

thread_local! {
    static STREAMS: RefCell<Arena<AudioStream>> = RefCell::new(Arena::new());
}

struct StreamData {
    format: swf::SoundFormat,
    audio_data: Vec<u8>,
}

enum SoundSource {
    Decoder(Vec<u8>),
    AudioBuffer(web_sys::AudioBuffer),
}

struct Sound {
    format: swf::SoundFormat,
    source: SoundSource,
}

type Decoder = Box<dyn Iterator<Item=i16>>;
enum AudioStream {
    Decoder { decoder: Decoder, is_stereo: bool, },// closure: Option<Closure<Box<FnMut(web_sys::AudioProcessingEvent)>>> } ,
    AudioBuffer { node: web_sys::AudioBufferSourceNode },
}

type Error = Box<std::error::Error>;

impl WebAudioBackend {
    pub fn new() -> Result<Self, Error> {
        let context = AudioContext::new().map_err(|_| "Unable to create AudioContext")?;
        log::info!("sample rate: {}", context.sample_rate());
        Ok(Self {
            context,
            sounds: Arena::new(),
            stream_data: FnvHashMap::default(),
            id_to_sound: FnvHashMap::default(),
            left_samples: vec![],
            right_samples: vec![],
        })
    }

    fn play_sound_internal(&mut self, handle: SoundHandle) -> SoundHandle {
        let sound = self.sounds.get(handle).unwrap();
        match &sound.source {
            SoundSource::AudioBuffer(audio_buffer) => {
                let node = self.context.create_buffer_source().unwrap();
                node.set_buffer(Some(audio_buffer));
                node
                    .connect_with_audio_node(&self.context.destination())
                    .unwrap();

                let audio_stream = AudioStream::AudioBuffer {
                    node
                };
                STREAMS.with(|streams| {
                    let mut streams = streams.borrow_mut();
                    streams.insert(audio_stream)
                })
            }
            SoundSource::Decoder(audio_data) => {
                let decoder: Decoder = match sound.format.compression {
                    AudioCompression::Adpcm => Box::new(AdpcmDecoder::new(
                        std::io::Cursor::new(audio_data.to_vec()),
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
                        std::io::Cursor::new(audio_data.to_vec())//&sound.data[..]
                    )),
                    _ => unimplemented!()
                };

                let decoder: Decoder = if sound.format.sample_rate != self.context.sample_rate() as u16 {
                    Box::new(resample(decoder, sound.format.sample_rate, self.context.sample_rate() as u16, sound.format.is_stereo))
                } else {
                    decoder
                };
                
                let audio_stream = AudioStream::Decoder {
                    decoder,
                    is_stereo: sound.format.is_stereo,
                    //closure: None,
                };
                STREAMS.with(|streams| {
                    let mut streams = streams.borrow_mut();
                    let stream_handle = streams.insert(audio_stream);
                    let script_processor_node = self.context.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(4096, 0, if sound.format.is_stereo { 2 } else { 1 }).unwrap();
                    let script_node = script_processor_node.clone();
                    
                    let closure = Closure::wrap(Box::new(move |event| {
                            STREAMS.with(|streams| {
                                let mut streams = streams.borrow_mut();
                                let audio_stream = streams.get_mut(stream_handle).unwrap();
                                let complete = WebAudioBackend::update_script_processor(audio_stream, event);
                                if complete {
                                    streams.remove(stream_handle);
                                    script_node.disconnect().unwrap();
                                }
                            })
                        }) as Box<FnMut(web_sys::AudioProcessingEvent)>);
                        script_processor_node.set_onaudioprocess(Some(closure.as_ref().unchecked_ref()));
                        closure.forget();
                    // if let AudioStream::Decoder { ref mut closure, .. } = audio_stream {
                    //     // *closure = Some(Closure::wrap(Box::new(move |event| {
                    //     //     STREAMS.with(|streams| {
                    //     //         let mut streams = streams.borrow_mut();
                    //     //         let complete = WebAudioBackend::update_script_processor(audio_stream, event);
                    //     //         if complete {
                    //     //             streams.remove(stream_handle);
                    //     //             script_node.disconnect().unwrap();
                    //     //         }
                    //     //     })
                    //     // }) as Box<FnMut(web_sys::AudioProcessingEvent)>));
                    //     // script_processor_node.set_onaudioprocess(Some(closure.as_ref().unchecked_ref()));
                    // }

                    stream_handle
                })
            }
        }
    }

    fn decompress_to_audio_buffer(&mut self, format: &swf::SoundFormat, audio_data: &[u8]) -> web_sys::AudioBuffer {
        let audio_buffer = self.context.create_buffer(if format.is_stereo { 2 } else { 1 }, 999, format.sample_rate.into()).unwrap();

        match format.compression {
            AudioCompression::Uncompressed => {
                self.left_samples = audio_data.iter().step_by(2).cloned().map(|n| f32::from(n) / 32767.0).collect();
                if format.is_stereo {
                    self.right_samples = audio_data.iter().skip(1).step_by(2).cloned().map(|n| f32::from(n) / 32767.0).collect();
                }
            }
            AudioCompression::Adpcm => {
                let mut decoder = AdpcmDecoder::new(audio_data,
                    format.is_stereo,
                    format.sample_rate
                ).unwrap();
                if format.is_stereo {
                    while let (Some(l), Some(r)) = (decoder.next(), decoder.next()) {
                        self.left_samples.push(f32::from(l) / 32767.0);
                        self.right_samples.push(f32::from(r) / 32767.0);
                    } 
                } else {
                    self.left_samples = decoder.map(|n| f32::from(n) / 32767.0).collect();
                }
            }
            AudioCompression::Mp3 => {
                // let data_array = unsafe { Uint8Array::view(audio_data) };
                // let array_buffer = data_array.buffer().slice_with_end(
                //     data_array.byte_offset(),
                //     data_array.byte_offset() + data_array.byte_length(),
                // );
                // let closure = Closure::wrap(Box::new(move |buffer: wasm_bindgen::JsValue| {
                    
                // })
                //     as Box<dyn FnMut(wasm_bindgen::JsValue)>);
                // self.context.decode_audio_data_with_success_callback(array_buffer, &closure);
                let mut decoder = Mp3Decoder::new(if format.is_stereo { 2 } else { 1 }, format.sample_rate.into(), audio_data);
                if format.is_stereo {
                    while let (Some(l), Some(r)) = (decoder.next(), decoder.next()) {
                        self.left_samples.push(f32::from(l) / 32767.0);
                        self.right_samples.push(f32::from(r) / 32767.0);
                    } 
                } else {
                    self.left_samples = decoder.map(|n| f32::from(n) / 32767.0).collect();
                }
            }
            _ => unimplemented!(),
        }

        audio_buffer.copy_to_channel(&mut self.left_samples, 0).unwrap();
        if format.is_stereo {
            audio_buffer.copy_to_channel(&mut self.right_samples, 1).unwrap();
        }

        audio_buffer
    }

    fn update_script_processor(
        audio_stream: &mut AudioStream,
        event: web_sys::AudioProcessingEvent,
    ) -> bool {
        let mut complete = false;
        let mut left_samples = vec![];
        let mut right_samples = vec![];
        if let AudioStream::Decoder { decoder, is_stereo, .. } = audio_stream {
            let output_buffer = event.output_buffer().unwrap();
            let num_frames = output_buffer.length() as usize;

            for _ in 0..num_frames {
                if let (Some(l), Some(r)) = (decoder.next(), decoder.next()) {
                    left_samples.push(f32::from(l) / 32767.0);
                    if *is_stereo {
                        right_samples.push(f32::from(r) / 32767.0);
                    }
                } else {
                    complete = true;
                    break;
                }
            }
            output_buffer.copy_to_channel(&mut left_samples[..], 0).unwrap();
            if *is_stereo {
                output_buffer.copy_to_channel(&mut right_samples[..], 1).unwrap();
            }
        }
        
        complete
    }
}

impl AudioBackend for WebAudioBackend {
    fn register_sound(&mut self, sound: &swf::Sound) -> Result<SoundHandle, Error> {
        let sound = Sound {
            format: sound.format.clone(),
            source: SoundSource::AudioBuffer(self.decompress_to_audio_buffer(&sound.format, &sound.data[..])),
        };
        Ok(self.sounds.insert(sound))
    }

    fn preload_sound_stream_head(&mut self, clip_id: swf::CharacterId, stream_info: &swf::SoundStreamHead) {
        self.stream_data.entry(clip_id).or_insert_with(|| {
            StreamData {
                format: stream_info.stream_format.clone(),
                audio_data: vec![],
            }
        });
    }

    fn preload_sound_stream_block(&mut self, clip_id: swf::CharacterId, audio_data: &[u8]) {
        if let Some(stream) = self.stream_data.get_mut(&clip_id) {
            if let AudioCompression::Mp3 = stream.format.compression {
                stream.audio_data.extend_from_slice(&audio_data[4..]);
            } else {
                stream.audio_data.extend_from_slice(&audio_data[2..]);
            }
        }
    }

    fn preload_sound_stream_end(&mut self, clip_id: swf::CharacterId) {
        if let Some(stream) = self.stream_data.remove(&clip_id) {
            let audio_buffer = self.decompress_to_audio_buffer(&stream.format, &stream.audio_data[..]);
            let handle = self.sounds.insert(Sound {
                format: stream.format,
                source: SoundSource::AudioBuffer(audio_buffer),
            });
            self.id_to_sound.insert(clip_id, handle);
        }
    }

    fn play_sound(&mut self, sound: SoundHandle) {
        self.play_sound_internal(sound);
    }

    fn start_stream(
        &mut self,
        clip_id: swf::CharacterId,
        _clip_data: ruffle_core::tag_utils::SwfSlice,
        _stream_info: &swf::SoundStreamHead,
    ) -> AudioStreamHandle {
        let handle = *self.id_to_sound.get(&clip_id).unwrap();
        self.play_sound_internal(handle)
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
