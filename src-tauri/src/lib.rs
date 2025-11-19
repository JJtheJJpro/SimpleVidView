use ffmpeg_next as ffmpeg;
use http::{header::*, response::Builder as ResponseBuilder, status::StatusCode};
use http_range::HttpRange;
use std::{
    error::Error,
    io::{Read, Seek, SeekFrom, Write},
};
use tauri::{DragDropEvent, WindowEvent};

// Helper enum to hold state
enum Transcoder {
    Video(
        ffmpeg::codec::decoder::Video,
        ffmpeg::codec::encoder::Video,
        usize,            // Output stream index
        ffmpeg::Rational, // Input time base
    ),
    Audio(
        ffmpeg::codec::decoder::Audio,
        ffmpeg::codec::encoder::Audio,
        usize,
        ffmpeg::Rational,
    ),
}

fn convert_to_mp4<PI: AsRef<std::path::Path> + ?Sized, PO: AsRef<std::path::Path> + ?Sized>(
    input_path: &PI,
    output_path: &PO,
) -> Result<(), Box<dyn Error>> {
    // 1. Input Context
    let mut ictx = ffmpeg::format::input(input_path)?;

    // 2. Output Context
    let mut octx = ffmpeg::format::output(output_path)?;

    // Map input stream index to (Output Stream Index, Transcoder Context)
    let mut streamer = std::collections::HashMap::new();

    // 3. Setup Streams & Transcoders
    for (stream_index, (istream, ostream_index)) in ictx
        .streams()
        .filter_map(|s| {
            let medium = s.parameters().medium();
            if medium == ffmpeg::media::Type::Video || medium == ffmpeg::media::Type::Audio {
                Some((
                    s.index(),
                    (
                        s,
                        octx.add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))
                            .unwrap()
                            .index(),
                    ),
                ))
            } else {
                None // Ignore subtitles/data for this simple example
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
    {
        let istream_params = istream.parameters();
        let medium = istream_params.medium();

        if medium == ffmpeg::media::Type::Video {
            // -- VIDEO TRANSCODER (H.264) --

            // Decoder
            let context_decoder = ffmpeg::codec::context::Context::from_parameters(istream_params)?;
            let mut decoder = context_decoder.decoder().video()?;

            // Encoder (H.264)
            let global_header = octx
                .format()
                .flags()
                .contains(ffmpeg::format::flag::Flags::GLOBAL_HEADER);
            let codec =
                ffmpeg::encoder::find(ffmpeg::codec::Id::H264).expect("H.264 codec not found");
            let mut context_encoder = ffmpeg::codec::context::Context::new_with_codec(codec);
            let mut encoder = context_encoder.encoder().video()?;

            // Set Encoder Parameters
            encoder.set_height(decoder.height());
            encoder.set_width(decoder.width());
            encoder.set_aspect_ratio(decoder.aspect_ratio());
            encoder.set_format(ffmpeg::format::Pixel::YUV420P); // Standard for MP4 compatibility
            encoder.set_frame_rate(decoder.frame_rate());
            encoder.set_time_base(istream.time_base()); // Use input timebase

            if global_header {
                encoder.set_flags(ffmpeg::codec::flag::Flags::GLOBAL_HEADER);
            }

            // Optional: Set H.264 specific options (presets)
            let mut opts = ffmpeg::Dictionary::new();
            opts.set("preset", "medium");
            let encoder = encoder.open_with(opts)?;

            // Update output stream parameters to match encoder
            let mut ostream = octx.stream_mut(ostream_index).unwrap();
            ostream.set_parameters(&encoder);

            streamer.insert(
                stream_index,
                Transcoder::Video(decoder, encoder, ostream_index, istream.time_base()),
            );
        } else if medium == ffmpeg::media::Type::Audio {
            // -- AUDIO TRANSCODER (AAC) --

            let context_decoder = ffmpeg::codec::context::Context::from_parameters(istream_params)?;
            let mut decoder = context_decoder.decoder().audio()?;

            let global_header = octx
                .format()
                .flags()
                .contains(ffmpeg::format::flag::Flags::GLOBAL_HEADER);
            let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).expect("AAC codec not found");
            let mut context_encoder = ffmpeg::codec::context::Context::new_with_codec(codec);
            let mut encoder = context_encoder.encoder().audio()?;

            // Set Encoder Parameters
            encoder.set_rate(decoder.rate() as i32);
            // ffmpeg-next handling of channel layouts can be tricky; using default/stereo is safest for a mimic
            encoder.set_channel_layout(ffmpeg::channel_layout::ChannelLayout::STEREO);
            encoder.set_format(ffmpeg::format::Sample::F32(
                ffmpeg::format::sample::Type::Planar,
            )); // AAC usually likes planar floats
            encoder.set_time_base(ffmpeg::Rational::new(1, decoder.rate() as i32));

            if global_header {
                encoder.set_flags(ffmpeg::codec::flag::Flags::GLOBAL_HEADER);
            }

            let encoder = encoder.open()?;

            // Update output stream parameters
            let mut ostream = octx.stream_mut(ostream_index).unwrap();
            ostream.set_parameters(&encoder);

            streamer.insert(
                stream_index,
                Transcoder::Audio(decoder, encoder, ostream_index, istream.time_base()),
            );
        }
    }

    // 4. Write Header
    octx.write_header()?;

    // 5. Transcoding Loop
    for (stream, mut packet) in ictx.packets() {
        if let Some(transcoder) = streamer.get_mut(&stream.index()) {
            match transcoder {
                Transcoder::Video(decoder, encoder, out_index, in_time_base) => {
                    // Decode
                    decoder.send_packet(&packet)?;
                    let mut decoded_frame = ffmpeg::frame::Video::empty();
                    while decoder.receive_frame(&mut decoded_frame).is_ok() {
                        // Rescale timestamps for the frame (Input -> Encoder)
                        let pts = decoded_frame.pts();
                        decoded_frame.set_pts(pts); // Often needs rescaling here if bases differ significantly

                        // Encode
                        encoder.send_frame(&decoded_frame)?;
                        let mut encoded_packet = ffmpeg::Packet::empty();
                        while encoder.receive_packet(&mut encoded_packet).is_ok() {
                            encoded_packet.set_stream(*out_index);
                            // Rescale Packet Timestamp (Encoder -> Output)
                            encoded_packet.rescale_ts(
                                *in_time_base,
                                octx.stream(*out_index).unwrap().time_base(),
                            );
                            encoded_packet.write_interleaved(&mut octx)?;
                        }
                    }
                }
                Transcoder::Audio(decoder, encoder, out_index, in_time_base) => {
                    decoder.send_packet(&packet)?;
                    let mut decoded_frame = ffmpeg::frame::Audio::empty();
                    while decoder.receive_frame(&mut decoded_frame).is_ok() {
                        encoder.send_frame(&decoded_frame)?;
                        let mut encoded_packet = ffmpeg::Packet::empty();
                        while encoder.receive_packet(&mut encoded_packet).is_ok() {
                            encoded_packet.set_stream(*out_index);
                            encoded_packet.rescale_ts(
                                *in_time_base,
                                octx.stream(*out_index).unwrap().time_base(),
                            );
                            encoded_packet.write_interleaved(&mut octx)?;
                        }
                    }
                }
            }
        }
    }

    // 6. Flush Encoders
    for (_, transcoder) in streamer.iter_mut() {
        match transcoder {
            Transcoder::Video(_, encoder, out_index, in_time_base) => {
                encoder.send_eof()?;
                let mut encoded_packet = ffmpeg::Packet::empty();
                while encoder.receive_packet(&mut encoded_packet).is_ok() {
                    encoded_packet.set_stream(*out_index);
                    encoded_packet
                        .rescale_ts(*in_time_base, octx.stream(*out_index).unwrap().time_base());
                    encoded_packet.write_interleaved(&mut octx)?;
                }
            }
            Transcoder::Audio(_, encoder, out_index, in_time_base) => {
                encoder.send_eof()?;
                let mut encoded_packet = ffmpeg::Packet::empty();
                while encoder.receive_packet(&mut encoded_packet).is_ok() {
                    encoded_packet.set_stream(*out_index);
                    encoded_packet
                        .rescale_ts(*in_time_base, octx.stream(*out_index).unwrap().time_base());
                    encoded_packet.write_interleaved(&mut octx)?;
                }
            }
        }
    }

    // 7. Write Trailer
    octx.write_trailer()?;

    Ok(())
}

