use ffmpeg_next::{self as ffmpeg, decoder, frame::Video, media, software, Rational};
use std::{error::Error, path::Path};

type BasicResult<T> = Result<T, Box<dyn Error>>;

fn rational_to_f64(rat: Rational) -> f64 {
    rat.numerator() as f64 / rat.denominator() as f64
}

pub struct FFHelp {
    ictx: ffmpeg::format::context::Input,
    video_stream_index: usize,
    decoder: decoder::Video,
    scalar: software::scaling::Context,
    w: u32,
    h: u32,
    time_base: ffmpeg::Rational,
    fps: f64,
}

unsafe impl Sync for FFHelp {}
unsafe impl Send for FFHelp {}

impl FFHelp {
    pub fn open<P>(path: &P) -> BasicResult<Self>
    where
        P: AsRef<Path>,
    {
        ffmpeg::init()?;

        let ictx = ffmpeg::format::input(path)?;

        let stream = ictx
            .streams()
            .best(media::Type::Video)
            .expect("no video stream");

        let video_stream_index = stream.index();

        let context_decoder =
            ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        let decoder = context_decoder.decoder().video()?;

        let w = decoder.width();
        let h = decoder.height();

        let scalar = software::scaling::context::Context::get(
            decoder.format(),
            w,
            h,
            ffmpeg::format::Pixel::RGB24,
            w,
            h,
            software::scaling::flag::Flags::BILINEAR,
        )?;

        let tb = stream.time_base();
        let fps = stream.avg_frame_rate();
        let fps = fps.numerator() as f64 / fps.denominator() as f64;

        Ok(Self {
            ictx,
            video_stream_index,
            decoder,
            scalar,
            w: w,
            h: h,
            time_base: tb,
            fps,
        })
    }

    pub fn total_frames(&self) -> usize {
        let duration = self.ictx.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64;
        (duration * self.fps).ceil() as usize
    }

    fn seek_to_frame(&mut self, target: usize) -> BasicResult<()> {
        let ts = (target as f64 / self.fps) / rational_to_f64(self.time_base);
        self.ictx.seek(ts as i64, ..)?;
        self.decoder.flush();
        Ok(())
    }

    pub fn get_frame(&mut self, frame_index: usize) -> BasicResult<Vec<u8>> {
        self.seek_to_frame(frame_index)?;

        let mut decoded = Video::empty();
        let mut rgb = Video::empty();

        for (stream, packet) in self.ictx.packets() {
            if stream.index() == self.video_stream_index {
                self.decoder.send_packet(&packet)?;

                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    let pts = decoded.pts().unwrap_or(0);
                    let current_frame =
                        (pts as f64 * rational_to_f64(self.time_base) * self.fps) as usize;

                    if current_frame >= frame_index {
                        self.scalar.run(&decoded, &mut rgb)?;
                        let vec = rgb.data(0).to_vec();
                        let mut buf = Vec::with_capacity(self.w as usize * self.h as usize * 4);
                        let bpp = rgb.stride(0);
                        for y in 0..self.h {
                            let start = y as usize * bpp;
                            buf.extend_from_slice(&vec[start..start + self.w as usize * bpp]);
                        }
                        return Ok(buf);
                    }
                }
            }
        }

        Err(Box::new(ffmpeg::Error::Other { errno: 0 }))
    }

    pub fn get_frames(&mut self, start: usize, count: usize) -> BasicResult<Vec<Vec<u8>>> {
        let mut out = Vec::new();

        for i in 0..count {
            let f = self.get_frame(start + i)?;
            out.push(f);
        }

        Ok(out)
    }

    pub fn get_width_height(&self) -> (u32, u32) {
        (self.w, self.h)
    }
}
