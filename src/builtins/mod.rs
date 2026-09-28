//! The builtin library.
//!
//! Every builtin is an ordinary function with one shape, registered by name in
//! a [`Registry`] that `Interp::new` builds once. `interp.rs` no longer knows
//! what any individual builtin does; it looks the name up, copies the function
//! pointer out of the map (the pointer is `Copy`, which is what lets `&self`
//! and `&mut self` coexist) and calls it.
//!
//! The `usize` in the signature is `nargout`, the number of values the caller
//! asked for. Nothing produces more than one value yet, but having it in the
//! signature now means cycle 03 can add multiple returns without rewriting
//! every builtin. `tic` and `toc` already read it: both change what they do
//! when the caller wants a value.
//!
//! The return is a `Vec`, and an empty one means "produced no value". That is
//! legal as a statement and is "Too many output arguments." in an expression.

pub mod args;
pub mod core;
pub mod linalg;
pub mod math;

use std::collections::HashMap;

use crate::interp::{Interp, R};
use crate::value::{Class, Matrix, Value};

/// The one shape every builtin has. The `usize` is `nargout`.
pub type BuiltinFn = fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>;

/// What the registry stores for a name.
pub struct Entry {
    pub f: BuiltinFn,
    /// One line, in the style of MATLAB's `help`. Nothing consumes these yet;
    /// `help` itself arrives in cycle 13.
    pub help: &'static str,
}

pub type Registry = HashMap<&'static str, Entry>;

/// Builds the whole library. Called once per interpreter, from `Interp::new`.
pub fn registry() -> Registry {
    let mut r = Registry::new();
    core::register(&mut r);
    math::register(&mut r);
    linalg::register(&mut r);
    r
}

fn add(r: &mut Registry, name: &'static str, f: BuiltinFn, help: &'static str) {
    let clash = r.insert(name, Entry { f, help });
    debug_assert!(clash.is_none(), "builtin '{name}' registered twice");
}

/// A builtin that produced one numeric matrix. The result is a double
/// whatever the argument was, which is MATLAB's rule for every numeric
/// builtin: `abs(true)`, `cumsum('abc')` and `sum(true, 3)` are doubles. It
/// is enforced here rather than trusted to each builtin, because several of
/// them hand back a clone of their argument on some path.
fn one_mat(m: Matrix) -> R<Vec<Value>> {
    Ok(vec![Value::Mat(m.with_class(Class::Double))])
}

/// A builtin that produced one matrix whose class it decided itself: the
/// rearrangements, which keep their argument's, and the predicates and
/// conversions, which name theirs.
fn one_as(m: Matrix) -> R<Vec<Value>> {
    Ok(vec![Value::Mat(m)])
}

/// A builtin that produced one value of any kind.
fn one(v: Value) -> R<Vec<Value>> {
    Ok(vec![v])
}

/// A builtin that produced no value, such as `disp`.
fn none() -> R<Vec<Value>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 79 builtins that existed before cycle 01, plus `tic` and `toc`,
    /// less `e`, which cycle 01c removed: MATLAB has no `e` constant. Cycle
    /// 02 added the eight class builtins: `class`, `islogical`, `ischar`,
    /// `isnumeric`, `isa`, `logical`, `char` and `double`.
    const EXPECTED: usize = 88;

    #[test]
    fn the_registry_holds_every_name_exactly_once() {
        let r = registry();
        assert_eq!(r.len(), EXPECTED, "registry size changed");
        // A spot check across all three files, including both halves of every
        // shared arm.
        for name in [
            "pi",
            "Inf",
            "inf",
            "NaN",
            "nan",
            "eps",
            "true",
            "false",
            "zeros",
            "ones",
            "eye",
            "rand",
            "linspace",
            "size",
            "numel",
            "length",
            "isempty",
            "isscalar",
            "isvector",
            "sum",
            "prod",
            "mean",
            "any",
            "all",
            "max",
            "min",
            "cumsum",
            "cumprod",
            "abs",
            "sqrt",
            "exp",
            "log",
            "log2",
            "log10",
            "sin",
            "cos",
            "tan",
            "asin",
            "acos",
            "atan",
            "sinh",
            "cosh",
            "tanh",
            "floor",
            "ceil",
            "round",
            "fix",
            "sign",
            "isnan",
            "isinf",
            "isfinite",
            "mod",
            "rem",
            "atan2",
            "hypot",
            "power",
            "transpose",
            "inv",
            "det",
            "trace",
            "diag",
            "norm",
            "dot",
            "reshape",
            "repmat",
            "fliplr",
            "flipud",
            "find",
            "sort",
            "disp",
            "fprintf",
            "sprintf",
            "num2str",
            "error",
            "clear",
            "clc",
            "who",
            "whos",
            "tic",
            "toc",
            "class",
            "islogical",
            "ischar",
            "isnumeric",
            "isa",
            "logical",
            "char",
            "double",
        ] {
            assert!(r.contains_key(name), "'{name}' is missing");
        }
        // `exp(1)` is the MATLAB spelling; `e` is an Octave extension, and an
        // ordinary name here, free to be a variable.
        assert!(!r.contains_key("e"), "'e' is back in the registry");
    }

    #[test]
    fn every_entry_is_callable_and_documented() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        for (name, e) in &registry() {
            assert!(!e.help.is_empty(), "'{name}' has no help line");
            // Calling with no arguments must return or fail, never panic.
            let _ = (e.f)(&mut it, &[], 0);
        }
    }
}
