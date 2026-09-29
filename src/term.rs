//! The raw-mode terminal around the line editor (cycle 13).
//!
//! The editing itself is `splatcrab::editor`, a pure state machine the unit
//! tests drive; this module is the thin shell the binary owns: it puts the
//! terminal into raw mode for exactly as long as one line is being read,
//! feeds what the terminal sends through the editor's decoder, draws the
//! line after each key, and puts the terminal back before the line is
//! returned. The restore is a guard's `Drop`, so it runs on every way out:
//! a submitted line, Ctrl-C, Ctrl-D, a read error and a panic alike, and
//! the interpreter always runs, and `input` always reads, in the terminal's
//! normal mode.
//!
//! Raw mode is reached through raw declarations of the platform's own calls,
//! as `main.rs` declares `SetConsoleOutputCP`, because the crate takes no
//! dependencies: `GetConsoleMode`, `SetConsoleMode` and `ReadConsoleW` from
//! `kernel32` on Windows, with virtual-terminal input and output, so both
//! platforms send and draw the same ANSI sequences; `tcgetattr`,
//! `tcsetattr` and `cfmakeraw` from the C library elsewhere. The REPL uses
//! this only when standard input and standard output are both terminals; a
//! pipe reads plain lines exactly as before, so nothing a golden case or CI
//! runs ever reaches this code.
//!
//! This module and `main.rs` are the only places allowed to `print!`.

use std::io::{self, Write};
use std::path::PathBuf;

use splatcrab::editor::{Action, Decoder, Editor};
use splatcrab::history;

/// What reading a line gave.
pub enum Line {
    /// A submitted line, without its line end.
    Text(String),
    /// Ctrl-C: the line was cleared.
    Cleared,
    /// Ctrl-D on an empty line, or the input ended.
    Eof,
}

/// The line editor over the terminal, with its history.
pub struct LineReader {
    editor: Editor,
    decoder: Decoder,
    /// The history file, when there is one to read and extend.
    file: Option<PathBuf>,
}

impl LineReader {
    /// A reader over the terminal, with the history file loaded, or `None`
    /// when the terminal cannot be put into raw mode, in which case the
    /// REPL reads plain lines.
    pub fn new() -> Option<LineReader> {
        drop(Raw::enter()?);
        let file = history::default_path();
        let entries = file.as_deref().map(history::load).unwrap_or_default();
        Some(LineReader {
            editor: Editor::new(entries),
            decoder: Decoder::default(),
            file,
        })
    }

    /// Reads one line after `prompt`. `complete` lists the names starting
    /// with a prefix, for Tab. A submitted line that is not blank or a
    /// repeat goes into the history and onto the end of its file; a
    /// failure to write the file is ignored.
    pub fn read_line(
        &mut self,
        prompt: &str,
        complete: &mut dyn FnMut(&str) -> Vec<String>,
    ) -> Line {
        let Some(mut raw) = Raw::enter() else {
            return Line::Eof;
        };
        draw(prompt, &self.editor);
        loop {
            let Some(c) = raw.read_char() else {
                print!("\r\n");
                flush();
                return Line::Eof;
            };
            let Some(key) = self.decoder.feed(c) else {
                continue;
            };
            match self.editor.key(key, complete) {
                Action::Redraw => draw(prompt, &self.editor),
                Action::Submit(line) => {
                    print!("\r\n");
                    flush();
                    drop(raw);
                    if self.editor.remember(&line) {
                        if let Some(f) = &self.file {
                            let _ = history::append(f, &line);
                        }
                    }
                    return Line::Text(line);
                }
                Action::Cleared => {
                    print!("^C\r\n");
                    flush();
                    return Line::Cleared;
                }
                Action::Eof => {
                    print!("\r\n");
                    flush();
                    return Line::Eof;
                }
                Action::List(names) => {
                    print!("\r\n{}\r\n", names.join("  "));
                    draw(prompt, &self.editor);
                }
            }
        }
    }
}

/// Draws the prompt and the line over the current terminal line, clears
/// whatever was left after it, and moves the cursor back to its place.
fn draw(prompt: &str, editor: &Editor) {
    print!("\r{}{}\x1b[K", prompt, editor.buffer());
    let back = editor.after_cursor();
    if back > 0 {
        print!("\x1b[{}D", back);
    }
    flush();
}

fn flush() {
    io::stdout().flush().ok();
}

#[cfg(windows)]
use windows::Raw;

