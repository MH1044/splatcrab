//! The file browser's listing (cycle U2): one folder under the file root,
//! as the protocol's `files` operation answers it.
//!
//! The file root is fixed once when a client mode starts, `--protocol`,
//! `--ui` or `--http-stdio`: the process's working directory, canonicalised
//! ([`session_root`]). It is held on [`crate::interp::Interp::file_root`]
//! and never changes, whatever `cd` does to `Interp::cwd`.
//!
//! A requested path is judged by the confinement rule of
//! `docs/modules/U2-ui-desktop.md`, in this order, before anything is read:
//!
//! 1. it is split at `/`; a path that starts with `/`, or holds a `\`, a `:`
//!    or a NUL anywhere, is malformed, so no absolute path, drive, UNC name,
//!    Windows separator or alternate data stream is ever joined to the root;
//! 2. empty components and `.` are dropped and `..` removes the component
//!    before it, and a `..` with nothing before it is outside the root: all
//!    of this on the text alone ([`normalise`]), so no request can make the
//!    server touch a path outside the root, even to find it missing;
//! 3. the rest is joined to the root and canonicalised, which resolves every
//!    link and junction; a path that cannot be is not a folder, a canonical
//!    path that is not the root or inside it, compared component by
//!    component, is outside the root, judged before its kind so a refusal
//!    says nothing about what lies outside, and one inside that is not a
//!    folder is not a folder.
//!
//! An entry that is a link is listed by what it points to when that is
//! inside the root, and otherwise, dangling included, as neither a folder
//! nor a file of known size, so a listing says nothing about what is
//! outside. Entries come folders first, then everything else, each group
//! in byte order of name, at most a bound of them, the first in that order;
//! an entry whose name is not valid Unicode is left out.
//!
//! Since cycle U3 the editor's three operations judge their paths by the
//! same rule, steps 1 and 2 unchanged, as `docs/modules/U3-ui-editor.md`
//! fixes them:
//!
//! - [`read_file`] and [`run_file`] take step 3 for a file: a path that
//!   cannot be canonicalised, or whose canonical form is not a regular file
//!   (the root, a folder, a device, a pipe), is not a file, and one outside
//!   the root is outside it, judged first. A read judges the file's length
//!   before a byte is read and reads no further than its bound plus one, so
//!   a file that grew is refused too, and its bytes must be UTF-8. A run
//!   needs a name ending in `.m`.
//! - [`write_file`] judges the name before the disk: a path with no
//!   component left, or whose last component is a Windows device name
//!   (`CON`, `PRN`, `AUX`, `NUL`, `COM0` to `COM9`, `LPT0` to `LPT9`, in any
//!   case, alone or before a `.`) or ends in a `.` or a space, is not a file,
//!   on every platform, so a name always names the file it says. Then the
//!   folder, every component but the last, canonicalised: missing or not a
//!   folder, or outside the root. Then whatever is at the name, a link
//!   included, canonicalised: inside the root, not a folder, and a link
//!   that resolves nowhere is not a file, so a write never follows a link
//!   out of the root or makes a file at a dangling link's target. Last the
//!   text's length. No folder is ever created.
//!
//! [`relative`] names an error frame's file relative to the root, for the
//! stack the protocol answers.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::DirEntry;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use crate::error::{self, MError};

/// The most entries one listing holds: what the protocol's `files` passes
/// to [`list`] as its bound.
pub const MAX_ENTRIES: usize = 10_000;

/// The most bytes `read_file` reads and `write_file` writes, 4 MiB: what
/// the protocol passes to [`read_file`] and [`write_file`] as their bound.
pub const MAX_TEXT: u64 = 4 * 1024 * 1024;

/// One entry of a listing. `size` is a file's length in bytes, and `None`
/// for a folder and for a link that points outside the root or nowhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub size: Option<u64>,
}

/// One folder's listing, as `files` answers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    /// The last component of the root's path, or `/` at the top of a file
    /// system.
    pub root: String,
    /// The folder listed, relative to the root and normalised: `""` is the
    /// root itself.
    pub path: String,
    pub entries: Vec<Entry>,
    /// True when entries past the bound were left out.
    pub truncated: bool,
}

/// The file root a client mode fixes as it starts: the process's working
/// directory, canonicalised, or as it stands when it cannot be. An
/// uncanonical root refuses every listing as outside itself, which is the
/// safe way for that to fail.
pub fn session_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    std::fs::canonicalize(&cwd).unwrap_or(cwd)
}

/// The root's name as `files` reports it.
pub fn root_name(root: &Path) -> String {
    root.file_name()
        .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned())
}

/// Steps 1 and 2 of the confinement rule, on the text alone: the components
/// of `path` once `.`, empty components and `..` are resolved.
pub fn normalise(path: &str) -> Result<Vec<&str>, MError> {
    if path.starts_with('/') || path.contains(['\\', ':', '\0']) {
        return Err(error::files_path_malformed());
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(error::files_outside_root(path));
                }
            }
            name => parts.push(name),
        }
    }
    Ok(parts)
}

/// Where an entry's size is read from once it is known to be listed, so a
/// folder far larger than the bound costs a look at each name, and one
/// resolution of each entry that is a link (to learn whether its target is
/// inside the root and a folder), and nothing more.
enum SizeFrom {
    /// A folder, or a link that points outside the root or nowhere.
    Nothing,
    /// The entry itself, which is not a link: boxed, since on Windows it
    /// carries the whole of the system's find data.
    Entry(Box<DirEntry>),
    /// A link's target inside the root.
    Target(PathBuf),
}

/// An entry on its way into the listing, ordered as the listing orders
/// them: folders first, then by name in byte order.
struct Candidate {
    not_dir: bool,
    name: String,
    size: SizeFrom,
}

