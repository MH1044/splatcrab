//! The command history file (cycle 13), shared by every front end that keeps
//! a history, so that a terminal session and a browser session can read
//! and extend one history rather than inventing two.
//!
//! The format is plain UTF-8 text, one entry per line, each line ending in
//! LF, oldest first. An entry is the text submitted, with a backslash
//! written `\\`, a line feed `\n` and a carriage return `\r`, so a
//! multi-line entry stays one line of the file; any other backslash pair is
//! read as the two characters it is. Empty lines are skipped. A front end
//! appends one line per entry with [`append`] and reads the file with
//! [`load`], which keeps the newest [`MAX_ENTRIES`] and rewrites the file
//! when it has grown past twice that, so appending never needs a lock and
//! the file never grows without bound.
//!
//! Two bounds in bytes hold since cycle 17. An entry is at most
//! [`MAX_ENTRY_BYTES`] (64 KiB) of UTF-8: [`remember`] refuses a longer
//! one, so no front end keeps or appends it, and [`load`] leaves out a
//! longer one another program wrote. And [`load`] reads at most the last
//! [`MAX_FILE_BYTES`] (4 MiB) of the file, judging its length from its
//! metadata and seeking there, never reading the rest: a file of no more
//! than that is read whole, a longer one from that many bytes before its
//! end, less the partial line the read starts in, and then rewritten with
//! the entries kept, so the next load reads it whole again. A line another
//! program wrote with a byte that is not UTF-8, or a backslash pair that is
//! not an escape, is written back longer, as U+FFFD or with its backslash
//! doubled, and a file so written back past the bound is cut once more by
//! the next load, after which its lines are this module's own. So a
//! history answer carries at most the entries of 4 MiB of the file,
//! whatever size the file has grown to.
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

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The most entries a history keeps.
pub const MAX_ENTRIES: usize = 1000;

/// The most bytes of UTF-8 an entry may hold and be kept, 64 KiB: see the
/// module comment.
pub const MAX_ENTRY_BYTES: usize = 65_536;

/// The most bytes of the history file [`load`] reads, its last 4 MiB: see
/// the module comment.
pub const MAX_FILE_BYTES: u64 = 4_194_304;

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

/// The lines of `text` that hold an entry, oldest first: every line but an
/// empty one, without its LF or CR LF.
fn entry_lines(text: &str) -> impl DoubleEndedIterator<Item = &str> + Clone {
    text.lines()
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .filter(|l| !l.is_empty())
}

/// The entries of `text`, the contents of a history file, oldest first.
pub fn parse(text: &str) -> Vec<String> {
    entry_lines(text).map(decode).collect()
}

/// The newest [`MAX_ENTRIES`] entries of the file at `path`, oldest first,
/// read from at most its last [`MAX_FILE_BYTES`] and leaving out an entry
/// of more than [`MAX_ENTRY_BYTES`]; none when it does not exist or cannot
/// be read. A file grown past twice [`MAX_ENTRIES`] entries or past
/// [`MAX_FILE_BYTES`] is rewritten with the entries kept, and a failure to
/// rewrite it is ignored: the history is a convenience, never a reason to
/// fail.
pub fn load(path: &Path) -> Vec<String> {
    load_within(path, MAX_FILE_BYTES, MAX_ENTRY_BYTES)
}

/// [`load`] with its two bounds as parameters, so a unit test reaches them
/// with small numbers: at most the last `max_bytes` of the file are read,
/// and an entry of more than `max_entry` bytes, judged after decoding, is
/// left out before the newest [`MAX_ENTRIES`] are kept, so a long line
/// never takes a place among them.
fn load_within(path: &Path, max_bytes: u64, max_entry: usize) -> Vec<String> {
    let Some((bytes, longer)) = read_tail(path, max_bytes) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let lines = entry_lines(&text);
    let total = lines.clone().count();
    // Newest first, so only the lines kept are decoded, then turned round.
    let mut entries: Vec<String> = lines
        .rev()
        .map(decode)
        .filter(|e| e.len() <= max_entry)
        .take(MAX_ENTRIES)
        .collect();
    entries.reverse();
    if longer || total > 2 * MAX_ENTRIES {
        let _ = save(path, &entries);
    }
    entries
}

