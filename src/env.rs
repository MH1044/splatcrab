//! The environment a name can come from, as seen from outside the evaluator.
//!
//! [`completions`] answers the protocol's `completions` operation: every name
//! a user could type next, from the two places a name resolves today, the
//! workspace variables and the builtin registry. Cycle 13 adds files on the
//! path, and builds the terminal's tab completion on the same function.

use std::collections::HashMap;

use crate::builtins::Registry;
use crate::value::Value;

/// Every variable and builtin whose name starts with `prefix`, sorted by
/// byte order and without duplicates: a variable that shadows a builtin is
/// one name, not two. An empty prefix lists everything.
pub fn completions(
    prefix: &str,
    vars: &HashMap<String, Value>,
    registry: &Registry,
) -> Vec<String> {
    let mut names: Vec<String> = vars
        .keys()
        .map(String::as_str)
        .chain(registry.keys().copied())
        .filter(|n| n.starts_with(prefix))
        .map(str::to_string)
        .collect();
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
        assert_eq!(completions("dis", &vars(&[]), &r), ["disp"]);
        assert_eq!(completions("zzz", &vars(&[]), &r), Vec::<String>::new());
    }

    #[test]
    fn variables_and_builtins_are_merged_in_order() {
        let r = registry();
        let got = completions("dis", &vars(&["display_count", "dia"]), &r);
        assert_eq!(got, ["disp", "display_count"]);
        // Cycle 12's `xlabel` and `xlim` sort among the variables.
        let got = completions("x", &vars(&["x2", "x", "x10"]), &r);
        assert_eq!(got, ["x", "x10", "x2", "xlabel", "xlim"]);
    }

    #[test]
    fn a_variable_shadowing_a_builtin_appears_once() {
        let r = registry();
        let got = completions("disp", &vars(&["disp"]), &r);
        assert_eq!(got, ["disp"]);
    }

    #[test]
    fn an_empty_prefix_lists_every_name_once_sorted() {
        let r = registry();
        let got = completions("", &vars(&["sum", "aaa"]), &r);
        assert_eq!(got.len(), r.len() + 1);
        assert!(got.windows(2).all(|w| w[0] < w[1]));
        // Byte order, as `who` sorts: capitals first.
        assert_eq!(got[..3], ["Inf", "NaN", "aaa"]);
    }

    #[test]
    fn the_prefix_is_case_sensitive() {
        let r = registry();
        assert_eq!(completions("D", &vars(&["Data"]), &r), ["Data"]);
    }
}