impl Candidate {
    fn key(&self) -> (bool, &str) {
        (self.not_dir, &self.name)
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

/// What a directory entry is, for the listing: a folder or not, and where
/// its size comes from. A link counts by its target only when the target
/// is inside `root`.
fn classify(entry: DirEntry, root: &Path) -> (bool, SizeFrom) {
    let Ok(kind) = entry.file_type() else {
        return (false, SizeFrom::Nothing);
    };
    if kind.is_symlink() {
        return match std::fs::canonicalize(entry.path()) {
            Ok(target) if target.starts_with(root) => {
                if target.is_dir() {
                    (true, SizeFrom::Nothing)
                } else {
                    (false, SizeFrom::Target(target))
                }
            }
            _ => (false, SizeFrom::Nothing),
        };
    }
    if kind.is_dir() {
        (true, SizeFrom::Nothing)
    } else {
        (false, SizeFrom::Entry(Box::new(entry)))
    }
}

/// `parts`, the components [`normalise`] left, joined to `root`.
///
/// One push of the whole relative path, never one a component: on Windows
/// a push onto a verbatim `\\?\` root, which a canonical root is, rebuilds
/// the whole path, so a push a component would cost time quadratic in a
/// request's components. No component is empty or holds a separator, so
/// this names the same path.
fn joined(root: &Path, parts: &[&str]) -> PathBuf {
    let mut joined = root.to_path_buf();
    if !parts.is_empty() {
        joined.push(parts.join(std::path::MAIN_SEPARATOR_STR));
    }
    joined
}

/// The listing of the folder `path` names under `root`, at most `bound`
/// entries of it; see the module comment for the rule every step follows.
/// `path` appears in a refusal exactly as it was sent.
pub fn list(root: &Path, path: &str, bound: usize) -> Result<Listing, MError> {
    let parts = normalise(path)?;
    let Ok(canonical) = std::fs::canonicalize(joined(root, &parts)) else {
        return Err(error::files_not_a_folder(path));
    };
    if !canonical.starts_with(root) {
        return Err(error::files_outside_root(path));
    }
    if !canonical.is_dir() {
        return Err(error::files_not_a_folder(path));
    }
    let Ok(read) = std::fs::read_dir(&canonical) else {
        return Err(error::files_not_a_folder(path));
    };
    // The first `bound` entries in listing order, kept in a heap whose top
    // is the last of them, so memory is bounded by `bound` however large
    // the folder.
    let mut kept: BinaryHeap<Candidate> = BinaryHeap::new();
    let mut truncated = false;
    for entry in read.filter_map(Result::ok) {
        // A name the page could not send back is left out.
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let (dir, size) = classify(entry, root);
        kept.push(Candidate {
            not_dir: !dir,
            name,
            size,
        });
        if kept.len() > bound {
            kept.pop();
            truncated = true;
        }
    }
    let entries = kept
        .into_sorted_vec()
        .into_iter()
        .map(|c| {
            let size = match c.size {
                SizeFrom::Nothing => None,
                SizeFrom::Entry(e) => e.metadata().ok().map(|m| m.len()),
                SizeFrom::Target(t) => std::fs::metadata(t).ok().map(|m| m.len()),
            };
            Entry {
                name: c.name,
                dir: !c.not_dir,
                size,
            }
        })
        .collect();
    Ok(Listing {
        root: root_name(root),
        path: parts.join("/"),
        entries,
        truncated,
    })
}

/// A file's text, as `read_file` answers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileText {
    /// The path relative to the root, normalised.
    pub path: String,
    /// The file's bytes exactly, a byte-order mark included.
    pub text: String,
}

/// What `write_file` wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    /// The path relative to the root, normalised.
    pub path: String,
    /// The bytes written.
    pub size: u64,
    /// The file written, a full path: what the interpreter's file cache is
    /// told of (cycle U3).
    pub file: PathBuf,
}

/// The `.m` file `run_file` runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runnable {
    /// The file's full path, under the root.
    pub path: PathBuf,
    /// Its name, as `run` names a file it runs: the last component less
    /// its extension.
    pub name: String,
}

/// Step 3 for a file: the canonical path of the file `parts` names under
/// `root`, which must be inside the root, judged first, and a regular
/// file, which the root and a folder are not, nor a device or a pipe that
/// a read could wait on for ever.
fn existing_file(root: &Path, parts: &[&str], path: &str) -> Result<PathBuf, MError> {
    let Ok(canonical) = std::fs::canonicalize(joined(root, parts)) else {
        return Err(error::file_not_a_file(path));
    };
    if !canonical.starts_with(root) {
        return Err(error::files_outside_root(path));
    }
    if !canonical.is_file() {
        return Err(error::file_not_a_file(path));
    }
    Ok(canonical)
}

/// At most `bound` bytes from `reader`, whose length the file system gave
/// as `len`: `None` when `len` is past the bound, before a byte is read,
/// and when the reader holds more than the bound all the same, as a file
/// that grew since its length was read does, which it reads no further
/// than one byte past the bound.
fn read_bounded(reader: impl Read, len: u64, bound: u64) -> io::Result<Option<Vec<u8>>> {
    if len > bound {
        return Ok(None);
    }
    // `len` is at most the bound, so this is judged before it allocates.
    let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
    reader
        .take(bound.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > bound {
        return Ok(None);
    }
    Ok(Some(bytes))
}

/// `read_file`: the text of the file `path` names under `root`, at most
/// `bound` bytes of it; see the module comment for the rule. `path`
/// appears in a refusal exactly as it was sent.
pub fn read_file(root: &Path, path: &str, bound: u64) -> Result<FileText, MError> {
    let parts = normalise(path)?;
    let canonical = existing_file(root, &parts, path)?;
    // A file that cannot be opened or measured, one locked by another
    // program say, is not a file this can read.
    let not_read = || error::file_not_a_file(path);
    let file = std::fs::File::open(&canonical).map_err(|_| not_read())?;
    let len = file.metadata().map_err(|_| not_read())?.len();
    let bytes = read_bounded(file, len, bound)
        .map_err(|_| not_read())?
        .ok_or_else(|| error::file_too_large(path))?;
    let text = String::from_utf8(bytes).map_err(|_| error::file_not_utf8(path))?;
    Ok(FileText {
        path: parts.join("/"),
        text,
    })
}

/// A Windows device name, `CON`, `PRN`, `AUX`, `NUL`, `COM0` to `COM9` or
/// `LPT0` to `LPT9`, in any case.
fn is_device(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        _ => {
            let b = upper.as_bytes();
            b.len() == 4
                && (upper.starts_with("COM") || upper.starts_with("LPT"))
                && b[3].is_ascii_digit()
        }
    }
}

