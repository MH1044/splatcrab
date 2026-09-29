//! The environment a name can come from, as seen from outside the evaluator.
//!
//! [`completions`] answers the protocol's `completions` operation: every name
//! a user could type next, from the three places a name resolves: the
//! workspace variables, the `.m` files on the path (cycle 13) and the
//! builtin registry. The terminal's tab completion (cycle 13) calls the
//! same function, so the two can never offer different names.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::builtins::Registry;
use crate::value::Value;

/// Every variable, function file on the path and builtin whose name starts
/// with `prefix`, sorted by byte order and without duplicates: a variable
/// or a file that shadows a builtin is one name, not two. An empty prefix
/// lists everything.
///
/// A function file is a file in one of the `path` folders whose name is a
/// MATLAB identifier followed by `.m`, listed without the `.m`, as a call
/// names it. A folder that cannot be read contributes nothing.
pub fn completions(
    prefix: &str,
    vars: &HashMap<String, Value>,
    registry: &Registry,
    path: &[PathBuf],
) -> Vec<String> {
    let mut names: Vec<String> = vars
        .keys()
        .map(String::as_str)
        .chain(registry.keys().copied())
        .filter(|n| n.starts_with(prefix))
        .map(str::to_string)
        .collect();
    for dir in path {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let file = entry.file_name();
            let Some(stem) = file.to_str().and_then(|f| f.strip_suffix(".m")) else {
                continue;
            };
            let is_file = entry.file_type().is_ok_and(|t| !t.is_dir());
            if is_file && stem.starts_with(prefix) && crate::interp::is_identifier(stem) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::registry;
    use crate::value::Matrix;

    fn vars(names: &[&str]) -> HashMap<String, Value> {
        names
            .iter()
            .map(|n| (n.to_string(), Value::Mat(Matrix::scalar(1.0))))
            .collect()
    }

    #[test]
    fn builtins_alone() {
        let r = registry();
        assert_eq!(completions("dis", &vars(&[]), &r, &[]), ["disp"]);
        assert_eq!(
            completions("zzz", &vars(&[]), &r, &[]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn variables_and_builtins_are_merged_in_order() {
        let r = registry();
        let got = completions("dis", &vars(&["display_count", "dia"]), &r, &[]);
        assert_eq!(got, ["disp", "display_count"]);
        // Cycle 12's `xlabel` and `xlim` sort among the variables.
        let got = completions("x", &vars(&["x2", "x", "x10"]), &r, &[]);
        assert_eq!(got, ["x", "x10", "x2", "xlabel", "xlim"]);
    }

    #[test]
    fn a_variable_shadowing_a_builtin_appears_once() {
        let r = registry();
        let got = completions("disp", &vars(&["disp"]), &r, &[]);
        assert_eq!(got, ["disp"]);
    }

    #[test]
    fn an_empty_prefix_lists_every_name_once_sorted() {
        let r = registry();
        let got = completions("", &vars(&["sum", "aaa"]), &r, &[]);
        assert_eq!(got.len(), r.len() + 1);
        assert!(got.windows(2).all(|w| w[0] < w[1]));
        // Byte order, as `who` sorts: capitals first.
        assert_eq!(got[..3], ["Inf", "NaN", "aaa"]);
    }

    /// Cycle 13: the `.m` files of every path folder, without the `.m`,
    /// merged with the rest; a file that is not a function name, a folder
    /// and a file shadowing a builtin are handled.
    #[test]
    fn path_files_are_listed_by_their_function_names() {
        let dir = std::env::temp_dir().join(format!("splatcrab-env-{}", std::process::id()));
        let other = dir.join("more");
        std::fs::create_dir_all(&other).unwrap();
        for f in ["mytool.m", "myother.m", "disp.m", "my-bad.m", "mydata.txt"] {
            std::fs::write(dir.join(f), "x = 1;\n").unwrap();
        }
        std::fs::create_dir_all(dir.join("mydir.m")).unwrap();
        std::fs::write(other.join("myfar.m"), "x = 1;\n").unwrap();
        let r = registry();
        let path = [dir.clone(), other.clone(), dir.join("missing")];
        let got = completions("my", &vars(&["myvar"]), &r, &path);
        assert_eq!(got, ["myfar", "myother", "mytool", "myvar"]);
        assert_eq!(completions("disp", &vars(&[]), &r, &path), ["disp"]);
        // No path: exactly the names the protocol offered before.
        assert_eq!(completions("my", &vars(&[]), &r, &[]), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_prefix_is_case_sensitive() {
        let r = registry();
        assert_eq!(completions("D", &vars(&["Data"]), &r, &[]), ["Data"]);
    }
}
