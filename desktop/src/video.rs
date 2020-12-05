//! Video decoding backend(s) for desktop.

use generational_arena::Arena;
use ruffle_codec_h263::parser::{decode_picture, H263Reader};
use ruffle_codec_h263::{DecoderOption, H263State, PictureTypeCode};
use ruffle_core::backend::render::{BitmapInfo, RenderBackend};
use ruffle_core::backend::video::{
    EncodedFrame, Error, FrameDependency, VideoBackend, VideoStreamHandle,
};
use swf::{VideoCodec, VideoDeblocking};

/// A single preloaded video stream.
pub enum VideoStream {
    /// An H.263 video stream.
    H263(H263State),
}

/// Desktop video backend.
///
/// TODO: Currently, this just proxies out to `ruffle_h263`, in the future it
/// should support desktop media playback APIs so we can take advantage of
/// hardware-accelerated video decoding.
pub struct DesktopVideoBackend {
    streams: Arena<VideoStream>,
}

impl DesktopVideoBackend {
    pub fn new() -> Self {
        Self {
            streams: Arena::new(),
        }
    }
}

impl VideoBackend for DesktopVideoBackend {
    fn register_video_stream(
        &mut self,
        num_frames: u32,
        size: (u16, u16),
        codec: VideoCodec,
        filter: VideoDeblocking,
    ) -> Result<VideoStreamHandle, Error> {
        match codec {
            VideoCodec::H263 => Ok(self.streams.insert(VideoStream::H263(H263State::new(
                DecoderOption::SorensonSparkBitstream.into(),
            )))),
            _ => Err(format!("Unsupported video codec type {:?}", codec).into()),
        }
    }

    fn preload_video_stream_frame(
        &mut self,
        stream: VideoStreamHandle,
        encoded_frame: EncodedFrame<'_>,
    ) -> Result<FrameDependency, Error> {
        let stream = self
            .streams
            .get_mut(stream)
            .ok_or("Unregistered video stream")?;

        match stream {
            VideoStream::H263(state) => {
                let mut reader = H263Reader::from_source(encoded_frame.data());
                let picture = decode_picture(
                    &mut reader,
                    DecoderOption::SorensonSparkBitstream.into(),
                    None,
                )?
                .ok_or("Picture in video stream is not a picture")?;

                match picture.picture_type {
                    PictureTypeCode::IFrame => Ok(FrameDependency::Keyframe),
                    PictureTypeCode::PFrame => Ok(FrameDependency::LastFrame),
                    PictureTypeCode::DisposablePFrame => Ok(FrameDependency::LastFrame),
                    _ => Err("Invalid picture type code!".into()),
                }
            }
        }
    }

    fn decode_video_stream_frame(
        &mut self,
        stream: VideoStreamHandle,
        encoded_frame: EncodedFrame<'_>,
        renderer: &mut dyn RenderBackend,
    ) -> Result<BitmapInfo, Error> {
        let stream = self
            .streams
            .get_mut(stream)
            .ok_or("Unregistered video stream")?;

        match stream {
            VideoStream::H263(state) => {
                let mut reader = H263Reader::from_source(encoded_frame.data());

                state.decode_next_picture(&mut reader)?;

                let picture = state
                    .get_last_picture()
                    .expect("Decoding a picture should let us grab that picture");

                //TODO: YUV 4:2:0 decoding
                //TODO: Construct a bitmap drawable for the renderer and hand
                //it back
                unimplemented!("oops");
            }
        }
    }
}
