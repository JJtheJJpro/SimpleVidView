use std::{error::Error, net::TcpListener};
use tungstenite::accept;

fn run_ws_server() -> Result<(), Box<dyn Error>> {
    let server = TcpListener::bind("127.0.0.1:9001")?;
    for stream in server.incoming() {
        let mut websocket = accept(stream.unwrap())?;
        // Wait for client handshake; then stream frames in a loop
        loop {
            // frame_bytes: Vec<u8> from your FFHelp (RGBA packed)
            let frame_bytes: Vec<u8> = get_next_rgba_frame(); // your code
            websocket.send(tungstenite::Message::Binary(frame_bytes))?;
            // sleep or sync to framerate
        }
    }

    Ok(())
}