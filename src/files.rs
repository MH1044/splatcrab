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

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};

use crate::error::{self, MError};

/// The most entries one listing holds: what the protocol's `files` passes
/// to [`list`] as its bound.
pub const MAX_ENTRIES: usize = 10_000;

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

/// The listing of the folder `path` names under `root`, at most `bound`
/// entries of it; see the module comment for the rule every step follows.
/// `path` appears in a refusal exactly as it was sent.
pub fn list(root: &Path, path: &str, bound: usize) -> Result<Listing, MError> {
    let parts = normalise(path)?;
    // One push of the whole relative path, never one a component: on
    // Windows a push onto a verbatim `\\?\` root, which a canonical root
    // is, rebuilds the whole path, so a push a component would cost time
    // quadratic in a request's components. No component is empty or holds
    // a separator, so this names the same path.
    let mut joined = root.to_path_buf();
    if !parts.is_empty() {
        joined.push(parts.join(std::path::MAIN_SEPARATOR_STR));
    }
    let Ok(canonical) = std::fs::canonicalize(&joined) else {
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
