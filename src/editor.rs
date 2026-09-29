//! The terminal's line editor as a pure state machine (cycle 13).
//!
//! [`Decoder`] turns the characters a terminal sends into [`Key`]s, escape
//! sequences included, and [`Editor`] turns keys into a buffer, a cursor
//! and a place in the history. Neither reads or writes anything: the thin
//! raw-mode shell in the binary (`src/term.rs`) reads the characters,
//! feeds them through here and draws what [`Editor::buffer`] and
//! [`Editor::cursor`] say, so every key's effect is reachable from a unit
//! test with no terminal at all.

/// One key, as the editor understands it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// A character to insert.
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Tab,
    /// Ctrl-C: clears the line.
    Interrupt,
    /// Ctrl-D: the end of input on an empty line, Delete otherwise.
    EndOfInput,
    /// A key or a sequence the editor does nothing with.
    Ignored,
}

/// Turns terminal input, one character at a time, into keys: plain
/// characters, the control characters, and the ANSI escape sequences both
/// a Unix terminal in raw mode and a Windows console with virtual-terminal
/// input send (`ESC [ A` for Up, `ESC [ 3 ~` for Delete, `ESC O H` for
/// Home, with any modifier parameters ignored).
#[derive(Default)]
pub struct Decoder {
    /// The escape sequence read so far, without its `ESC`; empty when none
    /// is open, and `esc` says whether one is.
    seq: String,
    esc: bool,
}

/// The longest escape sequence the decoder waits for before giving up on
/// it, so that a stray `ESC [` cannot swallow every key after it.
const MAX_SEQUENCE: usize = 16;

impl Decoder {
    /// The key `c` completes, if it completes one.
    pub fn feed(&mut self, c: char) -> Option<Key> {
        if self.esc {
            return self.sequence(c);
        }
        Some(match c {
            '\x1b' => {
                self.esc = true;
                self.seq.clear();
                return None;
            }
            '\r' | '\n' => Key::Enter,
            '\x7f' | '\x08' => Key::Backspace,
            '\t' => Key::Tab,
            '\x03' => Key::Interrupt,
            '\x04' => Key::EndOfInput,
            '\x01' => Key::Home,
            '\x05' => Key::End,
            '\x02' => Key::Left,
            '\x06' => Key::Right,
            '\x10' => Key::Up,
            '\x0e' => Key::Down,
            c if c.is_control() => Key::Ignored,
            c => Key::Char(c),
        })
    }

    /// The next character of an open escape sequence.
    fn sequence(&mut self, c: char) -> Option<Key> {
        if self.seq.is_empty() {
            if c == '[' || c == 'O' {
                self.seq.push(c);
                return None;
            }
            // `ESC` and anything else: not a sequence this editor knows.
            self.esc = false;
            return Some(Key::Ignored);
        }
        self.seq.push(c);
        // A CSI or SS3 sequence ends at its first final byte.
        if !('@'..='~').contains(&c) || (self.seq.len() == 2 && c == '[') {
            if self.seq.len() > MAX_SEQUENCE {
                self.esc = false;
                return Some(Key::Ignored);
            }
            return None;
        }
        self.esc = false;
        let params = &self.seq[1..self.seq.len() - 1];
        let first = params.split(';').next().unwrap_or("");
        Some(match c {
            'A' => Key::Up,
            'B' => Key::Down,
            'C' => Key::Right,
            'D' => Key::Left,
            'H' => Key::Home,
            'F' => Key::End,
            '~' => match first {
                "1" | "7" => Key::Home,
                "4" | "8" => Key::End,
                "3" => Key::Delete,
                _ => Key::Ignored,
            },
            _ => Key::Ignored,
        })
    }
}

/// What a key did, for the shell to act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// The line may have changed: redraw it.
    Redraw,
    /// Enter: the line is submitted. The editor is ready for the next one.
    Submit(String),
    /// Ctrl-C: the line was cleared, and anything the REPL was collecting
    /// for an unfinished block should be dropped too.
    Cleared,
    /// Ctrl-D on an empty line: the end of input.
    Eof,
    /// Tab with several completions and nothing more they share: the shell
    /// lists them, then redraws the line.
    List(Vec<String>),
}

