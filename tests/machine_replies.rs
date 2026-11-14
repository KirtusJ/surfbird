use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    thread,
};

fn run_client(send: impl FnOnce(TcpStream) + Send + 'static) -> Output {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let peer = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut command = String::new();
        BufReader::new(&mut socket).read_line(&mut command).unwrap();
        assert_eq!(command.trim(), r#"{"op":"read"}"#);
        send(socket);
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_surfbird"))
        .args(["--machine", &address.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"op\":\"read\"}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    peer.join().unwrap();
    output
}

#[test]
fn oversized_reply_is_rejected_before_any_output() {
    let result = run_client(|mut socket| {
        let chunk = [b'x'; 16 * 1024];
        for _ in 0..1024 {
            socket.write_all(&chunk).unwrap();
        }
        let _ = socket.write_all(b"\n");
    });
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("exceeds 16 MiB"));
}

#[test]
fn disconnects_never_publish_an_incomplete_reply() {
    for bytes in [b"".as_slice(), b"{\"ok\":true}"] {
        let result = run_client(move |mut socket| socket.write_all(bytes).unwrap());
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8(result.stderr)
            .unwrap()
            .contains("disconnected"));
    }
}
