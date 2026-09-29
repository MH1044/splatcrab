//! `splatcrab --ui`: the loopback socket around [`http::handle`].
//!
//! The listener is bound to `127.0.0.1` and nothing else. Since cycle U2
//! each connection is read on a thread of its own, at most
//! [`MAX_CONNECTIONS`] at once, a connection past that closed at once with
//! no answer: the thread reads one request under [`http::read_request`]'s
//! limits and a deadline and hands it, whole, through a channel to the
//! calling thread, which owns the session's one [`Interp`] and answers
//! requests one at a time in the order they arrived whole; the connection's
//! own thread then writes the answer under a deadline of its own, lingers
//! and closes. So an idle or trickling connection holds only its own
//! thread, never the interpreter, and one request's linger never delays
//! the next. The interpreter never leaves its thread, which it could not:
//! it holds `Rc`s. Nothing a client does ends the loop; a connection that
//! fails is dropped.
//!
//! The security model is in `docs/modules/U1-ui-server.md` and
//! `docs/ARCHITECTURE.md`: loopback only, the session token in a custom
//! header, `Host` and `Origin` checked, the limits enforced before
//! buffering. All of it but the binding is decided in `http.rs`, where it is
//! unit-tested without a socket.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufReader, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::http::{self, Config};
use crate::interp::Interp;

/// A connection whose request has not arrived this long after it was
/// accepted is closed, and so is one whose response has not been taken this
/// long after writing began. The limit is on the whole request, not on each
/// read, so a client that trickles a byte at a time cannot hold its
/// connection's thread for longer; a read that stalls this long is caught
/// by the same rule.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// The most connections read, waiting for their answer or writing it at
/// once. A connection accepted past it is closed at once, unanswered.
pub const MAX_CONNECTIONS: usize = 16;

/// After answering, unread input is drained for at most this long and this
/// many bytes before the socket closes, so that the client reads the answer
/// rather than a reset: closing a socket with input still unread makes the
/// operating system send RST, which can overtake the response.
const LINGER: Duration = Duration::from_secs(1);
const LINGER_BYTES: usize = 1024 * 1024;

/// Binds `127.0.0.1:<port>`, where 0 lets the operating system pick.
pub fn bind(port: u16) -> io::Result<TcpListener> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
}

/// The address the user opens: the token rides in the fragment, which a
/// browser never sends to a server or puts in a `Referer`.
pub fn url(port: u16, token: &str) -> String {
    format!("http://127.0.0.1:{}/#{}", port, token)
}

/// A fresh 128-bit session token as 32 lower-case hex digits.
///
/// Each half is the standard library's randomly keyed SipHash, whose keys
/// come from the operating system's random source once per thread and are
/// stepped for every `RandomState`, over the time and the process id. No
/// crate: the project takes none.
pub fn new_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let pid = std::process::id();
    let mut token = String::with_capacity(32);
    for half in 0u8..2 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_u32(pid);
        h.write_u8(half);
        token.push_str(&format!("{:016x}", h.finish()));
    }
    token
}

