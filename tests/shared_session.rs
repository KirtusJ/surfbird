use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn fixture() -> String {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    thread::spawn(move || {
        for mut stream in server.incoming().flatten() {
            let mut first = String::new();
            let mut reader = BufReader::new(&stream);
            reader.read_line(&mut first).unwrap();
            let mut header = String::new();
            loop {
                header.clear();
                if reader.read_line(&mut header).unwrap() == 0 || header == "\r\n" {
                    break;
                }
            }
            let (status, headers, body) = match first.split_whitespace().nth(1) {
                Some("/start") => ("302 Found", "Location: /wharf/start\r\n", ""),
                Some("/wharf/start") => ("200 OK", "Content-Type: text/html; charset=utf-8\r\n",
                    "<title>Harbour</title><base href='../maps/'><p>Tide <b>tables</b>.</p><a href='jetty'>North jetty</a>"),
                Some("/maps/jetty") => ("200 OK", "Content-Type: text/html\r\n",
                    "<title>North jetty</title><p>High water at noon.</p><a href='/start'>Harbour</a>"),
                _ => ("200 OK", "Content-Type: image/png\r\n", "abc"),
            };
            write!(stream, "HTTP/1.1 {status}\r\n{headers}Connection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    url
}

fn prompt(out: &mut impl Read) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"surf> ") {
        let mut c = [0];
        out.read_exact(&mut c)
            .expect("terminal closed before prompt");
        bytes.push(c[0]);
        assert!(bytes.len() < 16384);
    }
    String::from_utf8(bytes).unwrap()
}

fn machine(client: &mut BufReader<TcpStream>, req: Value) -> Value {
    writeln!(client.get_mut(), "{req}").unwrap();
    let mut line = String::new();
    client.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

fn launch() -> (Running, String) {
    let mut child = Running(
        Command::new(env!("CARGO_BIN_EXE_surfbird"))
            .args(["--listen", "127.0.0.1:0"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    BufReader::new(child.0.stderr.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    (
        child,
        address
            .trim()
            .strip_prefix("Machine session ")
            .unwrap()
            .to_owned(),
    )
}

#[test]
fn terminal_and_machine_share_navigation_content_and_back_history() {
    let base = fixture();
    let (mut child, address) = launch();
    let socket = TcpStream::connect(&address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut client = BufReader::new(socket);
    let mut input = child.0.stdin.take().unwrap();
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    assert!(prompt(&mut output).contains("surfbird:home"));
    // A cross-origin HTTP POST reaching the raw port cannot issue a browser command.
    let mut http = BufReader::new(TcpStream::connect(&address).unwrap());
    http.get_mut()
        .write_all(b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n{\"op\":\"follow\",\"link\":1}\n")
        .unwrap();
    let mut rejected = String::new();
    http.read_line(&mut rejected).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&rejected).unwrap()["ok"],
        false
    );
    assert_eq!(
        machine(&mut client, json!({"op":"read"}))["state"]["revision"],
        0
    );

    writeln!(input, "open {base}/start").unwrap();
    assert!(prompt(&mut output).contains("Tide tables."));
    let start = machine(&mut client, json!({"op":"read"}))["state"].clone();
    assert_eq!(start["page"]["url"], format!("{base}/wharf/start"));
    assert_eq!(
        start["page"]["links"][0]["url"],
        format!("{base}/maps/jetty")
    );
    writeln!(input, "1").unwrap();
    let human_at_jetty = prompt(&mut output);
    let first = machine(&mut client, json!({"op":"read"}))["state"].clone();
    assert_eq!(first["page"]["title"], "North jetty");
    assert!(human_at_jetty.contains(first["page"]["url"].as_str().unwrap()));
    for line in first["page"]["lines"].as_array().unwrap() {
        assert!(human_at_jetty.contains(line.as_str().unwrap()));
    }

    let back = machine(&mut client, json!({"op":"back"}))["state"].clone();
    assert_eq!(back["page"], start["page"]);
    assert_eq!(back["forward"], true);
    writeln!(input, "1").unwrap(); // Human still sees the old page.
    assert!(prompt(&mut output).contains("Page changed"));
    let same = machine(
        &mut client,
        json!({"op":"follow","link":1,"revision":back["revision"]}),
    )["state"]
        .clone();
    assert_eq!(same["page"], first["page"]);
    assert_eq!(same["back"], first["back"]);
    assert_eq!(same["forward"], false);
    writeln!(input, "read").unwrap();
    assert!(prompt(&mut output).contains("High water at noon."));
    assert_eq!(
        machine(
            &mut client,
            json!({"op":"open","url":format!("{base}/bad")})
        )["ok"],
        false
    );
    assert_eq!(machine(&mut client, json!({"op":"read"}))["state"], same);
    writeln!(input, "back").unwrap();
    let previous = prompt(&mut output);
    assert!(previous.contains("Tide tables."));
    assert_eq!(
        machine(&mut client, json!({"op":"read"}))["state"]["page"],
        start["page"]
    );
    assert_eq!(
        machine(&mut client, json!({"op":"forward"}))["state"]["page"],
        first["page"]
    );
    assert_eq!(
        machine(&mut client, json!({"op":"reload"}))["state"]["page"],
        first["page"]
    );
    writeln!(input, "quit").unwrap();
    assert!(child.0.wait().unwrap().success());
}

#[test]
fn external_machine_binding_is_rejected() {
    let output = Command::new(env!("CARGO_BIN_EXE_surfbird"))
        .args(["--listen", "0.0.0.0:0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("loopback"));
}

#[test]
#[ignore = "fetches the current Rust Book over public HTTPS"]
fn rust_book_over_both_interfaces() {
    let (mut child, address) = launch();
    let mut input = child.0.stdin.take().unwrap();
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    let mut agent = Running(
        Command::new(env!("CARGO_BIN_EXE_surfbird"))
            .args(["--machine", &address])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut agent_in = agent.0.stdin.take().unwrap();
    let mut agent_out = BufReader::new(agent.0.stdout.take().unwrap());
    let mut act = |command: Value| {
        writeln!(agent_in, "{command}").unwrap();
        let mut line = String::new();
        agent_out.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["ok"], true, "{response}");
        response["state"].clone()
    };

    prompt(&mut output);
    writeln!(input, "open https://doc.rust-lang.org/book/").unwrap();
    prompt(&mut output);
    let start = act(json!({"op":"read"}));
    assert_eq!(start["page"]["status"], 200);
    let index = start["page"]["links"]
        .as_array()
        .unwrap()
        .iter()
        .position(|l| {
            l["url"]
                .as_str()
                .unwrap()
                .ends_with("/ch01-01-installation.html")
        })
        .unwrap()
        + 1;
    writeln!(input, "{index}").unwrap();
    let human = prompt(&mut output);
    let destination = act(json!({"op":"read"}));
    assert_eq!(destination["page"]["status"], 200);
    let previous = act(json!({"op":"back"}));
    assert_eq!(previous["page"], start["page"]);
    let repeated = act(json!({"op":"follow", "link":index, "revision":previous["revision"]}));
    assert_eq!(repeated["page"], destination["page"]);
    assert!(human.contains(destination["page"]["url"].as_str().unwrap()));
    for line in destination["page"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .take(20)
    {
        assert!(human.contains(line.as_str().unwrap()));
    }
    writeln!(input, "read").unwrap();
    let reread = prompt(&mut output);
    assert!(reread.contains(destination["page"]["title"].as_str().unwrap()));
    writeln!(input, "back").unwrap();
    prompt(&mut output);
    assert_eq!(act(json!({"op":"read"}))["page"], start["page"]);
    println!(
        "Both controls reached {} ({} lines, {} links); Back restored {}.",
        destination["page"]["url"],
        destination["page"]["lines"].as_array().unwrap().len(),
        destination["page"]["links"].as_array().unwrap().len(),
        start["page"]["url"]
    );
    writeln!(input, "quit").unwrap();
    assert!(child.0.wait().unwrap().success());
}
