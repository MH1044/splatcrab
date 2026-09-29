//! The command history file (cycle 13), shared by every front end that keeps
//! a history, so that a terminal session and a browser session can read
//! and extend one history rather than inventing two.
//!
//! The format is plain UTF-8 text, one entry per line, each line ending in
//! LF, oldest first. An entry is the text submitted, with a backslash
//! written `\\`, a line feed `\n` and a carriage return `\r`, so a
//! multi-line entry stays one line of the file; any other backslash pair is
//! read as the two characters it is. Empty lines are skipped. A front end
//! appends one line per entry with [`append`] and reads the file whole with
//! [`load`], which keeps the newest [`MAX_ENTRIES`] and rewrites the file
//! when it has grown past twice that, so appending never needs a lock and
//! the file never grows without bound.
//!
//! The file is `SPLATCRAB_HISTORY` when that variable is set, and otherwise
//! `.splatcrab_history` in the home folder (`HOME`, or `USERPROFILE` on
//! Windows). Two front ends read and write it: the terminal's line editor,
//! only when standard input and output are both terminals, so a script and
//! a piped REPL never touch it; and since cycle U2 the protocol's `history`
//! and `history_add`, which the browser desktop's history pane asks for,
//! under `--protocol`, `--ui` and `--http-stdio`. The golden harness points
//! `SPLATCRAB_HISTORY` at a file of each case's own, so no case touches the
//! user's history.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The most entries a history keeps.
pub const MAX_ENTRIES: usize = 1000;

/// Where the history lives: see the module comment. `None` when neither
/// the override nor a home folder is known.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPLATCRAB_HISTORY").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(|home| PathBuf::from(home).join(".splatcrab_history"))
}

/// One entry as a line of the file, without its LF.
pub fn encode(entry: &str) -> String {
    let mut line = String::with_capacity(entry.len());
    for c in entry.chars() {
        match c {
            '\\' => line.push_str("\\\\"),
            '\n' => line.push_str("\\n"),
            '\r' => line.push_str("\\r"),
            c => line.push(c),
        }
    }
    line
}

/// One line of the file, without its LF, as the entry it holds.
pub fn decode(line: &str) -> String {
    let mut entry = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            entry.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => entry.push('\\'),
            Some('n') => entry.push('\n'),
            Some('r') => entry.push('\r'),
            Some(other) => {
                entry.push('\\');
                entry.push(other);
            }
            None => entry.push('\\'),
        }
    }
    entry
}

/// The entries of `text`, the contents of a history file, oldest first.
pub fn parse(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .filter(|l| !l.is_empty())
        .map(decode)
        .collect()
}

/// The newest [`MAX_ENTRIES`] entries of the file at `path`, oldest first;
/// none when it does not exist or cannot be read. A file grown past twice
/// the limit is rewritten with the entries kept, and a failure to rewrite
/// it is ignored: the history is a convenience, never a reason to fail.
pub fn load(path: &Path) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let mut entries = parse(&String::from_utf8_lossy(&bytes));
    let total = entries.len();
    if total > MAX_ENTRIES {
        entries.drain(..total - MAX_ENTRIES);
    }
    if total > 2 * MAX_ENTRIES {
        let _ = save(path, &entries);
    }
    entries
}

/// Writes `entries` as the whole file at `path`.
pub fn save(path: &Path, entries: &[String]) -> io::Result<()> {
    let mut text = String::new();
    for e in entries {
        text.push_str(&encode(e));
        text.push('\n');
    }
    std::fs::write(path, text)
}

/// Adds `entry` at the end of the file at `path`, creating it if need be.
pub fn append(path: &Path, entry: &str) -> io::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(format!("{}\n", encode(entry)).as_bytes())
}

/// Adds `entry` to an in-memory history as a front end should: an entry
/// that is only whitespace, or the same as the newest one, is not added,
/// and the oldest entry goes once there are more than [`MAX_ENTRIES`].
/// True when it was added, and so should be appended to the file too.
pub fn remember(entries: &mut Vec<String>, entry: &str) -> bool {
    if entry.trim().is_empty() || entries.last().is_some_and(|e| e == entry) {
        return false;
    }
    entries.push(entry.to_string());
    if entries.len() > MAX_ENTRIES {
        entries.remove(0);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir(name: &str) -> Dir {
        let d =
            std::env::temp_dir().join(format!("splatcrab-hist-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    #[test]
    fn an_entry_round_trips_through_its_line() {
        for e in [
            "x = 1",
            "a\\b",
            "for k = 1:3\n  disp(k)\nend",
            "tab\there",
            "cr\r",
            "\\n",
        ] {
            let line = encode(e);
            assert!(!line.contains('\n') && !line.contains('\r'), "{line:?}");
            assert_eq!(decode(&line), e);
        }
        assert_eq!(encode("a\\b\nc"), "a\\\\b\\nc");
        // A pair that is not an escape is its two characters; so is a
        // backslash at the end of a line.
        assert_eq!(decode("a\\qb\\"), "a\\qb\\");
    }

    #[test]
    fn the_file_round_trips_and_appends() {
        let d = dir("round");
        let path = d.0.join("history");
        assert!(load(&path).is_empty(), "a missing file is an empty history");
        let entries = vec!["x = 1".to_string(), "if x\n  disp(x)\nend".to_string()];
        save(&path, &entries).unwrap();
        assert_eq!(load(&path), entries);
        append(&path, "y = 'a\\b'").unwrap();
        let mut want = entries.clone();
        want.push("y = 'a\\b'".to_string());
        assert_eq!(load(&path), want);
        // Blank lines and CRLF endings, as another writer might leave them.
        std::fs::write(&path, "a\r\n\r\nb\n").unwrap();
        assert_eq!(load(&path), ["a", "b"]);
    }

    #[test]
    fn loading_keeps_the_newest_and_compacts_a_long_file() {
        let d = dir("long");
        let path = d.0.join("history");
        let many: Vec<String> = (0..2 * MAX_ENTRIES + 5)
            .map(|k| format!("x = {k}"))
            .collect();
        save(&path, &many).unwrap();
        let got = load(&path);
        assert_eq!(got.len(), MAX_ENTRIES);
        assert_eq!(got.last().unwrap(), &format!("x = {}", 2 * MAX_ENTRIES + 4));
        // The file itself was rewritten with the entries kept.
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), MAX_ENTRIES);
    }

    #[test]
    fn remember_skips_blanks_and_repeats_and_caps() {
        let mut h = Vec::new();
        assert!(remember(&mut h, "a"));
        assert!(!remember(&mut h, "a"));
        assert!(!remember(&mut h, "   "));
        assert!(remember(&mut h, "b"));
        assert!(remember(&mut h, "a"));
        assert_eq!(h, ["a", "b", "a"]);
        for k in 0..MAX_ENTRIES {
            remember(&mut h, &k.to_string());
        }
        assert_eq!(h.len(), MAX_ENTRIES);
        assert_eq!(h[0], "0");
    }
}
