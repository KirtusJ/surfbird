use crate::{
    browser::{Browser, Command},
    human,
    session::{self, Call},
};
use std::{
    env,
    io::{self, BufRead, IsTerminal, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// Even at six JSON bytes per text scalar, 512 × 2048 line scalars and
// 256 × 160 label scalars use under 6.3 MiB. Canonical HTTP(S) URLs have
// no raw control bytes and need at most two JSON bytes per byte. At 8192
// bytes each, 257 URLs add at most 4.02 MiB. The rest covers the title
// (including a host used as a plain-text title), keys, counters and framing.
const MAX_REPLY_BYTES: usize = 16 * 1024 * 1024;
// One navigation can issue seven requests of up to twelve seconds each.
const REPLY_TIMEOUT: Duration = Duration::from_secs(120);

fn read_reply(reader: &mut io::BufReader<TcpStream>, timeout: Duration) -> io::Result<String> {
    let deadline = Instant::now() + timeout;
    let mut reply = Vec::new();
    let timed_out = || io::Error::new(io::ErrorKind::TimedOut, "Session reply timed out");
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(timed_out)?;
        reader.get_ref().set_read_timeout(Some(remaining))?;
        let chunk = match reader.fill_buf() {
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                return Err(timed_out())
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if Instant::now() >= deadline {
            return Err(timed_out());
        }
        if chunk.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Session disconnected before a complete reply",
            ));
        }
        let newline = chunk.iter().position(|&byte| byte == b'\n');
        let take = newline.map_or(chunk.len(), |i| i + 1);
        let capacity = MAX_REPLY_BYTES - reply.len();
        if take > capacity || (take == capacity && newline.is_none()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Session reply exceeds 16 MiB",
            ));
        }
        reply.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if newline.is_some() {
            return String::from_utf8(reply)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        }
    }
}

pub fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if matches!(args.first().map(String::as_str), Some("--help" | "-h")) {
        println!("surfbird [--listen 127.0.0.1:PORT] [URL]\nsurfbird --machine 127.0.0.1:PORT\nThe machine client reads one JSON command per line and prints its response.");
        return Ok(());
    }
    if args.first().is_some_and(|v| v == "--machine") {
        if args.len() != 2 {
            return Err("Pass the live session's loopback address".into());
        }
        let address = local(&args[1])?;
        let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let mut reader = io::BufReader::new(socket.try_clone().map_err(|e| e.to_string())?);
        for l in io::stdin().lock().lines() {
            let line = l.map_err(|e| e.to_string())?;
            writeln!(socket, "{line}").map_err(|e| e.to_string())?;
            let reply = read_reply(&mut reader, REPLY_TIMEOUT).map_err(|e| e.to_string())?;
            print!("{reply}");
            io::stdout().flush().map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let (mut listen, mut url) = (None, None);
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--listen" && listen.is_none() {
            i += 1;
            let addr = args.get(i).ok_or("Missing --listen address")?;
            listen = Some(TcpListener::bind(local(addr)?).map_err(|e| e.to_string())?);
        } else if url.is_none() && !args[i].starts_with('-') {
            url = Some(args[i].clone());
        } else {
            return Err("Use --help for usage".into());
        }
        i += 1;
    }
    let (bus, rx) = mpsc::sync_channel::<Call>(16);
    thread::spawn(move || {
        let mut browser = Browser::new();
        for call in rx {
            call(&mut browser);
        }
    });
    if let Some(listener) = listen {
        eprintln!(
            "Machine session {}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let bus = bus.clone();
        thread::spawn(move || session::serve(listener, bus));
    }
    if let Some(url) = url {
        session::dispatch(&bus, Command::Open { url })?;
    }
    human::run(
        io::stdin().lock(),
        io::stdout().lock(),
        &bus,
        io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none(),
    )
    .map_err(|e| e.to_string())
}
fn local(s: &str) -> Result<SocketAddr, String> {
    let addr: SocketAddr = s
        .parse()
        .map_err(|_| "Use a numeric loopback IP and port")?;
    if !addr.ip().is_loopback() {
        return Err("The machine interface is available on loopback only".into());
    }
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::{Link, Page};

    fn peer() -> (io::BufReader<TcpStream>, TcpStream) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let sender = listener.accept().unwrap().0;
        (io::BufReader::new(socket), sender)
    }

    #[test]
    fn reply_deadline_survives_trickled_bytes() {
        let (mut reader, mut sender) = peer();
        let sending = thread::spawn(move || {
            for _ in 0..50 {
                if sender.write_all(b"x").is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            let _ = sender.write_all(b"\n");
        });
        assert_eq!(
            read_reply(&mut reader, Duration::from_millis(150))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        drop(reader);
        sending.join().unwrap();
    }

    #[test]
    fn maximum_reply_includes_newline_and_preserves_the_next_frame() {
        let (mut reader, mut sender) = peer();
        let sending = thread::spawn(move || {
            let mut frame = vec![b'x'; MAX_REPLY_BYTES - 1];
            frame.extend_from_slice(b"\n{}\n");
            sender.write_all(&frame).unwrap();
        });
        assert_eq!(
            read_reply(&mut reader, REPLY_TIMEOUT).unwrap().len(),
            MAX_REPLY_BYTES
        );
        assert_eq!(read_reply(&mut reader, REPLY_TIMEOUT).unwrap(), "{}\n");
        sending.join().unwrap();
    }

    #[test]
    fn serialized_page_fits_with_maximum_fields_and_json_escapes() {
        let url = format!("https://x/{}", "\\".repeat(8192 - 10));
        let page = Page {
            url: url.clone(),
            title: "\0".repeat(8192),
            lines: vec!["\0".repeat(2048); 512],
            links: vec![
                Link {
                    url,
                    label: "\0".repeat(160)
                };
                256
            ],
            truncated: false,
            status: u16::MAX,
        };
        let reply = serde_json::json!({"ok":true,"state":{"revision":u64::MAX,"back":false,"forward":false,"page":page}});
        assert!(serde_json::to_vec(&reply).unwrap().len() < MAX_REPLY_BYTES);
    }
}