/// Tries to open `url` in the default browser, and ignores any failure: the
/// URL is printed already, and the user can open it by hand. The REPL
/// opens a figure's temporary SVG file the same way (cycle 12), since each
/// launcher opens a path with the program the system gives its type.
pub fn open_browser(url: &str) {
    let mut cmd = if cfg!(windows) {
        // `start` is a shell built-in; its first quoted argument is a window
        // title, so an empty one keeps the URL from being taken for it.
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Ok(mut child) = cmd.spawn() {
        // Reaped on a thread of its own, so the launcher is not left behind
        // as a zombie and the server does not wait for it.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// How many connections are open, against a bound: taking a [`Slot`]
/// counts one more, and dropping it one fewer.
#[derive(Clone)]
struct Slots {
    open: Arc<AtomicUsize>,
    max: usize,
}

/// One connection's place under the bound, given back when it drops.
struct Slot(Arc<AtomicUsize>);

impl Slots {
    fn new(max: usize) -> Self {
        Slots {
            open: Arc::new(AtomicUsize::new(0)),
            max,
        }
    }

    /// A slot, or `None` when every one is taken.
    fn take(&self) -> Option<Slot> {
        self.open
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < self.max).then_some(n + 1)
            })
            .ok()
            .map(|_| Slot(self.open.clone()))
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// One whole request on its way to the interpreter thread, and where its
/// answer goes back.
struct Job {
    request: Vec<u8>,
    reply: Sender<Vec<u8>>,
}

/// The exit code of a panic on the interpreter thread: the code `main`
/// gives a panic that reaches its join, so `--ui` exits as it did in
/// cycle U1.
const PANIC_EXIT: i32 = 101;

/// Serves connections on `listener` for ever: accepted on a thread of their
/// own, read on one thread each, answered here.
///
/// A panic while answering ends the process with [`PANIC_EXIT`], after the
/// default hook has printed its message. Left to unwind, it would never
/// reach `main`: the scope waits for the accepting thread, which loops for
/// ever, so `--ui` would hang instead of exiting. A panic on a
/// connection's thread costs only that connection: its slot is given back
/// as the thread unwinds, and the interpreter never sees it.
pub fn serve(listener: &TcpListener, it: &mut Interp, cfg: &Config) -> ! {
    std::thread::scope(|scope| {
        let (jobs, requests) = mpsc::channel::<Job>();
        let accepting = std::thread::Builder::new()
            .name("accept".to_string())
            .spawn_scoped(scope, move || accept(listener, &jobs));
        if accepting.is_ok() {
            if let Some(code) = guarded(|| answer(&requests, it, cfg)) {
                std::process::exit(code);
            }
        }
    });
    // Reached only when no thread could be started to accept on: then
    // connections are served one at a time here, as before cycle U2.
    serve_in_turn(listener, it, cfg)
}

/// Runs `f` and gives the exit code the process must end with because it
/// panicked, [`PANIC_EXIT`], or `None` when it returned. The panic is
/// caught, never resumed: the caller exits, so nothing `f` left half done
/// is ever looked at again, which is what makes `AssertUnwindSafe` sound.
fn guarded(f: impl FnOnce()) -> Option<i32> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .err()
        .map(|_| PANIC_EXIT)
}

/// The interpreter's loop: each request in the order it arrived whole,
/// answered and handed back. Returns only when no connection can send one
/// again, which the accepting thread, holding a sender for ever, never
/// allows.
fn answer(requests: &Receiver<Job>, it: &mut Interp, cfg: &Config) {
    answer_with(requests, |request| http::handle(request, it, cfg));
}

/// [`answer`]'s loop around any handler, so a unit test can hand it one
/// that panics.
fn answer_with(requests: &Receiver<Job>, mut handle: impl FnMut(&[u8]) -> Vec<u8>) {
    while let Ok(job) = requests.recv() {
        let reply = handle(&job.request);
        // A connection that has gone away takes no answer.
        let _ = job.reply.send(reply);
    }
}

/// Accepts for ever, each connection read on a thread of its own while a
/// slot is free and closed at once, unanswered, while none is.
fn accept(listener: &TcpListener, jobs: &Sender<Job>) {
    let slots = Slots::new(MAX_CONNECTIONS);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let Some(slot) = slots.take() else {
                    drop(stream);
                    continue;
                };
                let jobs = jobs.clone();
                // A thread that cannot be started drops its connection and
                // its slot with the closure.
                let _ = std::thread::Builder::new()
                    .name("connection".to_string())
                    .spawn(move || connection(stream, &jobs, slot));
            }
            // Accepting can fail for want of a descriptor; pause rather than
            // spin while that lasts.
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// One connection, on its own thread: read the request, have the
/// interpreter thread answer it, write the answer and close. The slot is
/// held until the connection is closed.
fn connection(stream: TcpStream, jobs: &Sender<Job>, _slot: Slot) {
    let Ok(Some(request)) = read_one(&stream) else {
        return;
    };
    let (reply, answer) = mpsc::channel();
    if jobs.send(Job { request, reply }).is_err() {
        return;
    }
    if let Ok(bytes) = answer.recv() {
        let _ = write_one(&stream, &bytes);
    }
}

/// Serves connections one at a time on this thread, for ever: the fallback
/// when no thread can be started to accept on.
fn serve_in_turn(listener: &TcpListener, it: &mut Interp, cfg: &Config) -> ! {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Ok(Some(request)) = read_one(&stream) {
                    let reply = http::handle(&request, it, cfg);
                    let _ = write_one(&stream, &reply);
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Reads one request under the limits and the deadline. `None` when the
/// client closes without sending a byte, which gets no answer.
fn read_one(stream: &TcpStream) -> io::Result<Option<Vec<u8>>> {
    let mut reader = BufReader::new(Timed::new(stream));
    Ok(http::read_request(&mut reader, false)?.map(|r| r.bytes))
}

/// Writes an answer under a deadline of its own, since the evaluation's
/// time is not the client's, then lingers and closes.
fn write_one(stream: &TcpStream, reply: &[u8]) -> io::Result<()> {
    let mut writer = Timed::new(stream);
    writer.write_all(reply)?;
    writer.flush()?;
    linger(stream);
    Ok(())
}

/// The stream under one deadline, [`TIMEOUT`] from when it was made: each
/// read or write may wait only for what is left of it, and none may start
/// once it has passed.
struct Timed<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
}

impl<'a> Timed<'a> {
    fn new(stream: &'a TcpStream) -> Self {
        Timed {
            stream,
            deadline: Instant::now() + TIMEOUT,
        }
    }

    /// What is left of the deadline, or `TimedOut` when nothing is. Never
    /// zero, since a zero timeout is refused by the socket.
    fn left(&self) -> io::Result<Duration> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(left)
        }
    }
}

