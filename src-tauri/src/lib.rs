mod ffhelp;

use crate::ffhelp::FFHelp;
use std::{
    net::TcpListener,
    sync::{Arc, RwLock},
    thread,
};
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, State, WindowEvent};
use tungstenite::Bytes;

type AppData = Arc<RwLock<AppDataInner>>;

struct AppDataInner {
    ffhelp: Option<FFHelp>,
}

#[tauri::command]
fn set_file(path: String, appdata: State<AppData>) {
    appdata.write().unwrap().ffhelp = Some(FFHelp::open(&path).unwrap());
}

#[tauri::command]
fn get_frame(idx: usize, app: AppHandle, appdata: State<AppData>) {
    if let Some(appdata_z) = &mut appdata.write().unwrap().ffhelp {
        let (w, h, p) = {
            let frame = appdata_z.get_frame(idx).unwrap();
            let (w, h) = appdata_z.get_width_height();
            (w, h, frame)
        };

        let mut buf = Vec::new();
        {
            let img = image::RgbImage::from_raw(w, h, p).unwrap();
            img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .unwrap();
        }

        app.emit("video-frame", (idx, buf)).unwrap();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            const WS_SOCKET: &'static str = "127.0.0.1:9001";

            let app_hd = app.app_handle().clone();
            thread::spawn(move || {
                let server = TcpListener::bind(WS_SOCKET).unwrap();
                for stream in server.incoming() {
                    let mut websocket = tungstenite::accept(stream.unwrap()).unwrap();
                    loop {
                        if let Some(v) = &mut app_hd.state::<AppData>().write().unwrap().ffhelp {
                            let frame_bytes = v.get_frame(0).unwrap();
                            websocket
                                .send(tungstenite::Message::Binary(Bytes::from(frame_bytes)))
                                .unwrap();
                        }
                    }
                }
            });

            app.manage::<AppData>(Arc::new(RwLock::new(AppDataInner { ffhelp: None })));

            Ok(())
        })
        .on_window_event(|win, ev| {
            if let WindowEvent::DragDrop(ev) = ev {
                match ev {
                    DragDropEvent::Enter { paths, .. } => if paths.len() != 1 {},
                    DragDropEvent::Drop { paths, .. } => {
                        if paths.len() == 1 {
                            win.emit("new-video", paths[0].to_str().unwrap().to_string())
                                .unwrap();
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![get_frame, set_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