/// The bytes of the file at `path` that [`load_within`] reads, and whether
/// the file was longer than `max_bytes`; `None` when it cannot be opened,
/// measured or read. The length is judged from the file's metadata, never
/// by reading it: a file of no more than `max_bytes` is read whole, and a
/// longer one from `max_bytes` before its end, less the partial line the
/// read starts in. The byte before that start is read too, so a read that
/// starts exactly at the first byte of a line keeps it. Either read stops
/// after the bytes it asked for, so a file that grows meanwhile, or a
/// device that reports no length, is read no further; a file cut short
/// meanwhile is answered from what was read but not called longer.
fn read_tail(path: &Path, max_bytes: u64) -> Option<(Vec<u8>, bool)> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut bytes = Vec::new();
    if len <= max_bytes {
        file.take(max_bytes).read_to_end(&mut bytes).ok()?;
        return Some((bytes, false));
    }
    file.seek(SeekFrom::Start(len - max_bytes - 1)).ok()?;
    file.take(max_bytes + 1).read_to_end(&mut bytes).ok()?;
    // A file that another program cut short since it was measured is not
    // known to be long, and is not rewritten from what little was read.
    let longer = bytes.len() as u64 == max_bytes + 1;
    // An LF never occurs inside a UTF-8 character, so the first one here,
    // the byte before the tail included, ends the partial line, and one
    // that starts inside a character loses only that line.
    let start = bytes
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |k| k + 1);
    bytes.drain(..start);
    Some((bytes, longer))
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
/// of more than [`MAX_ENTRY_BYTES`] of UTF-8, one that is only whitespace,
/// or the same as the newest one, is not added, and the oldest entry goes
/// once there are more than [`MAX_ENTRIES`]. True when it was added, and
/// so should be appended to the file too.
pub fn remember(entries: &mut Vec<String>, entry: &str) -> bool {
    remember_within(entries, entry, MAX_ENTRY_BYTES)
}