/// True when `name`, a last component, names the file it says on every
/// platform: it does not end in a `.` or a space, which Windows drops, and
/// it is not a device name alone or before a `.`, which Windows opens as
/// the device. The part before the first `.` is judged with its trailing
/// spaces dropped, as Windows drops them there too, so `NUL .m` is the
/// device as surely as `NUL.m` is.
fn names_itself(name: &str) -> bool {
    if name.ends_with('.') || name.ends_with(' ') {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name);
    !is_device(stem.trim_end_matches(' '))
}

/// `write_file`: `text` written, as UTF-8, to the file `path` names under
/// `root`, created or replaced, at most `bound` bytes of it; see the module
/// comment for the order of the checks. No folder is ever created. `path`
/// appears in a refusal exactly as it was sent.
pub fn write_file(root: &Path, path: &str, text: &str, bound: u64) -> Result<Written, MError> {
    let parts = normalise(path)?;
    let Some((&name, folder)) = parts.split_last() else {
        return Err(error::file_not_a_file(path));
    };
    if !names_itself(name) {
        return Err(error::file_not_a_file(path));
    }
    let Ok(dir) = std::fs::canonicalize(joined(root, folder)) else {
        return Err(error::file_no_folder(path));
    };
    if !dir.starts_with(root) {
        return Err(error::files_outside_root(path));
    }
    if !dir.is_dir() {
        return Err(error::file_no_folder(path));
    }
    let at = dir.join(name);
    // Whatever is at the name, a link included, is judged by where it
    // leads; nothing there is a new file.
    let target = match std::fs::symlink_metadata(&at) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => at,
        Err(_) => return Err(error::file_not_written(path)),
        Ok(_) => {
            let Ok(canonical) = std::fs::canonicalize(&at) else {
                // A link that resolves nowhere: writing through it would
                // make a file wherever it points.
                return Err(error::file_not_a_file(path));
            };
            if !canonical.starts_with(root) {
                return Err(error::files_outside_root(path));
            }
            if canonical.is_dir() {
                return Err(error::file_is_a_folder(path));
            }
            if !canonical.is_file() {
                return Err(error::file_not_a_file(path));
            }
            canonical
        }
    };
    if text.len() as u64 > bound {
        return Err(error::text_too_large(path));
    }
    std::fs::write(&target, text).map_err(|_| error::file_not_written(path))?;
    Ok(Written {
        path: parts.join("/"),
        size: text.len() as u64,
        file: target,
    })
}

/// `run_file`: the `.m` file `path` names under `root`, judged as
/// [`read_file`] judges it, then by its name. `path` appears in a refusal
/// exactly as it was sent.
pub fn run_file(root: &Path, path: &str) -> Result<Runnable, MError> {
    let parts = normalise(path)?;
    let canonical = existing_file(root, &parts, path)?;
    // The root is not a file, so a file has a last component.
    let last = parts.last().copied().unwrap_or_default();
    if !last.ends_with(".m") {
        return Err(error::file_not_m(path));
    }
    let name = Path::new(last)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Runnable {
        path: plain(&joined(root, &parts), &canonical),
        name,
    })
}

/// `full` without the `\\?\` a canonical root gives it on Windows, when
/// the plain form names the same file, `canonical`: a script run from it
/// then sees its folder as `pwd` writes any other, and can join a path to
/// it with `/`, which a verbatim path does not read as a separator. `full`
/// itself when the plain form names anything else, and on every other
/// platform.
#[cfg(windows)]
fn plain(full: &Path, canonical: &Path) -> PathBuf {
    let Some(text) = full.to_str() else {
        return full.to_path_buf();
    };
    let plain = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        Some(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\')
            .then(|| rest.to_string())
    } else {
        None
    };
    match plain.map(PathBuf::from) {
        Some(p) if std::fs::canonicalize(&p).is_ok_and(|c| c.as_path() == canonical) => p,
        _ => full.to_path_buf(),
    }
}

#[cfg(not(windows))]
fn plain(full: &Path, _canonical: &Path) -> PathBuf {
    full.to_path_buf()
}

