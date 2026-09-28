//! The UI server over a real socket (cycle U1, acceptance test 12).
//!
//! The HTTP layer is pinned byte for byte by `src/http.rs`'s unit tests and
//! the `.http` golden cases, which drive it without a socket. This test
//! covers what they cannot: that `splatcrab --ui` binds a loopback port,
//! prints its URL, and answers over TCP. It spawns
//! `--ui --port 0 --no-browser --token itest`, so the system picks a free
//! port and no browser opens, and it connects to `127.0.0.1` only; it binds
//! nothing itself. The child is killed by a guard's `Drop`, so a failed
//! assertion kills it too.
//!
//! It also checks the command-line refusals of `--ui` and `--http-stdio`,
//! which no golden case kind can reach: every case runs with fixed flags.
//!
//!   cargo test --test ui_server

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);
const TOKEN: &str = "itest";

/// Kills the server however the test ends.
struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawns the server and reads the port from the line it prints.
fn start() -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splatcrab"))
        .args(["--ui", "--port", "0", "--no-browser", "--token", TOKEN])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn splatcrab --ui");
    let stdout = child.stdout.take().expect("stdout was piped");
    // The guard exists before anything can fail, so the child is killed
    // whatever happens next.
    let mut server = Server { child, port: 0 };

    // Read on a thread, so a server that never prints fails the test after
    // the timeout instead of hanging it.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx
        .recv_timeout(TIMEOUT)
        .expect("the server printed no URL in time");
    let line = line.trim_end();
    let rest = line
        .strip_prefix("SplatCrab UI: http://127.0.0.1:")
        .unwrap_or_else(|| panic!("unexpected first line {line:?}"));
    let (port, token) = rest
        .split_once("/#")
        .unwrap_or_else(|| panic!("no fragment in {line:?}"));
    assert_eq!(token, TOKEN, "the URL carries the token in its fragment");
    server.port = port.parse().expect("a port number");
    assert_ne!(server.port, 0, "port 0 means the system picks one");
    server
}

/// Sends one raw request and reads the whole response: the server closes the
/// connection after every one.
fn exchange(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the server");
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    stream.set_write_timeout(Some(TIMEOUT)).unwrap();
    stream
        .write_all(request.as_bytes())
        .expect("send the request");
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).expect("read the response");
    String::from_utf8(reply).expect("a UTF-8 response")
}

