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
    time::Duration,
};
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
            let mut reply = String::new();
            if reader.read_line(&mut reply).map_err(|e| e.to_string())? == 0 {
                return Err("Session disconnected".into());
            }
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