/// [`remember`] with the entry bound as a parameter, judged before any
/// other rule, so an entry past it is never even compared.
fn remember_within(entries: &mut Vec<String>, entry: &str, max_entry: usize) -> bool {
    if entry.len() > max_entry
        || entry.trim().is_empty()
        || entries.last().is_some_and(|e| e == entry)
    {
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

    /// The file's bytes, or `None` when it is gone.
    fn bytes_of(path: &Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    /// Cycle 17: a file longer than the byte bound is read from its tail,
    /// the partial line the read starts in dropped, wherever in a line or
    /// a character it starts, and the newest entries of the tail kept.
    #[test]
    fn load_reads_at_most_the_tail_of_a_long_file() {
        let d = dir("tail");
        let path = d.0.join("history");
        let tail = |text: &[u8], bound: u64| {
            std::fs::write(&path, text).unwrap();
            load_within(&path, bound, MAX_ENTRY_BYTES)
        };
        // "aaaa\n" is bytes 0 to 4, "bbbb\n" 5 to 9, "cccc\n" 10 to 14.
        let abc = b"aaaa\nbbbb\ncccc\n";
        // A read starting inside a line drops that line.
        assert_eq!(tail(abc, 7), ["cccc"]);
        assert_eq!(tail(abc, 9), ["cccc"]);
        // One starting exactly at the first byte of a line keeps it: the
        // byte before is an LF.
        assert_eq!(tail(abc, 10), ["bbbb", "cccc"]);
        // One starting at an LF drops only the empty rest of its line.
        assert_eq!(tail(abc, 11), ["bbbb", "cccc"]);
        // One byte of the bound short of the whole file.
        assert_eq!(tail(abc, 14), ["bbbb", "cccc"]);
        // The whole file is within a bound of its length.
        assert_eq!(tail(abc, 15), ["aaaa", "bbbb", "cccc"]);
        // A tail with no LF at all is no entry.
        assert!(tail(b"aaaaaaaaaaaaaaaaaaaa", 5).is_empty());
        assert!(tail(b"aaaa\nbbbbbbbbbbbbbbbb", 5).is_empty());
        // Without a final LF the last line is still an entry.
        assert_eq!(tail(b"aaaa\nbbbb\ncc", 4), ["cc"]);
        assert_eq!(tail(b"aaaa\nbbbb\ncc", 7), ["bbbb", "cc"]);

        // "é" is two bytes, C3 A9: "x\n" is 0 to 1, "éé\n" 2 to 6 and
        // "yy\n" 7 to 9. A read starting at the LF, between two characters
        // or at the second byte of either loses only the partial line, and
        // nothing is decoded as a replacement character.
        let multi = "x\néé\nyy\n".as_bytes();
        for bound in [4, 5, 6, 7] {
            assert_eq!(tail(multi, bound), ["yy"], "bound {bound}");
        }
        assert_eq!(tail(multi, 8), ["éé", "yy"]);

        // CR LF lines: "a1\r\n" is 0 to 3, "b2\r\n" 4 to 7, "c3\r\n" 8 to
        // 11. Starting at the CR or the LF of a line drops only its end.
        let crlf = b"a1\r\nb2\r\nc3\r\n";
        assert_eq!(tail(crlf, 7), ["c3"]);
        assert_eq!(tail(crlf, 8), ["b2", "c3"]);
        assert_eq!(tail(crlf, 9), ["b2", "c3"]);
        assert_eq!(tail(crlf, 10), ["b2", "c3"]);
        assert_eq!(tail(crlf, 11), ["b2", "c3"]);
        assert_eq!(tail(crlf, 12), ["a1", "b2", "c3"]);

        // The newest MAX_ENTRIES of the lines read are kept, oldest first:
        // 3000 lines of six bytes, a tail of the last 1500 of them.
        let lines: String = (0..3000).map(|k| format!("e{k:04}\n")).collect();
        let got = tail(lines.as_bytes(), 6 * 1500);
        assert_eq!(got.len(), MAX_ENTRIES);
        assert_eq!(got[0], "e2000");
        assert_eq!(got[MAX_ENTRIES - 1], "e2999");
    }

    /// Cycle 17: a file longer than the byte bound is rewritten with the
    /// entries kept, and read whole next time; a file within the bound is
    /// read whole and left exactly as it is; a rewrite that fails is
    /// ignored, the entries answered all the same.
    #[test]
    fn a_long_file_is_compacted_to_what_was_kept() {
        let d = dir("compact");
        let path = d.0.join("history");
        // Within the bound, blank lines and CR LF and all: not rewritten.
        let within = b"a\r\n\r\nb\\\\c\nd\\ne\n";
        std::fs::write(&path, within).unwrap();
        let want = ["a", "b\\c", "d\ne"];
        assert_eq!(load_within(&path, within.len() as u64, 64), want);
        assert_eq!(bytes_of(&path).unwrap(), within);

        // One byte past the bound: read from the tail and rewritten with
        // what was kept, escapes and all, in the file's own format.
        let long = b"xxxx\na\\\\b\nc\\nd\n";
        std::fs::write(&path, long).unwrap();
        let kept = load_within(&path, long.len() as u64 - 1, 64);
        assert_eq!(kept, ["a\\b", "c\nd"]);
        assert_eq!(bytes_of(&path).unwrap(), b"a\\\\b\nc\\nd\n");
        // The next load reads the compacted file whole and leaves it be.
        assert_eq!(load_within(&path, long.len() as u64 - 1, 64), kept);
        assert_eq!(bytes_of(&path).unwrap(), b"a\\\\b\nc\\nd\n");

        // A tail with no whole line leaves an empty history and an empty
        // file.
        std::fs::write(&path, [b'z'; 100]).unwrap();
        assert!(load_within(&path, 10, 64).is_empty());
        assert_eq!(bytes_of(&path).unwrap(), b"");

        // A line past the entry bound in the tail is not written back.
        std::fs::write(&path, b"old\nkeep\nxxxxxxxxxx\nnew\n").unwrap();
        assert_eq!(load_within(&path, 20, 8), ["keep", "new"]);
        assert_eq!(bytes_of(&path).unwrap(), b"keep\nnew\n");

        // A rewrite that fails, the file read-only, is ignored: the
        // entries are the tail's all the same, and the file is left as it
        // was or compacted, never anything else. (Where the file system
        // lets its owner write a read-only file, the rewrite succeeds.)
        std::fs::write(&path, long).unwrap();
        let original = std::fs::metadata(&path).unwrap().permissions();
        let mut read_only = original.clone();
        read_only.set_readonly(true);
        std::fs::set_permissions(&path, read_only).unwrap();
        let got = load_within(&path, long.len() as u64 - 1, 64);
        let after = bytes_of(&path).unwrap();
        std::fs::set_permissions(&path, original).unwrap();
        assert_eq!(got, ["a\\b", "c\nd"]);
        assert!(after == long || after == b"a\\\\b\nc\\nd\n", "{after:?}");
        // A history path that is a folder is no history, and no rewrite.
        assert!(load_within(&d.0, 1, 64).is_empty());
    }

    /// Cycle 17: an entry of more bytes of UTF-8 than the entry bound is
    /// not remembered, before any other rule; one at the bound is.
    #[test]
    fn an_entry_past_the_bound_is_not_remembered() {
        let mut h = Vec::new();
        assert!(remember_within(&mut h, "abcd", 4));
        assert!(!remember_within(&mut h, "abcde", 4));
        // Bytes, not characters: "éé" is four bytes, "ééx" five.
        assert!(remember_within(&mut h, "éé", 4));
        assert!(!remember_within(&mut h, "ééx", 4));
        // Whitespace past the bound is refused, as it is within it; nothing
        // is added.
        assert!(!remember_within(&mut h, "      ", 4));
        assert!(!remember_within(&mut h, "abcdef", 4));
        assert_eq!(h, ["abcd", "éé"]);

        // With the bound itself: 65,536 bytes kept, then a repeat of the
        // newest; 65,537 not kept, and the history unchanged.
        let mut h = vec!["x = 1".to_string()];
        let at = "a".repeat(MAX_ENTRY_BYTES);
        let past = "a".repeat(MAX_ENTRY_BYTES + 1);
        assert!(!remember(&mut h, &past));
        assert_eq!(h, ["x = 1"]);
        assert!(remember(&mut h, &at));
        assert!(!remember(&mut h, &at));
        assert_eq!(h.len(), 2);
        assert_eq!(h[1].len(), MAX_ENTRY_BYTES);
    }

    /// Cycle 17: a line of the file whose entry is longer than the entry
    /// bound, as another program may write one, is left out, judged after
    /// decoding, and takes no place among the newest entries.
    #[test]
    fn a_long_line_in_the_file_is_left_out() {
        let d = dir("longline");
        let path = d.0.join("history");
        let text = b"a\nxxxxxxxxx\nb\n";
        std::fs::write(&path, text).unwrap();
        assert_eq!(load_within(&path, 1 << 20, 8), ["a", "b"]);
        // Within the byte bound the file is left as it is.
        assert_eq!(bytes_of(&path).unwrap(), text);
        // Judged after decoding: sixteen bytes of "\\" escapes are eight
        // backslashes, at the bound; one more is past it.
        std::fs::write(
            &path,
            format!("{}\n{}\n", "\\\\".repeat(8), "\\\\".repeat(9)),
        )
        .unwrap();
        assert_eq!(load_within(&path, 1 << 20, 8), ["\\".repeat(8)]);
        // A byte that is not UTF-8 decodes to U+FFFD, three bytes, and is
        // judged so: two such bytes are six bytes of entry, within the
        // bound; three are nine, past it, though the line is only three.
        std::fs::write(&path, b"\xff\xff\n\xff\xff\xff\nz\n").unwrap();
        assert_eq!(load_within(&path, 1 << 20, 8), ["\u{fffd}\u{fffd}", "z"]);
        // A long line takes no place among the newest MAX_ENTRIES.
        let mut many: String = (0..MAX_ENTRIES).map(|k| format!("{k}\n")).collect();
        many.push_str(&"y".repeat(20));
        many.push('\n');
        std::fs::write(&path, &many).unwrap();
        let got = load_within(&path, 1 << 20, 8);
        assert_eq!(got.len(), MAX_ENTRIES);
        assert_eq!(got[0], "0");

        // With the bounds themselves: `a`, a line of 70,000 characters
        // and `b`, as another program may leave them.
        let seeded = format!("a\n{}\nb\n", "y".repeat(70_000));
        std::fs::write(&path, &seeded).unwrap();
        assert_eq!(load(&path), ["a", "b"]);
        assert_eq!(bytes_of(&path).unwrap(), seeded.as_bytes());
    }
}
