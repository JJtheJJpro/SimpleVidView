#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ffmpeg_next::{
    format::input,
    frame::Video,
    software::scaling::{context::Context as Scaler, flag::Flags},
    util::format::pixel::Pixel,
};
use rfd::FileDialog;
use sdl2::{
    event::Event, keyboard::Keycode, pixels::PixelFormatEnum, rect::Point, render::TextureAccess,
};
use std::{env, path::PathBuf, time::Duration};

const SLIDER_HEIGHT: u32 = 30;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize ffmpeg
    ffmpeg_next::init()?;

    let args = env::args().skip(1).collect::<Vec<_>>();
    let path = if let Some(p) = args.first() {
        PathBuf::from(p)
    } else {
        // Open file dialog
        FileDialog::new()
            .add_filter("Video", &["mp4", "mkv", "avi"])
            .pick_file()
            .expect("No file selected")
    };

    // Open input video
    let mut ictx = input(&path)?;
    let input_stream = ictx
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .ok_or("No video stream")?;
    let video_stream_index = input_stream.index();
    let context_decoder =
        ffmpeg_next::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;

    let spf = {
        let fps = input_stream.avg_frame_rate();
        (fps.denominator() as f64) / (fps.numerator() as f64)
    };
    let total_duration_secs = ictx.duration() as f64 / ffmpeg_next::ffi::AV_TIME_BASE as f64;

    // SDL2 setup
    let sdl_ctx = sdl2::init()?;
    let video_subsystem = sdl_ctx.video()?;
    let window = video_subsystem
        .window("Simple Vid View", decoder.width(), decoder.height())
        .position_centered()
        .resizable() // Enable maximize button
        //.maximized()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build()?;
    let texture_creator = canvas.texture_creator();

    let mut event_pump = sdl_ctx.event_pump()?;

    // Scaling context to RGB24 for SDL
    let mut scaler = Scaler::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )?;

    let mut paused = false;
    let mut frames: Vec<Video> = Vec::new();
    let mut current_frame = 0usize;

    // Decode all frames
    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;
            let mut frame = Video::empty();
            while decoder.receive_frame(&mut frame).is_ok() {
                frames.push(frame.clone());
            }
        }
    }
    // Flush decoder
    decoder.send_eof()?;
    let mut frame = Video::empty();
    while decoder.receive_frame(&mut frame).is_ok() {
        frames.push(frame.clone());
    }

    let nframes = frames.len();

    let mut seeking = false;

    // Main loop
    'running: loop {
        // Event handling
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Space),
                    ..
                } => paused = !paused,
                Event::KeyDown {
                    keycode: Some(Keycode::Left),
                    ..
                } => {
                    if current_frame > 0 {
                        current_frame -= 1;
                    } else {
                        current_frame = frames.len() - 1;
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => {
                    if current_frame + 1 < frames.len() {
                        current_frame += 1;
                    } else {
                        current_frame = 0;
                    }
                }
                Event::MouseButtonDown { x, y, .. } => {
                    let (win_w, win_h) = canvas.output_size()?;
                    if y as u32 >= win_h - SLIDER_HEIGHT {
                        if x as u32 > SLIDER_HEIGHT {
                            seeking = true;
                            let ratio = x as f64 / win_w as f64;
                            let target_ts = ratio * total_duration_secs;
                            seek_in_frames(
                                &mut current_frame,
                                target_ts,
                                total_duration_secs,
                                nframes,
                            );
                        } else {
                            paused = !paused;
                        }
                    }
                }
                Event::MouseButtonUp { .. } => {
                    seeking = false;
                }
                Event::MouseMotion { x, .. } => {
                    let (win_w, _) = canvas.output_size()?;
                    if seeking {
                        //&& (y as u32 >= win_h - SLIDER_HEIGHT) {
                        let ratio = if x as u32 >= SLIDER_HEIGHT {
                            (x as f64 - SLIDER_HEIGHT as f64) / win_w as f64
                        } else {
                            0.0
                        } + 0.01;
                        let target_ts = ratio * total_duration_secs;
                        seek_in_frames(&mut current_frame, target_ts, total_duration_secs, nframes);
                    }
                }
                _ => {}
            }
        }

        if !paused {
            current_frame = (current_frame + 1) % frames.len();
        }

        // Get current frame and convert to RGB
        let mut rgb_frame = Video::empty();
        scaler.run(&frames[current_frame], &mut rgb_frame)?;

        let pitch = rgb_frame.stride(0);
        let data = rgb_frame.data(0);

        let mut texture = texture_creator.create_texture(
            PixelFormatEnum::RGB24,
            TextureAccess::Streaming,
            decoder.width(),
            decoder.height(),
        )?;
        texture.update(None, data, pitch)?;

        // Compute letterbox/pillarbox rect
        let (win_w, win_h) = canvas.output_size()?;
        let vid_w = decoder.width();
        let vid_h = decoder.height();

        let scale_w = win_w as f32 / vid_w as f32;
        let scale_h = (win_h - SLIDER_HEIGHT) as f32 / vid_h as f32;
        let scale = scale_w.min(scale_h);

        let dest_w = (vid_w as f32 * scale) as u32;
        let dest_h = (vid_h as f32 * scale) as u32;
        let dest_x = ((win_w - dest_w) / 2) as i32;
        let dest_y = (((win_h - SLIDER_HEIGHT) - dest_h) / 2) as i32;
        let dest_rect = sdl2::rect::Rect::new(dest_x, dest_y, dest_w, dest_h);

        let play_pause_rect = sdl2::rect::Rect::new(
            0,
            (win_h - SLIDER_HEIGHT) as i32,
            SLIDER_HEIGHT,
            SLIDER_HEIGHT,
        );

        let slider_rect = sdl2::rect::Rect::new(
            SLIDER_HEIGHT as i32,
            (win_h - SLIDER_HEIGHT) as i32,
            win_w,
            SLIDER_HEIGHT,
        );

        // Render
        canvas.clear();

        canvas.copy(&texture, None, Some(dest_rect))?;
        canvas.set_draw_color(sdl2::pixels::Color::RGB(40, 40, 40));
        canvas.fill_rect(slider_rect)?;

        let progress = (current_frame as f64) / (nframes as f64);
        let progress_px = (progress * win_w as f64) as u32;

        let filled_rect = sdl2::rect::Rect::new(
            SLIDER_HEIGHT as i32,
            (win_h - SLIDER_HEIGHT + 4) as i32,
            progress_px,
            SLIDER_HEIGHT - 8,
        );
        canvas.set_draw_color(if !seeking && !paused {
            sdl2::pixels::Color::RGB(0x6F, 0, 0x6F)
        } else if paused && seeking {
            sdl2::pixels::Color::RGB(0, 0xAF, 0)
        } else if paused && !seeking {
            sdl2::pixels::Color::RGB(0xCF, 0, 0)
        } else {
            sdl2::pixels::Color::RGB(0xCF, 0, 0)
        });
        canvas.fill_rect(filled_rect)?;

        canvas.set_draw_color(sdl2::pixels::Color::RGB(20, 20, 20));
        canvas.fill_rect(play_pause_rect)?;

        canvas.set_draw_color(sdl2::pixels::Color::RGB(0xE2, 0xE2, 0xE2));
        if paused {
            let pause_rect1 =
                sdl2::rect::Rect::new(7, (win_h - SLIDER_HEIGHT + 4) as i32, 5, SLIDER_HEIGHT - 8);
            let pause_rect2 = sdl2::rect::Rect::new(
                SLIDER_HEIGHT as i32 - 12,
                (win_h - SLIDER_HEIGHT + 4) as i32,
                5,
                SLIDER_HEIGHT - 8,
            );
            canvas.fill_rect(pause_rect1)?;
            canvas.fill_rect(pause_rect2)?;
        } else {
            canvas.draw_lines(
                vec![
                    Point::new(7, (win_h - SLIDER_HEIGHT + 5) as i32),
                    Point::new(
                        (SLIDER_HEIGHT - 7) as i32,
                        (win_h - (SLIDER_HEIGHT / 2)) as i32,
                    ),
                    Point::new(7, (win_h - 5) as i32),
                    Point::new(7, (win_h - SLIDER_HEIGHT + 5) as i32),
                ]
                .as_slice(),
            )?;
        }

        canvas.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
        canvas.present();

        std::thread::sleep(Duration::from_secs_f64(spf));
    }

    Ok(())
}

fn seek_in_frames(
    current_frame: &mut usize,
    seconds: f64,
    total_duration_secs: f64,
    nframes: usize,
) {
    if nframes == 0 {
        return;
    }
    let ratio = (seconds / total_duration_secs).clamp(0.0, 1.0);
    let idx = (ratio * (nframes as f64 - 0.5)).round() as usize;
    *current_frame = idx.min(nframes - 1);
}