/// A line being edited, and the history it can recall.
pub struct Editor {
    buf: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    /// Which history entry is shown: an index into `history`, or
    /// `history.len()` for the line being typed.
    place: usize,
    /// The line being typed, kept while Up and Down show older entries.
    draft: Vec<char>,
}

impl Editor {
    /// An empty line, with `history` (oldest first) to recall.
    pub fn new(history: Vec<String>) -> Editor {
        let place = history.len();
        Editor {
            buf: Vec::new(),
            cursor: 0,
            history,
            place,
            draft: Vec::new(),
        }
    }

    /// The line as it stands.
    pub fn buffer(&self) -> String {
        self.buf.iter().collect()
    }

    /// The cursor, in characters from the start of the line.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The characters after the cursor: how far the shell moves back after
    /// drawing the whole line.
    pub fn after_cursor(&self) -> usize {
        self.buf.len() - self.cursor
    }

    /// The history entry shown, or `history().len()` for the line typed.
    pub fn place(&self) -> usize {
        self.place
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Adds `entry` to the history, as [`crate::history::remember`] does;
    /// true when it was added. The next line starts past it.
    pub fn remember(&mut self, entry: &str) -> bool {
        let added = crate::history::remember(&mut self.history, entry);
        self.place = self.history.len();
        added
    }

    /// Starts a fresh, empty line.
    fn reset(&mut self) {
        self.buf.clear();
        self.cursor = 0;
        self.draft.clear();
        self.place = self.history.len();
    }

    /// Shows history entry `place`, or the draft past the newest.
    fn show(&mut self, place: usize) {
        if self.place == self.history.len() {
            self.draft = std::mem::take(&mut self.buf);
        }
        self.place = place;
        self.buf = match self.history.get(place) {
            Some(e) => e.chars().collect(),
            None => std::mem::take(&mut self.draft),
        };
        self.cursor = self.buf.len();
    }

    /// Applies one key. `complete` lists the names starting with a prefix,
    /// sorted, which is `env::completions` in the shell.
    pub fn key(&mut self, key: Key, complete: &mut dyn FnMut(&str) -> Vec<String>) -> Action {
        match key {
            Key::Char(c) => {
                self.buf.insert(self.cursor, c);
                self.cursor += 1;
            }
            Key::Enter => {
                let line = self.buffer();
                self.reset();
                return Action::Submit(line);
            }
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.buf.remove(self.cursor);
            }
            Key::EndOfInput if self.buf.is_empty() => return Action::Eof,
            Key::Delete | Key::EndOfInput if self.cursor < self.buf.len() => {
                self.buf.remove(self.cursor);
            }
            Key::Left if self.cursor > 0 => self.cursor -= 1,
            Key::Right if self.cursor < self.buf.len() => self.cursor += 1,
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.buf.len(),
            Key::Up if self.place > 0 => self.show(self.place - 1),
            Key::Down if self.place < self.history.len() => self.show(self.place + 1),
            Key::Interrupt => {
                self.reset();
                return Action::Cleared;
            }
            Key::Tab => return self.complete(complete),
            _ => {}
        }
        Action::Redraw
    }

    /// Tab: completes the name that ends at the cursor. One candidate is
    /// inserted whole; several have their common part inserted, and when
    /// that adds nothing they are listed instead. With no name before the
    /// cursor, or no candidate, nothing happens.
    fn complete(&mut self, complete: &mut dyn FnMut(&str) -> Vec<String>) -> Action {
        let start = self.buf[..self.cursor]
            .iter()
            .rposition(|c| !(c.is_ascii_alphanumeric() || *c == '_'))
            .map_or(0, |k| k + 1);
        if start == self.cursor || self.buf[start].is_ascii_digit() {
            return Action::Redraw;
        }
        let prefix: String = self.buf[start..self.cursor].iter().collect();
        let names = complete(&prefix);
        let Some(first) = names.first() else {
            return Action::Redraw;
        };
        let shared = names.iter().skip(1).fold(first.as_str(), |acc, n| {
            let k = acc
                .char_indices()
                .zip(n.chars())
                .find(|((_, a), b)| a != b)
                .map_or(acc.len().min(n.len()), |((k, _), _)| k);
            &acc[..k]
        });
        if shared.len() <= prefix.len() {
            return if names.len() > 1 {
                Action::List(names)
            } else {
                Action::Redraw
            };
        }
        for c in shared[prefix.len()..].chars() {
            self.buf.insert(self.cursor, c);
            self.cursor += 1;
        }
        Action::Redraw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str) -> Vec<Key> {
        let mut d = Decoder::default();
        text.chars().filter_map(|c| d.feed(c)).collect()
    }

