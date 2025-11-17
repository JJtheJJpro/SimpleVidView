use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;
use ffmpeg_next as ffmpeg;
use std::fs::File;
use std::io::Read;
use std::sync::Mutex;
use std::thread;
use tauri::{
    command,
    http::{response::Builder as ResponseBuilder},
    ipc::Response,
    AppHandle, DragDropEvent, Manager, State, WindowEvent,
};

// --- State Management ---
struct VideoState {
    player: Mutex<Option<Player>>,
}

struct Player {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: Scaler,
    stream_index: usize,
    width: u32,
    height: u32,
    duration_sec: f64,
}

unsafe impl Send for Player {}

// --- Commands ---

#[command]
fn load_video(app: AppHandle, path: String, state: State<VideoState>) -> Result<String, String> {
    ffmpeg::init().map_err(|e| e.to_string())?;

    let input = input(&path).map_err(|e| e.to_string())?;
    let stream = input
        .streams()
        .best(Type::Video)
        .ok_or("No video stream found")?;
    let stream_index = stream.index();

    let context_decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .map_err(|e| e.to_string())?;
    let decoder = context_decoder
        .decoder()
        .video()
        .map_err(|e| e.to_string())?;

    let width = decoder.width();
    let height = decoder.height();

    // Create a scaler to convert whatever the video format is (e.g., YUV420P) to RGBA for WebGL
    let scaler = Scaler::get(
        decoder.format(),
        width,
        height,
        Pixel::RGBA, // WebGL friendly format
        width,
        height,
        Flags::BILINEAR,
    )
    .map_err(|e| e.to_string())?;

    let duration_sec = input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64;

    let mut player_guard = state.player.lock().map_err(|_| "Failed to lock state")?;
    *player_guard = Some(Player {
        input,
        decoder,
        scaler,
        stream_index,
        width,
        height,
        duration_sec,
    });

    // Return metadata JSON
    Ok(serde_json::json!({
        "width": width,
        "height": height,
        "duration": duration_sec
    })
    .to_string())
}

#[command]
fn get_frame(state: State<VideoState>) -> Result<Response, String> {
    let mut player_guard = state.player.lock().map_err(|_| "Failed to lock state")?;
    let player = player_guard.as_mut().ok_or("No video loaded")?;

    let mut decoded = Video::empty();

    // Decode loop: keep reading packets until we get a valid frame
    while player.decoder.receive_frame(&mut decoded).is_err() {
        let packet = player.input.packets().next();
        if let Some((stream, packet)) = packet {
            if stream.index() == player.stream_index {
                player
                    .decoder
                    .send_packet(&packet)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            return Err("End of stream".into());
        }
    }

    // Scale/Convert frame to RGBA
    let mut rgb_frame = Video::empty();
    player
        .scaler
        .run(&decoded, &mut rgb_frame)
        .map_err(|e| e.to_string())?;

    let data = rgb_frame.data(0);
    let stride = rgb_frame.stride(0);

    // Pack data tightly if stride != width * 4 (optional but safe)
    // For this example, we just return the raw buffer.
    // Note: In production, you handle stride/alignment carefully.

    // Return raw bytes directly (Zero-Copy-ish)
    Ok(Response::new(data.to_vec()))
}

#[command]
fn seek_video(state: State<VideoState>, time_sec: f64) -> Result<(), String> {
    let mut player_guard = state.player.lock().map_err(|_| "Failed to lock state")?;
    let player = player_guard.as_mut().ok_or("No video loaded")?;

    let position = (time_sec * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
    // Seek to the nearest keyframe
    player
        .input
        .seek(position, ..position)
        .map_err(|e| e.to_string())?;

    // Flush decoder to clear internal buffers
    player.decoder.flush();

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("stream", |_ctx, request| {
            // 1. Parse the path (e.g., stream://localhost/C:/Users/Videos/myvideo.mp4)
            let url = request.uri().path();
            let path = urlencoding::decode(url).unwrap_or_default().to_string();
            // Remove leading slash if on Windows might be needed depending on uri implementation
            let clean_path = if cfg!(windows) && path.starts_with('/') {
                &path[1..]
            } else {
                &path
            };

            // 2. Open File
            let mut file = match File::open(clean_path) {
                Ok(f) => f,
                Err(_) => return ResponseBuilder::new().status(404).body(vec![]).unwrap(),
            };

            // 3. Handle Range Requests (Crucial for video seeking!)
            let len = file.metadata().unwrap().len();
            let range_header = request.headers().get("range");

            // ... (Implement standard Range header parsing here) ...
            // For simplicity, this example just sends the whole file,
            // but for real video, you MUST parse "bytes=0-1024" etc.

            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).unwrap();

            ResponseBuilder::new()
                .header("Content-Type", "video/mp4")
                .header("Access-Control-Allow-Origin", "*")
                .body(buffer).unwrap()
        })
        .manage(VideoState {
            player: Mutex::new(None),
        })
        .plugin(tauri_plugin_websocket::init())
        //.setup(|app| {
        //    let app_clone = app.app_handle().clone();
        //    thread::spawn(move || {
        //        let addr = "localhost:9001";
        //        let listener = std::net::TcpListener::bind(addr).unwrap();
        //        while let Ok((stream, _)) = listener.accept() {
        //            let ws_stream = tungstenite::accept(stream).unwrap();
        //            if let Ok(mut v) = app_clone.state::<AppData>().write() {
        //                v.stream = Some(ws_stream);
        //            }
        //        }
        //    });
        //    //app.manage::<AppData>(Arc::new(RwLock::new(AppDataInner {
        //    //    ffhelp: None,
        //    //    stream: None,
        //    //})));
        //    Ok(())
        //})
        //.on_window_event(|win, ev| {
        //    if let WindowEvent::DragDrop(ev) = ev {
        //        match ev {
        //            DragDropEvent::Enter { paths, .. } => if paths.len() != 1 {},
        //            DragDropEvent::Drop { paths, .. } => {
        //                if paths.len() == 1 {
        //                    //set_file(paths[0].to_str().unwrap().to_string());
        //                    win.app_handle().state::<AppData>().write().unwrap().ffhelp =
        //                        Some(FFHelp::open(&paths[0]).unwrap());
        //                    shared_get_frame(0, &win.app_handle().state::<AppData>());
        //                }
        //            }
        //            _ => {}
        //        }
        //    }
        //})
        .invoke_handler(tauri::generate_handler![load_video, get_frame, seek_video])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
