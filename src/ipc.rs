use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

pub const IPC_PORT: u16 = 49281;

#[derive(Debug, Clone)]
pub enum IpcCommand {
    Wake,
    Show,
    ShowTab(String),
    TestAlert,
    Reset,
    Exit,
}

pub fn check_or_wake_instance() -> bool {
    let addr = format!("127.0.0.1:{}", IPC_PORT);
    if let Ok(mut stream) =
        TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(800))
    {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(800)));
        if stream.write_all(b"WAKE_AND_PING").is_ok() {
            let mut buf = [0u8; 16];
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 && &buf[..4] == b"PONG" {
                    return true;
                }
            }
        }
    }
    false
}

pub fn start_ipc_server(sender: Sender<IpcCommand>) {
    thread::spawn(move || {
        let addr = format!("127.0.0.1:{}", IPC_PORT);
        if let Ok(listener) = TcpListener::bind(&addr) {
            for mut sock in listener.incoming().flatten() {
                let _ = sock.set_read_timeout(Some(Duration::from_millis(1500)));
                let mut buf = [0u8; 64];
                if let Ok(n) = sock.read(&mut buf) {
                    let msg = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                    if msg == "WAKE_AND_PING" {
                        let _ = sock.write_all(b"PONG");
                        let _ = sender.send(IpcCommand::Wake);
                    } else if msg == "SHOW:datasaver" {
                        let _ = sock.write_all(b"OK");
                        let _ = sender.send(IpcCommand::ShowTab("datasaver".to_string()));
                    } else if msg == "TEST_ALERT" {
                        let _ = sock.write_all(b"OK");
                        let _ = sender.send(IpcCommand::TestAlert);
                    } else if msg == "RESET" {
                        let _ = sock.write_all(b"OK");
                        let _ = sender.send(IpcCommand::Reset);
                    } else if msg == "EXIT" {
                        let _ = sock.write_all(b"OK");
                        let _ = sender.send(IpcCommand::Exit);
                    } else if msg.starts_with("SHOW") {
                        let _ = sock.write_all(b"OK");
                        let _ = sender.send(IpcCommand::Show);
                    }
                }
            }
        }
    });
}