    fn none(_: &str) -> Vec<String> {
        Vec::new()
    }

    /// Applies each key and returns the last action.
    fn press(e: &mut Editor, ks: &[Key]) -> Action {
        let mut last = Action::Redraw;
        for k in ks {
            last = e.key(k.clone(), &mut none);
        }
        last
    }

    fn typed(e: &mut Editor, text: &str) {
        for c in text.chars() {
            e.key(Key::Char(c), &mut none);
        }
    }

    #[test]
    fn the_decoder_reads_characters_controls_and_sequences() {
        assert_eq!(
            keys("a\u{e9}\r\x7f\t\x03\x04\x01\x05"),
            [
                Key::Char('a'),
                Key::Char('\u{e9}'),
                Key::Enter,
                Key::Backspace,
                Key::Tab,
                Key::Interrupt,
                Key::EndOfInput,
                Key::Home,
                Key::End
            ]
        );
        assert_eq!(
            keys("\x1b[A\x1b[B\x1b[C\x1b[D\x1b[H\x1b[F\x1bOH\x1bOF"),
            [
                Key::Up,
                Key::Down,
                Key::Right,
                Key::Left,
                Key::Home,
                Key::End,
                Key::Home,
                Key::End
            ]
        );
        assert_eq!(
            keys("\x1b[3~\x1b[1~\x1b[4~\x1b[7~\x1b[8~\x1b[1;5C\x1b[5~"),
            [
                Key::Delete,
                Key::Home,
                Key::End,
                Key::Home,
                Key::End,
                Key::Right,
                Key::Ignored
            ]
        );
        // An escape that starts no sequence is dropped with the key after
        // it; a runaway sequence is given up on, and typing goes on.
        assert_eq!(keys("\x1bxa"), [Key::Ignored, Key::Char('a')]);
        let long = format!("\x1b[{}b", "1".repeat(40));
        let got = keys(&long);
        assert_eq!(got.first(), Some(&Key::Ignored));
        assert_eq!(got.last(), Some(&Key::Char('b')));
    }

    #[test]
    fn keys_move_the_cursor_and_edit_the_buffer() {
        let mut e = Editor::new(Vec::new());
        typed(&mut e, "disp(1)");
        assert_eq!((e.buffer().as_str(), e.cursor()), ("disp(1)", 7));
        press(&mut e, &[Key::Left, Key::Left, Key::Backspace]);
        assert_eq!((e.buffer().as_str(), e.cursor()), ("disp1)", 4));
        typed(&mut e, "(2");
        assert_eq!((e.buffer().as_str(), e.cursor()), ("disp(21)", 6));
        press(&mut e, &[Key::Delete]);
        assert_eq!(e.buffer(), "disp(2)");
        press(&mut e, &[Key::Home]);
        assert_eq!(e.cursor(), 0);
        // Backspace and Left at the start, Right and Delete at the end, do
        // nothing.
        press(&mut e, &[Key::Backspace, Key::Left]);
        assert_eq!((e.buffer().as_str(), e.cursor()), ("disp(2)", 0));
        press(&mut e, &[Key::End, Key::Right, Key::Delete]);
        assert_eq!(
            (e.buffer().as_str(), e.cursor(), e.after_cursor()),
            ("disp(2)", 7, 0)
        );
        press(&mut e, &[Key::Home, Key::Right]);
        assert_eq!(e.after_cursor(), 6);
        assert_eq!(
            press(&mut e, &[Key::Enter]),
            Action::Submit("disp(2)".into())
        );
        assert_eq!((e.buffer().as_str(), e.cursor()), ("", 0));
    }