/// The file an error frame records, relative to `root` with `/`
/// separators, judged on its canonical path, since a function found on
/// the path is recorded by a path that is not canonical and on Windows the
/// root is a verbatim one: `None` when it cannot be canonicalised, lies
/// outside the root or is the root, or holds a component that is not
/// Unicode, which the page could not name back.
pub fn relative(root: &Path, file: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(file).ok()?;
    let rest = canonical.strip_prefix(root).ok()?;
    let parts = rest
        .components()
        .map(|c| match c {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<&str>>>()?;
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the temporary folder, removed when dropped.
    struct Dir(PathBuf);

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The acceptance tests' fixture, canonicalised as a session's root is:
    /// `tree/` holding `Z.txt` (2 bytes), `a.txt` (empty), `b.txt`
    /// (5 bytes), `sub/deep.txt` (3 bytes) and `emptyish/.keep`.
    fn fixture(name: &str) -> Dir {
        let d =
            std::env::temp_dir().join(format!("splatcrab-files-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let tree = d.join("U2-ui-desktop").join("tree");
        std::fs::create_dir_all(tree.join("sub")).unwrap();
        std::fs::create_dir_all(tree.join("emptyish")).unwrap();
        std::fs::write(tree.join("Z.txt"), "xy").unwrap();
        std::fs::write(tree.join("a.txt"), "").unwrap();
        std::fs::write(tree.join("b.txt"), "hello").unwrap();
        std::fs::write(tree.join("sub").join("deep.txt"), "abc").unwrap();
        std::fs::write(tree.join("emptyish").join(".keep"), "").unwrap();
        Dir(d)
    }

    fn root_of(d: &Dir) -> PathBuf {
        std::fs::canonicalize(d.0.join("U2-ui-desktop")).unwrap()
    }

    fn entry(name: &str, dir: bool, size: Option<u64>) -> Entry {
        Entry {
            name: name.to_string(),
            dir,
            size,
        }
    }

    fn msg(r: Result<Listing, MError>) -> String {
        r.expect_err("a refusal").msg
    }

    #[test]
    fn step_one_refuses_what_could_name_another_place() {
        for path in [
            "/etc",
            "/",
            "C:/Windows",
            "C:",
            "tree\\sub",
            "a:b",
            "tree/x:stream",
            "\\\\server\\share",
            "a\0b",
        ] {
            assert_eq!(
                normalise(path).unwrap_err().msg,
                "Malformed request: 'path' must be a relative path with '/' separators.",
                "{path:?}"
            );
        }
    }

    #[test]
    fn step_two_resolves_dots_on_the_text_alone() {
        assert_eq!(normalise("").unwrap(), Vec::<&str>::new());
        assert_eq!(normalise(".").unwrap(), Vec::<&str>::new());
        assert_eq!(normalise("tree/./sub/../sub/").unwrap(), ["tree", "sub"]);
        assert_eq!(normalise("tree//sub").unwrap(), ["tree", "sub"]);
        assert_eq!(normalise("tree/sub/..").unwrap(), ["tree"]);
        assert_eq!(normalise("a/b/../../c").unwrap(), ["c"]);
        // A `..` with nothing before it is refused, even when the path would
        // come back inside, and the refusal names the path as sent.
        for path in [
            "..",
            "tree/../..",
            "../U2-ui-desktop/tree",
            "./..",
            "a/../../a",
        ] {
            assert_eq!(
                normalise(path).unwrap_err().msg,
                format!("Path '{path}' is outside the file root."),
                "{path:?}"
            );
        }
    }

    #[test]
    fn a_folder_is_listed_folders_first_then_by_byte_order() {
        let d = fixture("order");
        let root = root_of(&d);
        let got = list(&root, "tree", MAX_ENTRIES).unwrap();
        assert_eq!(got.root, "U2-ui-desktop");
        assert_eq!(got.path, "tree");
        assert!(!got.truncated);
        assert_eq!(
            got.entries,
            [
                entry("emptyish", true, None),
                entry("sub", true, None),
                entry("Z.txt", false, Some(2)),
                entry("a.txt", false, Some(0)),
                entry("b.txt", false, Some(5)),
            ]
        );
        // The same folder by other spellings, and the path reported
        // normalised.
        for spelled in ["tree/./sub/../sub/", "tree//sub"] {
            let sub = list(&root, spelled, MAX_ENTRIES).unwrap();
            assert_eq!(sub.path, "tree/sub");
            assert_eq!(sub.entries, [entry("deep.txt", false, Some(3))]);
        }
        let back = list(&root, "tree/sub/..", MAX_ENTRIES).unwrap();
        assert_eq!(back.path, "tree");
        assert_eq!(back.entries, got.entries);
        // A folder whose only entry starts with a dot.
        let e = list(&root, "tree/emptyish", MAX_ENTRIES).unwrap();
        assert_eq!(e.entries, [entry(".keep", false, Some(0))]);
        // The root itself.
        let top = list(&root, "", MAX_ENTRIES).unwrap();
        assert_eq!(top.path, "");
        assert_eq!(top.entries, [entry("tree", true, None)]);
    }

    #[test]
    fn step_three_refuses_what_is_missing_or_not_a_folder() {
        let d = fixture("kinds");
        let root = root_of(&d);
        assert_eq!(
            msg(list(&root, "tree/a.txt", MAX_ENTRIES)),
            "Path 'tree/a.txt' is not a folder."
        );
        assert_eq!(
            msg(list(&root, "nope", MAX_ENTRIES)),
            "Path 'nope' is not a folder."
        );
        assert_eq!(
            msg(list(&root, "tree/sub/deep.txt/", MAX_ENTRIES)),
            "Path 'tree/sub/deep.txt/' is not a folder."
        );
        // The text is resolved first, so a missing folder that a `..` steps
        // back out of is never looked for: this is `tree`.
        assert_eq!(
            list(&root, "tree/nope/..", MAX_ENTRIES).unwrap().path,
            "tree"
        );
    }

    /// Invariant 6: a path of 100,000 components is judged in time linear
    /// in its length. Joined a component at a time onto a verbatim root it
    /// took minutes on Windows; the bound here is far above what it takes.
    #[test]
    fn a_path_of_many_components_is_judged_in_linear_time() {
        let d = fixture("many");
        let root = root_of(&d);
        let start = std::time::Instant::now();
        let deep = "a/".repeat(100_000);
        assert_eq!(
            msg(list(&root, &deep, MAX_ENTRIES)),
            format!("Path '{deep}' is not a folder.")
        );
        let inside = format!("tree/sub/{}", "b/".repeat(100_000));
        assert_eq!(
            msg(list(&root, &inside, MAX_ENTRIES)),
            format!("Path '{inside}' is not a folder.")
        );
        // Steps that cancel out on the text come back to a folder that is
        // there.
        let back = format!("tree/{}", "x/../".repeat(100_000));
        assert_eq!(list(&root, &back, MAX_ENTRIES).unwrap().path, "tree");
        assert!(start.elapsed().as_secs() < 10, "{:?}", start.elapsed());
    }

    /// A root that is not canonical refuses every listing as outside
    /// itself rather than listing anything.
    #[test]
    fn an_uncanonical_root_refuses_as_outside() {
        let d = fixture("uncanonical");
        let root = d.0.join("U2-ui-desktop").join("tree").join("..");
        assert_eq!(
            msg(list(&root, "tree", MAX_ENTRIES)),
            "Path 'tree' is outside the file root."
        );
    }

    #[test]
    fn the_bound_keeps_the_first_entries_in_listing_order() {
        let d = fixture("bound");
        let root = root_of(&d);
        let got = list(&root, "tree", 3).unwrap();
        assert!(got.truncated);
        assert_eq!(
            got.entries,
            [
                entry("emptyish", true, None),
                entry("sub", true, None),
                entry("Z.txt", false, Some(2)),
            ]
        );
        let exact = list(&root, "tree", 5).unwrap();
        assert!(!exact.truncated);
        assert_eq!(exact.entries.len(), 5);
        let none = list(&root, "tree", 0).unwrap();
        assert!(none.truncated && none.entries.is_empty());
        assert!(!list(&root, "tree/sub", 1).unwrap().truncated);
    }

    #[test]
    fn the_root_is_named_by_its_last_component() {
        assert_eq!(root_name(Path::new("/")), "/");
        assert_eq!(root_name(Path::new("/home/u/work")), "work");
        if cfg!(windows) {
            assert_eq!(root_name(Path::new(r"\\?\C:\")), "/");
            assert_eq!(root_name(Path::new(r"\\?\C:\Users\x\U2")), "U2");
        }
        let root = session_root();
        assert!(root.is_absolute(), "{}", root.display());
    }

    /// A name that is not valid Unicode is left out, since the page could
    /// not name it back.
    #[test]
    fn a_name_that_is_not_unicode_is_left_out() {
        let d = fixture("unicode");
        let root = root_of(&d);
        let sub = root.join("tree").join("sub");
        #[cfg(unix)]
        let bad = {
            use std::os::unix::ffi::OsStrExt;
            sub.join(std::ffi::OsStr::from_bytes(b"bad\xff.txt"))
        };
        #[cfg(windows)]
        let bad = {
            use std::os::windows::ffi::OsStringExt;
            let name = std::ffi::OsString::from_wide(&[0x62, 0xD800, 0x2E, 0x74]);
            sub.join(name)
        };
        #[cfg(any(unix, windows))]
        {
            if std::fs::write(&bad, "q").is_err() {
                // A file system that refuses such a name cannot hold one.
                return;
            }
            let got = list(&root, "tree/sub", MAX_ENTRIES).unwrap();
            assert_eq!(got.entries, [entry("deep.txt", false, Some(3))]);
        }
    }

    /// Links on Unix: one inside the root that points outside is refused
    /// as a path and listed as neither a folder nor a sized file; one that
    /// points inside is listed as its target; a dangling one as outside.
    #[cfg(unix)]
    #[test]
    fn links_are_judged_by_where_they_point() {
        use std::os::unix::fs::symlink;
        let d = fixture("links");
        let root = root_of(&d);
        let outside = d.0.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "12345678").unwrap();
        let tree = root.join("tree");
        symlink(&outside, tree.join("out_dir")).unwrap();
        symlink(outside.join("secret.txt"), tree.join("out_file")).unwrap();
        symlink(tree.join("sub"), tree.join("in_dir")).unwrap();
        symlink(tree.join("b.txt"), tree.join("in_file")).unwrap();
        symlink(tree.join("missing"), tree.join("dangling")).unwrap();

        assert_eq!(
            msg(list(&root, "tree/out_dir", MAX_ENTRIES)),
            "Path 'tree/out_dir' is outside the file root."
        );
        assert_eq!(
            msg(list(&root, "tree/out_file", MAX_ENTRIES)),
            "Path 'tree/out_file' is outside the file root.",
            "judged outside before its kind"
        );
        assert_eq!(
            msg(list(&root, "tree/dangling", MAX_ENTRIES)),
            "Path 'tree/dangling' is not a folder."
        );
        let inside = list(&root, "tree/in_dir", MAX_ENTRIES).unwrap();
        assert_eq!(inside.path, "tree/in_dir");
        assert_eq!(inside.entries, [entry("deep.txt", false, Some(3))]);

        let got = list(&root, "tree", MAX_ENTRIES).unwrap();
        assert_eq!(
            got.entries,
            [
                entry("emptyish", true, None),
                entry("in_dir", true, None),
                entry("sub", true, None),
                entry("Z.txt", false, Some(2)),
                entry("a.txt", false, Some(0)),
                entry("b.txt", false, Some(5)),
                entry("dangling", false, None),
                entry("in_file", false, Some(5)),
                entry("out_dir", false, None),
                entry("out_file", false, None),
            ]
        );
    }

    // ---- the editor's operations (cycle U3) --------------------------------

    fn text_of(r: Result<FileText, MError>) -> String {
        r.expect("the file is read").text
    }

    fn read_msg(r: Result<FileText, MError>) -> String {
        r.expect_err("a refusal").msg
    }

    fn write_msg(r: Result<Written, MError>) -> String {
        r.expect_err("a refusal").msg
    }

    /// What the fixture's `tree` holds, by name, so a refusal can be shown
    /// to have created nothing.
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn read_file_answers_the_text_exactly_under_its_normalised_path() {
        let d = fixture("read");
        let root = root_of(&d);
        std::fs::write(root.join("tree").join("crlf.txt"), b"a\r\nb\r\n").unwrap();
        std::fs::write(root.join("tree").join("bom.m"), "\u{feff}x = 1;\n").unwrap();
        let got = read_file(&root, "tree/./sub/../b.txt", MAX_TEXT).unwrap();
        assert_eq!(got.path, "tree/b.txt");
        assert_eq!(got.text, "hello");
        assert_eq!(
            text_of(read_file(&root, "tree/crlf.txt", MAX_TEXT)),
            "a\r\nb\r\n"
        );
        // A byte-order mark is part of the text.
        assert_eq!(
            text_of(read_file(&root, "tree/bom.m", MAX_TEXT)),
            "\u{feff}x = 1;\n"
        );
        assert_eq!(text_of(read_file(&root, "tree/a.txt", MAX_TEXT)), "");
    }

    #[test]
    fn read_file_refuses_what_is_not_a_file_inside_the_root() {
        let d = fixture("read-refusals");
        let root = root_of(&d);
        for (path, want) in [
            ("..", "Path '..' is outside the file root."),
            ("tree/../..", "Path 'tree/../..' is outside the file root."),
            (
                "/etc",
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
            (
                "tree\\b.txt",
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
            ("", "Path '' is not a file."),
            (".", "Path '.' is not a file."),
            ("tree", "Path 'tree' is not a file."),
            ("tree/sub/", "Path 'tree/sub/' is not a file."),
            ("nope.m", "Path 'nope.m' is not a file."),
            ("tree/b.txt/x", "Path 'tree/b.txt/x' is not a file."),
        ] {
            assert_eq!(read_msg(read_file(&root, path, MAX_TEXT)), want, "{path:?}");
        }
    }

    /// Acceptance test 11: a file of exactly 4 MiB is read and one byte
    /// more is refused.
    #[test]
    fn read_file_reads_four_mebibytes_and_refuses_one_byte_more() {
        let d = fixture("read-bound");
        let root = root_of(&d);
        let at = MAX_TEXT as usize;
        std::fs::write(root.join("at.txt"), "a".repeat(at)).unwrap();
        std::fs::write(root.join("past.txt"), "a".repeat(at + 1)).unwrap();
        assert_eq!(text_of(read_file(&root, "at.txt", MAX_TEXT)).len(), at);
        assert_eq!(
            read_msg(read_file(&root, "past.txt", MAX_TEXT)),
            "Path 'past.txt' is larger than 4 MiB."
        );
        // The bound is a parameter, so the rule holds at any size.
        std::fs::write(root.join("five.txt"), "12345").unwrap();
        assert_eq!(text_of(read_file(&root, "five.txt", 5)), "12345");
        assert_eq!(
            read_msg(read_file(&root, "five.txt", 4)),
            "Path 'five.txt' is larger than 4 MiB."
        );
    }

    /// A reader that fails the test if it is read at all, and one that
    /// never ends, counting what it gives.
    struct Untouchable;

    impl Read for Untouchable {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("a file past the bound was read");
        }
    }

    struct Endless(std::rc::Rc<std::cell::Cell<u64>>);

    impl Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            buf.fill(b'a');
            self.0.set(self.0.get() + buf.len() as u64);
            Ok(buf.len())
        }
    }

    /// The length is judged before a byte is read, and a file that grew
    /// past its length is read no further than one byte past the bound.
    #[test]
    fn a_read_is_judged_by_its_length_first_and_bounded_after() {
        assert_eq!(
            read_bounded(Untouchable, MAX_TEXT + 1, MAX_TEXT).unwrap(),
            None
        );
        assert_eq!(read_bounded(Untouchable, u64::MAX, MAX_TEXT).unwrap(), None);
        let given = std::rc::Rc::new(std::cell::Cell::new(0));
        assert_eq!(
            read_bounded(Endless(given.clone()), 3, 1000).unwrap(),
            None,
            "a file that grew past the bound"
        );
        assert_eq!(given.get(), 1001, "read to one byte past the bound");
        assert_eq!(
            read_bounded(&b"abc"[..], 3, 3).unwrap(),
            Some(b"abc".to_vec())
        );
    }

    #[test]
    fn read_file_refuses_text_that_is_not_utf8() {
        let d = fixture("read-utf8");
        let root = root_of(&d);
        std::fs::write(root.join("latin1.txt"), b"caf\xe9").unwrap();
        std::fs::write(root.join("utf16.m"), b"\xff\xfex\x00").unwrap();
        for name in ["latin1.txt", "utf16.m"] {
            assert_eq!(
                read_msg(read_file(&root, name, MAX_TEXT)),
                format!("Path '{name}' is not UTF-8 text.")
            );
        }
    }

    #[test]
    fn write_file_creates_and_replaces_under_its_normalised_path() {
        let d = fixture("write");
        let root = root_of(&d);
        let got = write_file(&root, "tree/./new.m", "disp(7)\ndisp(8)\n", MAX_TEXT).unwrap();
        // A new file is named by its canonical folder and its name.
        assert_eq!(
            got,
            Written {
                path: "tree/new.m".to_string(),
                size: 16,
                file: root.join("tree").join("new.m"),
            }
        );
        assert_eq!(
            std::fs::read_to_string(root.join("tree").join("new.m")).unwrap(),
            "disp(7)\ndisp(8)\n"
        );
        // Replaced, shorter, and the size counts UTF-8 bytes; a file that
        // was there is named by its canonical path.
        let got = write_file(&root, "tree/new.m", "×", MAX_TEXT).unwrap();
        assert_eq!(got.size, 2);
        assert_eq!(
            got.file,
            std::fs::canonicalize(root.join("tree").join("new.m")).unwrap()
        );
        assert_eq!(
            std::fs::read(root.join("tree").join("new.m")).unwrap(),
            "×".as_bytes()
        );
        // Carriage returns and a byte-order mark are written as sent.
        write_file(&root, "tree/b.txt", "\u{feff}a\r\nb\r\n", MAX_TEXT).unwrap();
        assert_eq!(
            text_of(read_file(&root, "tree/b.txt", MAX_TEXT)),
            "\u{feff}a\r\nb\r\n"
        );
        // A name close to a device's is an ordinary name.
        for name in ["CONS.m", "COM10.m", "nul_x.m", "xcon", "LPT.m", "a.con"] {
            write_file(&root, &format!("tree/sub/{name}"), "1", MAX_TEXT)
                .unwrap_or_else(|e| panic!("{name}: {}", e.msg));
        }
    }

    #[test]
    fn write_file_refuses_in_the_spec_order_and_creates_nothing() {
        let d = fixture("write-refusals");
        let root = root_of(&d);
        let before = names_in(&root.join("tree"));
        for (path, want) in [
            ("..", "Path '..' is outside the file root."),
            (
                "tree/../../x.m",
                "Path 'tree/../../x.m' is outside the file root.",
            ),
            (
                "/x.m",
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
            ("", "Path '' is not a file."),
            ("tree/..", "Path 'tree/..' is not a file."),
            ("tree/NUL.m", "Path 'tree/NUL.m' is not a file."),
            ("tree/con", "Path 'tree/con' is not a file."),
            ("tree/x.m.", "Path 'tree/x.m.' is not a file."),
            ("tree/x.m ", "Path 'tree/x.m ' is not a file."),
            // The name is judged before the folder.
            ("nofolder/con", "Path 'nofolder/con' is not a file."),
            (
                "nofolder/x.m",
                "Path 'nofolder/x.m' is not in a folder of the file root.",
            ),
            (
                "tree/b.txt/x.m",
                "Path 'tree/b.txt/x.m' is not in a folder of the file root.",
            ),
            ("tree", "Path 'tree' is a folder."),
            ("tree/sub", "Path 'tree/sub' is a folder."),
        ] {
            assert_eq!(
                write_msg(write_file(&root, path, "x", MAX_TEXT)),
                want,
                "{path:?}"
            );
        }
        assert_eq!(names_in(&root.join("tree")), before);
        assert_eq!(names_in(&root), ["tree"]);
    }

    /// Acceptance test 11: every device name, in any case, alone or before
    /// a `.`, and every name ending in a `.` or a space, on every platform.
    #[test]
    fn every_device_name_and_trailing_character_is_refused() {
        let d = fixture("devices");
        let root = root_of(&d);
        let mut devices: Vec<String> = ["CON", "PRN", "AUX", "NUL"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for k in 0..10 {
            devices.push(format!("COM{k}"));
            devices.push(format!("LPT{k}"));
        }
        for device in &devices {
            let lower = device.to_lowercase();
            let mixed: String = device
                .chars()
                .enumerate()
                .map(|(k, c)| {
                    if k % 2 == 0 {
                        c.to_ascii_lowercase()
                    } else {
                        c
                    }
                })
                .collect();
            for stem in [device.clone(), lower, mixed] {
                for name in [
                    stem.clone(),
                    format!("{stem}.m"),
                    format!("{stem}.txt"),
                    format!("{stem}.tar.gz"),
                    format!("{stem} .m"),
                ] {
                    let path = format!("tree/{name}");
                    assert_eq!(
                        write_msg(write_file(&root, &path, "x", MAX_TEXT)),
                        format!("Path '{path}' is not a file."),
                        "{path:?}"
                    );
                }
            }
        }
        for name in ["x.", "x.m.", "x ", "x.m ", "...", "a. ", "x.m\t."] {
            let path = format!("tree/{name}");
            assert_eq!(
                write_msg(write_file(&root, &path, "x", MAX_TEXT)),
                format!("Path '{path}' is not a file."),
                "{path:?}"
            );
        }
        assert_eq!(
            names_in(&root.join("tree")),
            ["Z.txt", "a.txt", "b.txt", "emptyish", "sub"]
        );
    }

    /// Acceptance test 11: a text of exactly 4 MiB is written and one byte
    /// more is refused, with nothing created.
    #[test]
    fn write_file_writes_four_mebibytes_and_refuses_one_byte_more() {
        let d = fixture("write-bound");
        let root = root_of(&d);
        let at = MAX_TEXT as usize;
        let got = write_file(&root, "at.txt", &"a".repeat(at), MAX_TEXT).unwrap();
        assert_eq!(got.size, MAX_TEXT);
        assert_eq!(
            std::fs::metadata(root.join("at.txt")).unwrap().len(),
            MAX_TEXT
        );
        assert_eq!(
            write_msg(write_file(&root, "past.txt", &"a".repeat(at + 1), MAX_TEXT)),
            "Text for 'past.txt' is larger than 4 MiB."
        );
        assert!(!root.join("past.txt").exists());
        // A multi-byte character counts its UTF-8 bytes.
        assert_eq!(
            write_msg(write_file(&root, "wide.txt", "××", 3)),
            "Text for 'wide.txt' is larger than 4 MiB."
        );
        assert_eq!(write_file(&root, "wide.txt", "××", 4).unwrap().size, 4);
    }

    #[test]
    fn run_file_needs_a_dot_m_file_inside_the_root() {
        let d = fixture("run");
        let root = root_of(&d);
        std::fs::write(root.join("tree").join("s.m"), "disp(1)\n").unwrap();
        let got = run_file(&root, "tree/./s.m").unwrap();
        assert_eq!(got.name, "s");
        assert_eq!(
            std::fs::canonicalize(&got.path).unwrap(),
            root.join("tree").join("s.m")
        );
        for (path, want) in [
            ("tree/b.txt", "Path 'tree/b.txt' is not a .m file."),
            ("tree/nope.m", "Path 'tree/nope.m' is not a file."),
            ("tree", "Path 'tree' is not a file."),
            ("", "Path '' is not a file."),
            ("../x.m", "Path '../x.m' is outside the file root."),
            (
                "C:/x.m",
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
        ] {
            assert_eq!(run_file(&root, path).unwrap_err().msg, want, "{path:?}");
        }
    }

    /// On Windows the path run is the plain form of the verbatim root's, so
    /// a script sees its folder as `pwd` writes it; elsewhere it is the
    /// root's own.
    #[test]
    fn run_file_runs_a_plain_path() {
        let d = fixture("plain");
        let root = root_of(&d);
        std::fs::write(root.join("tree").join("s.m"), "disp(1)\n").unwrap();
        let got = run_file(&root, "tree/s.m").unwrap();
        let text = got.path.to_string_lossy().into_owned();
        assert!(!text.starts_with(r"\\?\"), "{text}");
        assert!(got.path.is_file());
    }

    #[test]
    fn relative_names_a_frame_file_under_the_root() {
        let d = fixture("relative");
        let root = root_of(&d);
        // The file as the interpreter records one it found on the path:
        // not canonical, and with a `..` in it.
        let spelled =
            d.0.join("U2-ui-desktop")
                .join("tree")
                .join("sub")
                .join("..")
                .join("b.txt");
        assert_eq!(relative(&root, &spelled).as_deref(), Some("tree/b.txt"));
        assert_eq!(
            relative(&root, &root.join("tree").join("sub").join("deep.txt")).as_deref(),
            Some("tree/sub/deep.txt")
        );
        let outside = d.0.join("outside.m");
        std::fs::write(&outside, "x").unwrap();
        assert_eq!(relative(&root, &outside), None);
        assert_eq!(relative(&root, &root), None);
        assert_eq!(relative(&root, &root.join("missing.m")), None);
        assert_eq!(relative(&root, Path::new("")), None);
    }

    /// Acceptance test 11, on Unix: a dangling link and a link out of the
    /// root are refused by `write_file` with nothing created, and a link
    /// inside the root is written through to its target.
    #[cfg(unix)]
    #[test]
    fn write_file_never_follows_a_link_out_of_the_root() {
        use std::os::unix::fs::symlink;
        let d = fixture("write-links");
        let root = root_of(&d);
        let outside = d.0.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.m"), "keep").unwrap();
        let tree = root.join("tree");
        symlink(outside.join("secret.m"), tree.join("out.m")).unwrap();
        symlink(outside.join("made.m"), tree.join("dangling_out.m")).unwrap();
        symlink(tree.join("missing.m"), tree.join("dangling_in.m")).unwrap();
        symlink(&outside, tree.join("out_dir")).unwrap();
        symlink(tree.join("b.txt"), tree.join("in.txt")).unwrap();
        symlink(tree.join("sub"), tree.join("in_dir")).unwrap();

        assert_eq!(
            write_msg(write_file(&root, "tree/out.m", "x", MAX_TEXT)),
            "Path 'tree/out.m' is outside the file root."
        );
        assert_eq!(
            std::fs::read_to_string(outside.join("secret.m")).unwrap(),
            "keep"
        );
        for name in ["dangling_out.m", "dangling_in.m"] {
            let path = format!("tree/{name}");
            assert_eq!(
                write_msg(write_file(&root, &path, "x", MAX_TEXT)),
                format!("Path '{path}' is not a file.")
            );
        }
        assert!(!outside.join("made.m").exists());
        assert!(!tree.join("missing.m").exists());
        // A folder reached through a link out of the root is outside it.
        assert_eq!(
            write_msg(write_file(&root, "tree/out_dir/x.m", "x", MAX_TEXT)),
            "Path 'tree/out_dir/x.m' is outside the file root."
        );
        assert!(!outside.join("x.m").exists());
        assert_eq!(
            write_msg(write_file(&root, "tree/out_dir", "x", MAX_TEXT)),
            "Path 'tree/out_dir' is outside the file root."
        );
        assert_eq!(
            write_msg(write_file(&root, "tree/in_dir", "x", MAX_TEXT)),
            "Path 'tree/in_dir' is a folder."
        );
        // Inside the root a link is written through, and stays a link.
        write_file(&root, "tree/in.txt", "new", MAX_TEXT).unwrap();
        assert_eq!(std::fs::read_to_string(tree.join("b.txt")).unwrap(), "new");
        assert!(
            std::fs::symlink_metadata(tree.join("in.txt"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        // A read follows the same rule.
        assert_eq!(
            read_msg(read_file(&root, "tree/out.m", MAX_TEXT)),
            "Path 'tree/out.m' is outside the file root."
        );
        assert_eq!(
            read_msg(read_file(&root, "tree/dangling_in.m", MAX_TEXT)),
            "Path 'tree/dangling_in.m' is not a file."
        );
    }

    /// The same on Windows where a link can be made, which needs developer
    /// mode or elevation; elsewhere the test has nothing to do.
    #[cfg(windows)]
    #[test]
    fn write_file_never_follows_a_link_out_of_the_root() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let d = fixture("write-links");
        let root = root_of(&d);
        let outside = d.0.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.m"), "keep").unwrap();
        let tree = root.join("tree");
        if symlink_file(outside.join("secret.m"), tree.join("out.m")).is_err() {
            return;
        }
        symlink_file(outside.join("made.m"), tree.join("dangling_out.m")).unwrap();
        symlink_dir(&outside, tree.join("out_dir")).unwrap();
        assert_eq!(
            write_msg(write_file(&root, "tree/out.m", "x", MAX_TEXT)),
            "Path 'tree/out.m' is outside the file root."
        );
        assert_eq!(
            std::fs::read_to_string(outside.join("secret.m")).unwrap(),
            "keep"
        );
        assert_eq!(
            write_msg(write_file(&root, "tree/dangling_out.m", "x", MAX_TEXT)),
            "Path 'tree/dangling_out.m' is not a file."
        );
        assert!(!outside.join("made.m").exists());
        assert_eq!(
            write_msg(write_file(&root, "tree/out_dir/x.m", "x", MAX_TEXT)),
            "Path 'tree/out_dir/x.m' is outside the file root."
        );
        assert!(!outside.join("x.m").exists());
    }

    /// The same on Windows where a directory link can be made, which needs
    /// developer mode or elevation; elsewhere the test has nothing to do.
    #[cfg(windows)]
    #[test]
    fn links_are_judged_by_where_they_point() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let d = fixture("links");
        let root = root_of(&d);
        let outside = d.0.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "12345678").unwrap();
        let tree = root.join("tree");
        if symlink_dir(&outside, tree.join("out_dir")).is_err() {
            return;
        }
        symlink_dir(tree.join("sub"), tree.join("in_dir")).unwrap();
        symlink_file(tree.join("b.txt"), tree.join("in_file")).unwrap();
        symlink_file(outside.join("secret.txt"), tree.join("out_file")).unwrap();
        assert_eq!(
            msg(list(&root, "tree/out_dir", MAX_ENTRIES)),
            "Path 'tree/out_dir' is outside the file root."
        );
        let got = list(&root, "tree", MAX_ENTRIES).unwrap();
        assert!(got.entries.contains(&entry("in_dir", true, None)));
        assert!(got.entries.contains(&entry("in_file", false, Some(5))));
        assert!(got.entries.contains(&entry("out_dir", false, None)));
        assert!(got.entries.contains(&entry("out_file", false, None)));
    }
}