#[cfg(unix)]
use unix::Raw;

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;

    type Handle = *mut c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> Handle;
        fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
        fn SetConsoleMode(console: Handle, mode: u32) -> i32;
        fn ReadConsoleW(
            console: Handle,
            buffer: *mut u16,
            to_read: u32,
            read: *mut u32,
            control: *mut c_void,
        ) -> i32;
    }

    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
    const ENABLE_LINE_INPUT: u32 = 0x0002;
    const ENABLE_ECHO_INPUT: u32 = 0x0004;
    const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
    const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    /// The console in raw mode, and the modes to put back.
    pub struct Raw {
        input: Handle,
        output: Handle,
        in_mode: u32,
        out_mode: u32,
        /// A high surrogate read ahead of its low half.
        pending: Option<u16>,
    }

    impl Raw {
        /// Raw, virtual-terminal input (no line buffering, no echo, Ctrl-C
        /// as a character) and virtual-terminal output, or `None` when
        /// either handle is not a console that allows them; nothing is
        /// left changed then.
        pub fn enter() -> Option<Raw> {
            // SAFETY: GetStdHandle takes a constant and returns a handle
            // the process owns, or null or INVALID_HANDLE_VALUE, both of
            // which make GetConsoleMode below fail.
            let (input, output) = unsafe {
                (
                    GetStdHandle(STD_INPUT_HANDLE),
                    GetStdHandle(STD_OUTPUT_HANDLE),
                )
            };
            let (mut in_mode, mut out_mode) = (0u32, 0u32);
            // SAFETY: both handles came from GetStdHandle, and each mode is
            // a valid, writable u32 for the call to fill.
            let consoles = unsafe {
                GetConsoleMode(input, &mut in_mode) != 0
                    && GetConsoleMode(output, &mut out_mode) != 0
            };
            if !consoles {
                return None;
            }
            let raw_in = (in_mode
                & !(ENABLE_PROCESSED_INPUT | ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT;
            let vt_out = out_mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            // SAFETY: console handles and plain mode flags; a failure is
            // reported by the return value, and the input mode is put back
            // when the output mode cannot be set.
            unsafe {
                if SetConsoleMode(input, raw_in) == 0 {
                    return None;
                }
                if SetConsoleMode(output, vt_out) == 0 {
                    SetConsoleMode(input, in_mode);
                    return None;
                }
            }
            Some(Raw {
                input,
                output,
                in_mode,
                out_mode,
                pending: None,
            })
        }

        /// One UTF-16 code unit from the console, `None` when the read
        /// fails or returns nothing.
        fn unit(&mut self) -> Option<u16> {
            if let Some(u) = self.pending.take() {
                return Some(u);
            }
            let mut unit = 0u16;
            let mut read = 0u32;
            // SAFETY: `unit` holds the one code unit asked for, `read` is a
            // valid u32, and the control argument may be null.
            let ok =
                unsafe { ReadConsoleW(self.input, &mut unit, 1, &mut read, std::ptr::null_mut()) };
            (ok != 0 && read == 1).then_some(unit)
        }

        /// The next character typed, a surrogate pair joined into one.
        pub fn read_char(&mut self) -> Option<char> {
            let first = self.unit()?;
            if (0xD800..0xDC00).contains(&first) {
                let second = self.unit()?;
                if (0xDC00..0xE000).contains(&second) {
                    return char::decode_utf16([first, second]).next()?.ok();
                }
                self.pending = Some(second);
                return Some(char::REPLACEMENT_CHARACTER);
            }
            Some(char::from_u32(u32::from(first)).unwrap_or(char::REPLACEMENT_CHARACTER))
        }
    }

    impl Drop for Raw {
        fn drop(&mut self) {
            // SAFETY: the handles and modes are the ones read in `enter`,
            // put back as they were.
            unsafe {
                SetConsoleMode(self.input, self.in_mode);
                SetConsoleMode(self.output, self.out_mode);
            }
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::io::Read;

    /// `struct termios`, opaque: larger and more aligned than any C
    /// library's, since only the library's own calls read or write it.
    #[repr(C, align(8))]
    struct Termios([u8; 256]);

    unsafe extern "C" {
        fn tcgetattr(fd: i32, t: *mut Termios) -> i32;
        fn tcsetattr(fd: i32, when: i32, t: *const Termios) -> i32;
        fn cfmakeraw(t: *mut Termios);
    }

    /// `TCSANOW`, which is 0 on every Unix.
    const TCSANOW: i32 = 0;

    /// Standard input in raw mode, and the settings to put back.
    pub struct Raw {
        saved: Termios,
    }

    impl Raw {
        /// Raw mode on standard input, or `None` when it is not a terminal;
        /// nothing is left changed then.
        pub fn enter() -> Option<Raw> {
            let mut saved = Termios([0; 256]);
            // SAFETY: `saved` is writable and larger than any `termios`.
            if unsafe { tcgetattr(0, &mut saved) } != 0 {
                return None;
            }
            let mut raw = Termios(saved.0);
            // SAFETY: `raw` holds the settings tcgetattr filled in, which
            // cfmakeraw edits in place and tcsetattr reads.
            let ok = unsafe {
                cfmakeraw(&mut raw);
                tcsetattr(0, TCSANOW, &raw) == 0
            };
            ok.then_some(Raw { saved })
        }

        fn byte(&mut self) -> Option<u8> {
            let mut b = [0u8; 1];
            match std::io::stdin().lock().read(&mut b) {
                Ok(1) => Some(b[0]),
                _ => None,
            }
        }

        /// The next character typed, decoded from UTF-8; an invalid
        /// sequence is U+FFFD.
        pub fn read_char(&mut self) -> Option<char> {
            let first = self.byte()?;
            let len = match first {
                0x00..=0x7F => return Some(char::from(first)),
                0xC0..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF7 => 4,
                _ => return Some(char::REPLACEMENT_CHARACTER),
            };
            let mut bytes = vec![first];
            for _ in 1..len {
                bytes.push(self.byte()?);
            }
            Some(
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|s| s.chars().next())
                    .unwrap_or(char::REPLACEMENT_CHARACTER),
            )
        }
    }

    impl Drop for Raw {
        fn drop(&mut self) {
            // SAFETY: `saved` is what tcgetattr read in `enter`, put back.
            unsafe {
                tcsetattr(0, TCSANOW, &self.saved);
            }
        }
    }
}

/// Neither Windows nor Unix: no raw mode, so the REPL reads plain lines.
#[cfg(not(any(windows, unix)))]
struct Raw;

#[cfg(not(any(windows, unix)))]
impl Raw {
    fn enter() -> Option<Raw> {
        None
    }
    fn read_char(&mut self) -> Option<char> {
        None
    }
}