    #[test]
    fn ctrl_c_clears_and_ctrl_d_ends_only_an_empty_line() {
        let mut e = Editor::new(Vec::new());
        typed(&mut e, "abc");
        assert_eq!(press(&mut e, &[Key::Interrupt]), Action::Cleared);
        assert_eq!(e.buffer(), "");
        typed(&mut e, "ab");
        press(&mut e, &[Key::Home]);
        assert_eq!(press(&mut e, &[Key::EndOfInput]), Action::Redraw);
        assert_eq!(e.buffer(), "b");
        press(&mut e, &[Key::EndOfInput]);
        assert_eq!(press(&mut e, &[Key::EndOfInput]), Action::Eof);
    }

    #[test]
    fn up_and_down_walk_the_history_and_keep_the_draft() {
        let mut e = Editor::new(vec!["a = 1".into(), "b = 2".into()]);
        assert_eq!(e.place(), 2);
        typed(&mut e, "dr");
        press(&mut e, &[Key::Up]);
        assert_eq!(
            (e.buffer().as_str(), e.place(), e.cursor()),
            ("b = 2", 1, 5)
        );
        press(&mut e, &[Key::Up, Key::Up]);
        assert_eq!((e.buffer().as_str(), e.place()), ("a = 1", 0));
        press(&mut e, &[Key::Down]);
        assert_eq!(e.buffer(), "b = 2");
        press(&mut e, &[Key::Down, Key::Down]);
        assert_eq!((e.buffer().as_str(), e.place()), ("dr", 2));
        // A recalled entry can be edited and submitted; remembering it puts
        // it last, and the next line starts past it.
        press(&mut e, &[Key::Up, Key::Up, Key::Backspace]);
        typed(&mut e, "5");
        assert_eq!(press(&mut e, &[Key::Enter]), Action::Submit("a = 5".into()));
        assert!(e.remember("a = 5"));
        assert!(!e.remember("a = 5"));
        assert_eq!(e.history(), ["a = 1", "b = 2", "a = 5"]);
        assert_eq!(e.place(), 3);
        press(&mut e, &[Key::Up]);
        assert_eq!(e.buffer(), "a = 5");
    }

    #[test]
    fn tab_completes_through_the_given_function() {
        let names = ["disp", "display_all", "dispatch", "sum"];
        let mut complete = |p: &str| -> Vec<String> {
            names
                .iter()
                .filter(|n| n.starts_with(p))
                .map(|n| n.to_string())
                .collect()
        };
        let mut e = Editor::new(Vec::new());
        typed(&mut e, "x = su");
        assert_eq!(e.key(Key::Tab, &mut complete), Action::Redraw);
        assert_eq!((e.buffer().as_str(), e.cursor()), ("x = sum", 7));
        // Several: the common part first, then the list.
        let mut e = Editor::new(Vec::new());
        typed(&mut e, "di(1)");
        press(&mut e, &[Key::Left, Key::Left, Key::Left]);
        e.key(Key::Tab, &mut complete);
        assert_eq!((e.buffer().as_str(), e.cursor()), ("disp(1)", 4));
        assert_eq!(
            e.key(Key::Tab, &mut complete),
            Action::List(vec!["disp".into(), "display_all".into(), "dispatch".into()])
        );
        assert_eq!(e.buffer(), "disp(1)");
        // Nothing before the cursor, a number, or no candidate: no change.
        let mut e = Editor::new(Vec::new());
        e.key(Key::Tab, &mut complete);
        typed(&mut e, "12 zz");
        e.key(Key::Tab, &mut complete);
        press(&mut e, &[Key::Left, Key::Left, Key::Left]);
        e.key(Key::Tab, &mut complete);
        assert_eq!(e.buffer(), "12 zz");
    }
}
