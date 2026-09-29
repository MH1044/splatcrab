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
//! Since cycle U2 (acceptance test 14) it checks that a connection left
//! idle does not hold the interpreter from another, now that each
//! connection is read on a thread of its own, and runs `files` and
//! `workspace` with `preview` over the socket against a fixture folder it
//! makes under the temporary folder and starts the server in, so the
//! folder is the file root. Every server it starts has `SPLATCRAB_HISTORY`
//! pointing at a file of its own there, so nothing here can touch the
//! user's history.
//!
//!   cargo test --test ui_server

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);
const TOKEN: &str = "itest";

/// How many servers this run has started, so each one's history file and
/// fixture folder are its own.
static STARTED: AtomicUsize = AtomicUsize::new(0);

/// Kills the server however the test ends, and removes its history file.
struct Server {
    child: Child,
    port: u16,
    history: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.history);
    }
}

/// A fresh name under the temporary folder, unique to this run and call.
fn scratch(what: &str) -> PathBuf {
    let n = STARTED.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "splatcrab-uiserver-{}-{}-{what}",
        std::process::id(),
        n
    ))
}

/// Spawns the server in the test's own working directory.
fn start() -> Server {
    start_in(None)
}

/// Spawns the server, in `dir` when given, which is then its file root,
/// and reads the port from the line it prints.
fn start_in(dir: Option<&Path>) -> Server {
    let history = scratch("history");
    let _ = std::fs::remove_file(&history);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_splatcrab"));
    cmd.args(["--ui", "--port", "0", "--no-browser", "--token", TOKEN])
        .env("SPLATCRAB_HISTORY", &history)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    let mut child = cmd.spawn().expect("failed to spawn splatcrab --ui");
    let stdout = child.stdout.take().expect("stdout was piped");
    // The guard exists before anything can fail, so the child is killed
    // whatever happens next.
    let mut server = Server {
        child,
        port: 0,
        history,
    };

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
/// for long, is still dropped when the request's deadline passes, and a
/// client that connects after it is served within that deadline: since
/// cycle U2 at once, on a thread of its own, where U1 served it once the
/// trickler had been dropped.
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

/// Cycle U2: a connection opened and left idle, and one that sent half a
/// head, hold only their own threads, so an `eval` on another connection is
/// answered at once rather than after their ten seconds.
#[test]
fn an_idle_connection_does_not_hold_the_interpreter() {
    let server = start();
    let port = server.port;
    let authorised = format!("Host: 127.0.0.1:{port}\r\nX-SplatCrab-Token: {TOKEN}\r\n");
    let _idle = TcpStream::connect(("127.0.0.1", port)).expect("an idle connection");
    let mut half = TcpStream::connect(("127.0.0.1", port)).expect("a half-sent connection");
    half.write_all(b"POST /api HTTP/1.1\r\nHost: 127")
        .expect("half a head");
    // Let both be accepted, and so reach their threads, first.
    std::thread::sleep(Duration::from_millis(300));
    let began = Instant::now();
    let reply = api(
        port,
        &authorised,
        r#"{"id":1,"op":"eval","code":"disp(6 * 7)"}"#,
    );
    let took = began.elapsed();
    assert_eq!(body(&reply), r#"{"id":1,"ok":true,"out":"    42\n"}"#);
    assert!(took < Duration::from_secs(3), "answered after {took:?}");
}

/// A folder of the test's own, removed however the test ends.
struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The acceptance tests' fixture, `U2-ui-desktop/tree/` holding `Z.txt`
/// (the 2 bytes `xy`), `a.txt` (empty), `b.txt` (`hello`), `sub/deep.txt`
/// (`abc`) and `emptyish/.keep`, under a folder of this run's own. Returns
/// the guard and the `U2-ui-desktop` folder, which the server is started in.
fn fixture() -> (Fixture, PathBuf) {
    let top = scratch("fixture");
    let _ = std::fs::remove_dir_all(&top);
    let root = top.join("U2-ui-desktop");
    let tree = root.join("tree");
    for dir in [tree.join("sub"), tree.join("emptyish")] {
        std::fs::create_dir_all(dir).expect("make the fixture's folders");
    }
    for (file, text) in [
        ("Z.txt", "xy"),
        ("a.txt", ""),
        ("b.txt", "hello"),
        ("sub/deep.txt", "abc"),
        ("emptyish/.keep", ""),
    ] {
        std::fs::write(tree.join(file), text).expect("write a fixture file");
    }
    (Fixture(top), root)
}

/// Cycle U2's acceptance test 14: `files` and `workspace` with `preview`
/// answer over the socket as their golden cases pin them, the root being
/// the folder the server started in, and the page, its script and its
/// stylesheet are still served.
#[test]
fn files_and_previews_answer_over_the_socket() {
    let (_guard, root) = fixture();
    let server = start_in(Some(&root));
    let port = server.port;
    let host = format!("Host: 127.0.0.1:{port}\r\n");
    let authorised = format!("{host}X-SplatCrab-Token: {TOKEN}\r\n");

    let reply = api(port, &authorised, r#"{"id":1,"op":"files","path":"tree"}"#);
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
    assert_eq!(
        body(&reply),
        concat!(
            r#"{"id":1,"ok":true,"root":"U2-ui-desktop","path":"tree","entries":["#,
            r#"{"name":"emptyish","dir":true,"size":null},"#,
            r#"{"name":"sub","dir":true,"size":null},"#,
            r#"{"name":"Z.txt","dir":false,"size":2},"#,
            r#"{"name":"a.txt","dir":false,"size":0},"#,
            r#"{"name":"b.txt","dir":false,"size":5}],"truncated":false}"#
        )
    );
    let reply = api(port, &authorised, r#"{"id":2,"op":"files","path":".."}"#);
    assert_eq!(
        body(&reply),
        r#"{"id":2,"ok":false,"error":{"message":"Path '..' is outside the file root.","line":null}}"#
    );

    let code = "x = 3; v = [1 2 3]; m = [1 2; 3 4]; s = 'it''s'; e = []; big = 1:11; \
                z = 1+2i; zz = [1+2i 3]; t = true; c = {1}; f = @(x) x+1; st.a = 1; \
                ch = ['ab'; 'cd'];";
    let reply = api(
        port,
        &authorised,
        &format!(r#"{{"id":3,"op":"eval","code":"{code}"}}"#),
    );
    assert_eq!(body(&reply), r#"{"id":3,"ok":true,"out":""}"#);
    let reply = api(
        port,
        &authorised,
        r#"{"id":4,"op":"workspace","preview":true}"#,
    );
    assert_eq!(
        body(&reply),
        concat!(
            r#"{"id":4,"ok":true,"vars":["#,
            r#"{"name":"big","size":[1,11],"class":"double","value":"1×11 double"},"#,
            r#"{"name":"c","size":[1,1],"class":"cell","value":"1×1 cell"},"#,
            r#"{"name":"ch","size":[2,2],"class":"char","value":"2×2 char"},"#,
            r#"{"name":"e","size":[0,0],"class":"double","value":"0×0 double"},"#,
            r#"{"name":"f","size":[1,1],"class":"function_handle","value":"@(x)x+1"},"#,
            r#"{"name":"m","size":[2,2],"class":"double","value":"[1,2;3,4]"},"#,
            r#"{"name":"s","size":[1,4],"class":"char","value":"'it''s'"},"#,
            r#"{"name":"st","size":[1,1],"class":"struct","value":"1×1 struct"},"#,
            r#"{"name":"t","size":[1,1],"class":"logical","value":"1"},"#,
            r#"{"name":"v","size":[1,3],"class":"double","value":"[1,2,3]"},"#,
            r#"{"name":"x","size":[1,1],"class":"double","value":"3"},"#,
            r#"{"name":"z","size":[1,1],"class":"double","value":"1.0000 + 2.0000i"},"#,
            r#"{"name":"zz","size":[1,2],"class":"double","value":"[1.0000 + 2.0000i,3]"}]}"#
        )
    );
    // The root stays where the server started, whatever `cd` does.
    let reply = api(
        port,
        &authorised,
        r#"{"id":5,"op":"eval","code":"cd tree"}"#,
    );
    assert_eq!(body(&reply), r#"{"id":5,"ok":true,"out":""}"#);
    let reply = api(
        port,
        &authorised,
        r#"{"id":6,"op":"files","path":"tree/sub"}"#,
    );
    assert!(
        body(&reply).starts_with(r#"{"id":6,"ok":true,"root":"U2-ui-desktop","path":"tree/sub","#),
        "{reply}"
    );

    for (path, starts) in [
        ("/", "<!doctype html>"),
        ("/app.js", "// SplatCrab"),
        ("/app.css", "/* SplatCrab"),
    ] {
        let reply = exchange(port, &format!("GET {path} HTTP/1.1\r\n{host}\r\n"));
        assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{path}: {reply}");
        assert!(body(&reply).starts_with(starts), "{path}");
    }
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