impl Read for Timed<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(self.left()?))?;
        (&*self.stream).read(buf)
    }
}

impl Write for Timed<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.left()?))?;
        (&*self.stream).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self.stream).flush()
    }
}

/// Half-closes the connection and drains what the client may still be
/// sending, within [`LINGER`] and [`LINGER_BYTES`], before it is dropped.
fn linger(stream: &TcpStream) {
    let _ = stream.shutdown(Shutdown::Write);
    // One deadline for the whole drain, so a client that keeps trickling
    // cannot stretch it a read at a time.
    let deadline = Instant::now() + LINGER;
    let mut scratch = [0u8; 4096];
    let mut drained = 0;
    while drained < LINGER_BYTES {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match (&*stream).read(&mut scratch) {
            Ok(0) | Err(_) => break,
            Ok(n) => drained += n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_32_lower_case_hex_digits_and_fresh_each_time() {
        let a = new_token();
        assert_eq!(a.len(), 32);
        assert!(
            a.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_ne!(a, new_token());
        assert_ne!(a[..16], a[16..]);
    }

    #[test]
    fn the_url_carries_the_token_in_its_fragment() {
        assert_eq!(url(8123, "abc"), "http://127.0.0.1:8123/#abc");
    }

    /// Acceptance test 15: the connection bound. Sixteen slots are handed
    /// out, the seventeenth is refused, and a slot given back is free again,
    /// from any thread.
    #[test]
    fn the_connection_bound_holds_and_frees() {
        let slots = Slots::new(MAX_CONNECTIONS);
        let mut held: Vec<Slot> = (0..MAX_CONNECTIONS)
            .map(|k| slots.take().unwrap_or_else(|| panic!("slot {k}")))
            .collect();
        assert!(slots.take().is_none(), "one past the bound");
        assert!(slots.clone().take().is_none(), "a clone shares the count");
        let one = held.pop().unwrap();
        std::thread::spawn(move || drop(one)).join().unwrap();
        let again = slots.take().expect("a freed slot is free");
        assert!(slots.take().is_none());
        drop(again);
        drop(held);
        assert_eq!(slots.open.load(Ordering::Acquire), 0);
        let none = Slots::new(0);
        assert!(none.take().is_none());
    }

    /// The interpreter thread answers the jobs in the order they arrive,
    /// each on its own reply channel, and a job whose connection has gone
    /// does not stop the ones after it.
    #[test]
    fn jobs_are_answered_in_arrival_order() {
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        let cfg = Config {
            port: 8123,
            token: "t".to_string(),
        };
        let (jobs, requests) = mpsc::channel();
        let mut answers = Vec::new();
        for (k, code) in ["x = 1;", "x = x + 1;", "disp(x)"].iter().enumerate() {
            let body = format!("{{\"id\":{k},\"op\":\"eval\",\"code\":\"{code}\"}}");
            let request = format!(
                "POST /api HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nX-SplatCrab-Token: t\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let (reply, answer) = mpsc::channel();
            jobs.send(Job {
                request: request.into_bytes(),
                reply,
            })
            .unwrap();
            answers.push(answer);
        }
        // The first connection has gone away before its answer.
        drop(answers.remove(0));
        drop(jobs);
        answer(&requests, &mut it, &cfg);
        let last = String::from_utf8(answers[1].recv().unwrap()).unwrap();
        assert!(
            last.ends_with("{\"id\":2,\"ok\":true,\"out\":\"     2\\n\"}"),
            "{last}"
        );
    }

    /// Cycle U2's review: a panic on the interpreter thread is caught and
    /// reported as the exit code 101, which `serve` then exits with, rather
    /// than left to unwind into a scope that waits for ever on the
    /// accepting thread. A loop that returns reports nothing.
    #[test]
    fn a_panic_while_answering_is_caught_and_reported_as_101() {
        let (jobs, requests) = mpsc::channel();
        let (reply, answer) = mpsc::channel();
        jobs.send(Job {
            request: b"anything".to_vec(),
            reply,
        })
        .unwrap();
        let mut handled = 0;
        let code = guarded(|| {
            answer_with(&requests, |_| {
                handled += 1;
                panic!("a handler that fails");
            })
        });
        assert_eq!(code, Some(101));
        assert_eq!(handled, 1);
        // The job's reply channel went with the unwind: its connection is
        // told there is no answer rather than left waiting.
        assert!(answer.recv().is_err());
        // With no sender left, the loop returns, and nothing is reported.
        drop(jobs);
        assert_eq!(guarded(|| answer_with(&requests, |r| r.to_vec())), None);
    }
}
