use crate::browser::{Browser, Command};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::{self, BufRead, Read, Write},
    net::TcpListener,
    sync::{
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
pub type Call = Box<dyn FnOnce(&mut Browser) + Send>;
pub type Bus = SyncSender<Call>;
pub fn snapshot(browser: &Browser) -> Value {
    let (back, forward) = browser.navigation();
    json!({"revision":browser.revision,"back":back,"forward":forward,"page":browser.page()})
}
pub fn dispatch(bus: &Bus, cmd: Command) -> Result<Value, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    bus.try_send(Box::new(move |b| {
        let result = b.act(cmd).map(|_| snapshot(b));
        let _ = tx.send(result);
    }))
    .map_err(|_| "Session is busy or closed".to_owned())?;
    rx.recv().map_err(|_| "Session closed".to_owned())?
}
pub fn serve(listener: TcpListener, bus: Bus) {
    let count = Arc::new(Mutex::new(HashSet::new()));
    for stream in listener.incoming().flatten() {
        let peer = match stream.peer_addr() {
            Ok(v) if v.ip().is_loopback() => v,
            _ => continue,
        };
        {
            let mut clients = count.lock().unwrap();
            if clients.len() >= 8 {
                continue;
            }
            clients.insert(peer);
        }
        let bus = bus.clone();
        let count = count.clone();
        thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
            if let Ok(reader) = stream.try_clone() {
                let _ = protocol(io::BufReader::new(reader), stream, &bus);
            }
            count.lock().unwrap().remove(&peer);
        });
    }
}
pub fn protocol(mut reader: impl BufRead, mut writer: impl Write, bus: &Bus) -> io::Result<()> {
    loop {
        let mut line = Vec::new();
        if reader.by_ref().take(8194).read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let long = line.len() > 8192;
        let command = if long {
            Err("Command exceeds 8192 bytes".into())
        } else {
            serde_json::from_slice::<Command>(&line).map_err(|e| e.to_string())
        };
        // An HTTP request from a website must never turn its body into a command.
        let invalid_frame = command.is_err();
        let result = command.and_then(|cmd| dispatch(bus, cmd));
        let reply = match result {
            Ok(v) => json!({"ok":true,"state":v}),
            Err(e) => json!({"ok":false,"error":e}),
        };
        serde_json::to_writer(&mut writer, &reply)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        if invalid_frame {
            break;
        }
    }
    Ok(())
}
