mod ffhelp;

use crate::ffhelp::FFHelp;
use std::{
    net::TcpStream,
    sync::{Arc, RwLock},
    thread,
};
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, State, WindowEvent};
use tungstenite::{Bytes, WebSocket};

type AppData = Arc<RwLock<AppDataInner>>;

struct AppDataInner {
    ffhelp: Option<FFHelp>,
    stream: Option<WebSocket<TcpStream>>,
}

#[tauri::command]
fn set_file(path: String, appdata: State<AppData>) {
    appdata.write().unwrap().ffhelp = Some(FFHelp::open(&path).unwrap());
}

#[tauri::command]
fn get_frame(idx: usize, appdata: State<AppData>) {
    shared_get_frame(idx, &appdata);
}

fn shared_get_frame(idx: usize, appdata: &AppData) {
    let buf = if let Some(appdata) = &mut appdata.write().unwrap().ffhelp {
        let (w, h, p) = {
            let frame = appdata.get_frame(idx).unwrap();
            let (w, h) = appdata.get_width_height();
            (w, h, frame)
        };

        let mut buf = Vec::new();
        {
            let img = image::RgbImage::from_raw(w, h, p).unwrap();
            img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .unwrap();
        }

        buf
        //app.emit("video-frame", (idx, buf)).unwrap();
    } else {
        vec![]
    };
    if buf.len() > 0 {
        if let Some(appdata) = &mut appdata.write().unwrap().stream {
            appdata
                .write(tungstenite::Message::Binary(Bytes::from(buf)))
                .unwrap();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_websocket::init())
        .setup(|app| {
            let app_clone = app.app_handle().clone();

            thread::spawn(move || {
                let addr = "localhost:9001";

                let listener = std::net::TcpListener::bind(addr).unwrap();

                while let Ok((stream, _)) = listener.accept() {
                    let ws_stream = tungstenite::accept(stream).unwrap();

                    if let Ok(mut v) = app_clone.state::<AppData>().write() {
                        v.stream = Some(ws_stream);
                    }
                }
            });

            app.manage::<AppData>(Arc::new(RwLock::new(AppDataInner {
                ffhelp: None,
                stream: None,
            })));

            Ok(())
        })
        .on_window_event(|win, ev| {
            if let WindowEvent::DragDrop(ev) = ev {
                match ev {
                    DragDropEvent::Enter { paths, .. } => if paths.len() != 1 {},
                    DragDropEvent::Drop { paths, .. } => {
                        if paths.len() == 1 {
                            //set_file(paths[0].to_str().unwrap().to_string());
                            win.app_handle().state::<AppData>().write().unwrap().ffhelp =
                                Some(FFHelp::open(&paths[0]).unwrap());
                            shared_get_frame(0, &win.app_handle().state::<AppData>());
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