fn get_stream_response(
    request: http::Request<Vec<u8>>,
) -> Result<http::Response<Vec<u8>>, Box<dyn std::error::Error>> {
    // skip leading `/`
    let path = percent_encoding::percent_decode(&request.uri().path().as_bytes()[1..])
        .decode_utf8_lossy()
        .to_string();

    // return error 404 if it's not our video
    if path != "v.mp4" {
        return Ok(ResponseBuilder::new().status(404).body(Vec::new())?);
    }

    let mut file = std::fs::File::open(&path)?;

    // get file length
    let len = {
        let old_pos = file.stream_position()?;
        let len = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(old_pos))?;
        len
    };

    let mut resp = ResponseBuilder::new().header(CONTENT_TYPE, "video/mp4");

    // if the webview sent a range header, we need to send a 206 in return
    let http_response = if let Some(range_header) = request.headers().get("range") {
        let not_satisfiable = || {
            ResponseBuilder::new()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(CONTENT_RANGE, format!("bytes */{len}"))
                .body(vec![])
        };

        // parse range header
        let ranges = if let Ok(ranges) = HttpRange::parse(range_header.to_str()?, len) {
            ranges
                .iter()
                // map the output back to spec range <start-end>, example: 0-499
                .map(|r| (r.start, r.start + r.length - 1))
                .collect::<Vec<_>>()
        } else {
            return Ok(not_satisfiable()?);
        };

        /// The Maximum bytes we send in one range
        const MAX_LEN: u64 = 1000 * 1024;

        if ranges.len() == 1 {
            let &(start, mut end) = ranges.first().unwrap();

            // check if a range is not satisfiable
            //
            // this should be already taken care of by HttpRange::parse
            // but checking here again for extra assurance
            if start >= len || end >= len || end < start {
                return Ok(not_satisfiable()?);
            }

            // adjust end byte for MAX_LEN
            end = start + (end - start).min(len - start).min(MAX_LEN - 1);

            // calculate number of bytes needed to be read
            let bytes_to_read = end + 1 - start;

            // allocate a buf with a suitable capacity
            let mut buf = Vec::with_capacity(bytes_to_read as usize);
            // seek the file to the starting byte
            file.seek(SeekFrom::Start(start))?;
            // read the needed bytes
            file.take(bytes_to_read).read_to_end(&mut buf)?;

            resp = resp.header(CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
            resp = resp.header(CONTENT_LENGTH, end + 1 - start);
            resp = resp.status(StatusCode::PARTIAL_CONTENT);
            resp.body(buf)
        } else {
            let mut buf = Vec::new();
            let ranges = ranges
                .iter()
                .filter_map(|&(start, mut end)| {
                    // filter out unsatisfiable ranges
                    //
                    // this should be already taken care of by HttpRange::parse
                    // but checking here again for extra assurance
                    if start >= len || end >= len || end < start {
                        None
                    } else {
                        // adjust end byte for MAX_LEN
                        end = start + (end - start).min(len - start).min(MAX_LEN - 1);
                        Some((start, end))
                    }
                })
                .collect::<Vec<_>>();

            let boundary = random_boundary();
            let boundary_sep = format!("\r\n--{boundary}\r\n");
            let boundary_closer = format!("\r\n--{boundary}\r\n");

            resp = resp.header(
                CONTENT_TYPE,
                format!("multipart/byteranges; boundary={boundary}"),
            );

            for (end, start) in ranges {
                // a new range is being written, write the range boundary
                buf.write_all(boundary_sep.as_bytes())?;

                // write the needed headers `Content-Type` and `Content-Range`
                buf.write_all(format!("{CONTENT_TYPE}: video/mp4\r\n").as_bytes())?;
                buf.write_all(
                    format!("{CONTENT_RANGE}: bytes {start}-{end}/{len}\r\n").as_bytes(),
                )?;

                // write the separator to indicate the start of the range body
                buf.write_all("\r\n".as_bytes())?;

                // calculate number of bytes needed to be read
                let bytes_to_read = end + 1 - start;

                let mut local_buf = vec![0_u8; bytes_to_read as usize];
                file.seek(SeekFrom::Start(start))?;
                file.read_exact(&mut local_buf)?;
                buf.extend_from_slice(&local_buf);
            }
            // all ranges have been written, write the closing boundary
            buf.write_all(boundary_closer.as_bytes())?;

            resp.body(buf)
        }
    } else {
        resp = resp.header(CONTENT_LENGTH, len);
        let mut buf = Vec::with_capacity(len as usize);
        file.read_to_end(&mut buf)?;
        resp.body(buf)
    };

    http_response.map_err(Into::into)
}