fn api(port: u16, headers: &str, body: &str) -> String {
    exchange(
        port,
        &format!(
            "POST /api HTTP/1.1\r\n{headers}Content-Type: application/json\r\n\
             Content-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    )
}

fn body(reply: &str) -> &str {
    reply
        .split_once("\r\n\r\n")
        .expect("a blank line ends the head")
        .1
}

#[test]
fn the_server_answers_over_a_loopback_socket() {
    let server = start();
    let port = server.port;
    let host = format!("Host: 127.0.0.1:{port}\r\n");
    let authorised = format!("{host}X-SplatCrab-Token: {TOKEN}\r\n");

    // An eval round trip, and a second request in the same session.
    let reply = api(
        port,
        &authorised,
        r#"{"id":1,"op":"eval","code":"x = 1 + 2"}"#,
    );
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
    assert!(reply.contains("\r\nContent-Length: 44\r\n"), "{reply}");
    assert_eq!(
        body(&reply),
        r#"{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}"#
    );
    let reply = api(
        port,
        &authorised,
        r#"{"id":2,"op":"eval","code":"disp(x * 2)"}"#,
    );
    assert_eq!(body(&reply), r#"{"id":2,"ok":true,"out":"     6\n"}"#);

    // Refused without the token, and for a foreign Host.
    let reply = api(port, &host, r#"{"id":3,"op":"eval","code":"y = 1"}"#);
    assert!(reply.starts_with("HTTP/1.1 403 Forbidden\r\n"), "{reply}");
    assert_eq!(body(&reply), "403 Forbidden");
    let foreign = format!("Host: evil.example:{port}\r\nX-SplatCrab-Token: {TOKEN}\r\n");
    let reply = api(port, &foreign, r#"{"id":4,"op":"eval","code":"y = 1"}"#);
    assert!(reply.starts_with("HTTP/1.1 403 Forbidden\r\n"), "{reply}");
    // Neither refusal reached the interpreter.
    let reply = api(port, &authorised, r#"{"id":5,"op":"workspace"}"#);
    assert_eq!(
        body(&reply),
        r#"{"id":5,"ok":true,"vars":[{"name":"x","size":[1,1],"class":"double"}]}"#
    );

    // The page, with its Content-Security-Policy.
    let reply = exchange(port, &format!("GET / HTTP/1.1\r\n{host}\r\n"));
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
    assert!(
        reply.contains(
            "\r\nContent-Security-Policy: default-src 'self'; frame-ancestors 'none'\r\n"
        ),
        "{reply}"
    );
    assert!(body(&reply).starts_with("<!doctype html>"));

    // A client that connects and leaves without a word does not end the
    // server, and neither does one that sends half a request and hangs up.
    drop(TcpStream::connect(("127.0.0.1", port)).unwrap());
    {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(b"POST /api HTTP/1.1\r\nHost: 127").unwrap();
    }
    let reply = exchange(port, &format!("GET /app.css HTTP/1.1\r\n{host}\r\n"));
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
}

/// A client that trickles its head a byte at a time, never stalling a read
/// for long, is still dropped when the request's deadline passes, and the
/// client queued behind it is then served: the one connection the server
/// serves cannot be held.
#[test]
fn a_trickling_client_is_dropped_at_the_deadline() {
    let server = start();
    let port = server.port;
    let began = Instant::now();
    let trickler = std::thread::spawn(move || {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let head = format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX: ");
        s.write_all(head.as_bytes()).unwrap();
        // Returns how long the server kept the connection, or `None` if it
        // was still open after twice the deadline.
        while began.elapsed() < 2 * TIMEOUT {
            if s.write_all(b"a").is_err() {
                return Some(began.elapsed());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        None
    });
    std::thread::sleep(Duration::from_millis(500));

    let mut next = TcpStream::connect(("127.0.0.1", port)).unwrap();
    next.set_read_timeout(Some(2 * TIMEOUT)).unwrap();
    next.write_all(format!("GET /app.css HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes())
        .unwrap();
    let mut reply = Vec::new();
    next.read_to_end(&mut reply)
        .expect("the queued client is served");
    let served = began.elapsed();
    assert!(reply.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert!(served < TIMEOUT + Duration::from_secs(3), "{served:?}");

    let dropped = trickler.join().unwrap().expect("the trickler was dropped");
    assert!(dropped < TIMEOUT + Duration::from_secs(3), "{dropped:?}");
}

/// Kills a child however the test ends; `refused` runs under one, so an
/// option that regresses into starting the server is killed rather than left
/// serving forever.
struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs the binary with `args`, which must make it exit within the timeout,
/// and returns its exit code and stderr. Every `--ui` caller passes
/// `--no-browser` first, so a regression cannot open a browser either.
fn refused(args: &[&str]) -> (Option<i32>, String) {
    let child = Command::new(env!("CARGO_BIN_EXE_splatcrab"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run splatcrab");
    let mut guard = Guard(child);
    let mut so = guard.0.stdout.take().expect("stdout was piped");
    let mut se = guard.0.stderr.take().expect("stderr was piped");
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        so.read_to_end(&mut b).ok();
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        se.read_to_end(&mut b).ok();
        b
    });
    let start = Instant::now();
    let status = loop {
        if let Some(s) = guard.0.try_wait().expect("try_wait failed") {
            break s;
        }
        assert!(
            start.elapsed() < TIMEOUT,
            "{args:?} did not exit within {TIMEOUT:?}; it may be serving"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = t_out.join().expect("stdout thread");
    let stderr = t_err.join().expect("stderr thread");
    assert!(stdout.is_empty(), "nothing on stdout for {args:?}");
    (status.code(), String::from_utf8_lossy(&stderr).into_owned())
}

#[test]
fn bad_options_are_clean_errors() {
    for (args, message) in [
        (
            &["--ui", "--no-browser", "--port", "http"][..],
            "Error: Option '--port' needs a port number from 0 to 65535, not 'http'.",
        ),
        (
            &["--ui", "--no-browser", "--port", "70000"],
            "Error: Option '--port' needs a port number from 0 to 65535, not '70000'.",
        ),
        (
            &["--ui", "--no-browser", "--token"],
            "Error: Option '--token' needs a value.",
        ),
        (
            &["--ui", "--no-browser", "--token", "a b"],
            "Error: Option '--token' needs letters, digits, '.', '_', '~' or '-' only.",
        ),
        (
            &["--ui", "--no-browser", "--token", "a&calc"],
            "Error: Option '--token' needs letters, digits, '.', '_', '~' or '-' only.",
        ),
        (
            &["--ui", "--no-browser", "--verbose"],
            "Error: Unknown option '--verbose' for --ui.",
        ),
        (
            &[
                "--http-stdio",
                "--port",
                "8123",
                "--no-browser",
                "--token",
                "t",
            ],
            "Error: Unknown option '--no-browser' for --http-stdio.",
        ),
        (
            &["--http-stdio", "--token", "t"],
            "Error: --http-stdio needs the option '--port'.",
        ),
        (
            &["--http-stdio", "--port", "8123"],
            "Error: --http-stdio needs the option '--token'.",
        ),
    ] {
        let (code, stderr) = refused(args);
        assert_eq!(code, Some(1), "{args:?}: {stderr}");
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

#[test]
fn a_port_in_use_is_a_clean_error() {
    // Hold a loopback port, then ask the server for it.
    let held = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = held.local_addr().expect("its address").port().to_string();
    let (code, stderr) = refused(&["--ui", "--no-browser", "--port", &port, "--token", TOKEN]);
    assert_eq!(code, Some(1), "{stderr}");
    let expected = format!("Error: Cannot listen on 127.0.0.1:{port}: ");
    assert!(stderr.starts_with(&expected), "{stderr}");
    drop(held);
}

#[test]
fn http_stdio_exits_zero_at_end_of_input() {
    let out = Command::new(env!("CARGO_BIN_EXE_splatcrab"))
        .args(["--http-stdio", "--port", "8123", "--token", "t"])
        .stdin(Stdio::null())
        .output()
        .expect("failed to run splatcrab");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}
