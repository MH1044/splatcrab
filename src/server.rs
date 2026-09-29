//! `splatcrab --ui`: the loopback socket around [`http::handle`].
//!
//! The listener is bound to `127.0.0.1` and nothing else. Connections are
//! served one at a time on the calling thread, which owns the session's one
//! [`Interp`]: accept, read one request under [`http::read_request`]'s
//! limits and a deadline, answer it, close. Nothing a client does ends
//! the loop; a connection that fails is dropped and the next is accepted.
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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::http::{self, Config};
use crate::interp::Interp;

/// A connection whose request has not arrived this long after it was
/// accepted is closed, and so is one whose response has not been taken this
/// long after writing began. The limit is on the whole request, not on each
/// read, so a client that trickles a byte at a time cannot hold the one
/// connection the server serves; a read that stalls this long is caught by
/// the same rule.
pub const TIMEOUT: Duration = Duration::from_secs(10);

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

/// Serves connections on `listener` one at a time, for ever.
pub fn serve(listener: &TcpListener, it: &mut Interp, cfg: &Config) -> ! {
    loop {
        match listener.accept() {
            // A connection that fails is that client's problem, not the
            // server's: it is dropped and the loop goes on.
            Ok((stream, _)) => {
                let _ = serve_one(stream, it, cfg);
            }
            // Accepting can fail for want of a descriptor; pause rather than
            // spin while that lasts.
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Reads one request, answers it and closes the connection.
fn serve_one(stream: TcpStream, it: &mut Interp, cfg: &Config) -> io::Result<()> {
    let request = {
        let mut reader = BufReader::new(Timed::new(&stream));
        http::read_request(&mut reader, false)?
    };
    // A client that connects and closes without a byte gets no answer.
    let Some(request) = request else {
        return Ok(());
    };
    // The evaluation is not the client's time: the write gets its own.
    let reply = http::handle(&request.bytes, it, cfg);
    let mut writer = Timed::new(&stream);
    writer.write_all(&reply)?;
    writer.flush()?;
    linger(&stream);
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
}