fn random_boundary() -> String {
    let mut x = [0_u8; 30];
    getrandom::fill(&mut x).expect("failed to get random bytes");
    (x[..])
        .iter()
        .map(|&x| format!("{x:x}"))
        .fold(String::new(), |mut a, x| {
            a.push_str(x.as_str());
            a
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    ffmpeg::init().expect("ffmpeg libraries failed to initialize.");
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("stream", move |ctx, request, responder| {
            match get_stream_response(request) {
                Ok(http_response) => responder.respond(http_response),
                Err(e) => responder.respond(
                    ResponseBuilder::new()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(CONTENT_TYPE, "text/plain")
                        .body(e.to_string().as_bytes().to_vec())
                        .unwrap(),
                ),
            }
        })
        .on_window_event(|win, ev| match ev {
            WindowEvent::DragDrop(ev) => match ev {
                DragDropEvent::Drop { paths, .. } => {
                    if paths.len() == 1 {
                        if std::fs::exists("./v.mp4").unwrap() {
                            std::fs::remove_file("./v.mp4").unwrap();
                        }
                        convert_to_mp4(&paths[0], "./v.mp4").unwrap();
                    }
                }
                _ => {}
            },
            WindowEvent::CloseRequested { .. } => {
                if std::fs::exists("./v.mp4").unwrap() {
                    std::fs::remove_file("./v.mp4").unwrap();
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
