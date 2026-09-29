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
pub mod cells;
pub mod complex;
pub mod core;
pub mod factor;
pub mod io;
pub mod linalg;
pub mod mat;
pub mod math;
pub mod numerics;
pub mod printf;
pub mod regex;
pub mod sets;
pub mod solvers;
pub mod strings;

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
    cells::register(&mut r);
    math::register(&mut r);
    linalg::register(&mut r);
    numerics::register(&mut r);
    sets::register(&mut r);
    solvers::register(&mut r);
    complex::register(&mut r);
    strings::register(&mut r);
    io::register(&mut r);
    r
}

/// The builtins that take a complex argument (cycle 10). Every other
/// builtin reads only the real parts of its arguments, so the registry
/// refuses a complex argument to it before it runs, with
/// `error::complex_argument`, rather than let it drop the imaginary part
/// in silence: `sort([1+2i 3])` is that refusal. On this list are the
/// functions of complex numbers themselves, the arithmetic that cycle 10
/// taught complex values (`sum`, `prod`, `mean`, `cumsum`, `abs`, `exp`,
/// `sin`, `cos`, `sqrt`, the logarithms, `asin`, `acos`, `power`), `fft`
/// and `ifft`, the output functions (`fprintf` and `sprintf` print only
/// the real parts, as MathWorks' `sprintf` page says), the shape and class
/// queries, which read no elements, and the functions that pass a value
/// through whole (the struct and cell functions, `deal`, `feval`,
/// `arrayfun`, `cellfun`), `isequal`, `double` and the plain `transpose`.
///
/// Only a matrix argument is judged here. A builtin that finds a complex
/// value inside a cell, or gets one back from a function it calls, judges
/// it itself: `cell2mat` concatenates it as a bracket would, the uniform
/// outputs of `cellfun` and `arrayfun` keep it, and a solver refuses it.
pub const TAKES_COMPLEX: &[&str] = &[
    // The functions of complex numbers, and the transforms.
    "real",
    "imag",
    "conj",
    "angle",
    "isreal",
    "fft",
    "ifft",
    // Arithmetic taught complex values in cycle 10.
    "abs",
    "sum",
    "prod",
    "mean",
    "cumsum",
    "exp",
    "sin",
    "cos",
    "sqrt",
    "log",
    "log2",
    "log10",
    "asin",
    "acos",
    "power",
    // Output: `disp` shows both parts, the printf family the real part.
    "disp",
    "fprintf",
    "sprintf",
    // Shape and class queries, which read no element.
    "size",
    "numel",
    "length",
    "isempty",
    "isscalar",
    "isvector",
    "class",
    "isa",
    "islogical",
    "ischar",
    "isnumeric",
    "iscell",
    "isstruct",
    // Values passed through whole.
    "struct",
    "getfield",
    "setfield",
    "deal",
    "feval",
    "arrayfun",
    "cellfun",
    "num2cell",
    "cell2mat",
    // Comparison, the class conversion to itself and the plain transpose.
    "isequal",
    "double",
    "transpose",
];

/// The refusal of a complex argument to a builtin not on
/// [`TAKES_COMPLEX`]; see there.
pub fn complex_gate(name: &str, args: &[Value]) -> R<()> {
    let complex = args
        .iter()
        .any(|v| matches!(v, Value::Mat(m) if m.is_complex()));
    if complex && !TAKES_COMPLEX.contains(&name) {
        return Err(crate::error::complex_argument(name));
    }
    Ok(())
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
    /// `isnumeric`, `isa`, `logical`, `char` and `double`. Cycle 04 added
    /// five: `rethrow`, `lasterr`, `warning`, `assert` and `isequal`. Cycle
    /// 05 added six: `nargin`, `nargout`, `exist`, `feval`, `addpath` and
    /// `rmpath`. Cycle 06 added three: `arrayfun`, `func2str` and
    /// `str2func`. Cycle 07 added thirteen: `cell`, `struct`,
    /// `fieldnames`, `isfield`, `rmfield`, `getfield`, `setfield`,
    /// `iscell`, `isstruct`, `cellfun`, `num2cell`, `cell2mat` and `deal`.
    /// Cycle 08 added fifteen: `lu`, `qr`, `chol`, `eig`, `svd`, `rank`,
    /// `pinv`, `null`, `orth`, `cond`, `kron`, `cross`, `triu`, `tril` and
    /// `magic`. Cycle 09 added thirty-three: the polynomials, samples,
    /// statistics, number theory, grids and counts of `numerics.rs`, the
    /// five set functions of `sets.rs` and the five solvers of `solvers.rs`.
    /// Cycle 10 added ten: `i`, `j`, `real`, `imag`, `conj`, `angle`,
    /// `isreal`, `complex`, `fft` and `ifft`. Cycle 11 added thirty-eight:
    /// the twenty-one string functions of `strings.rs` beside `num2str`,
    /// which moved there, and the sixteen file functions of `io.rs` beside
    /// `fprintf`, which moved there, and `input`.
    const EXPECTED: usize = 211;

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
            "rethrow",
            "lasterr",
            "warning",
            "assert",
            "isequal",
            "nargin",
            "nargout",
            "exist",
            "feval",
            "addpath",
            "rmpath",
            "arrayfun",
            "func2str",
            "str2func",
            "cell",
            "struct",
            "fieldnames",
            "isfield",
            "rmfield",
            "getfield",
            "setfield",
            "iscell",
            "isstruct",
            "cellfun",
            "num2cell",
            "cell2mat",
            "deal",
            "polyfit",
            "polyval",
            "roots",
            "conv",
            "deconv",
            "filter",
            "interp1",
            "trapz",
            "cumtrapz",
            "diff",
            "std",
            "var",
            "median",
            "mode",
            "factorial",
            "nchoosek",
            "primes",
            "isprime",
            "gcd",
            "lcm",
            "logspace",
            "meshgrid",
            "histc",
            "unique",
            "ismember",
            "setdiff",
            "intersect",
            "union",
            "fzero",
            "fminsearch",
            "integral",
            "ode45",
            "odeset",
            "i",
            "j",
            "real",
            "imag",
            "conj",
            "angle",
            "isreal",
            "complex",
            "fft",
            "ifft",
            "strcat",
            "strsplit",
            "strjoin",
            "strrep",
            "strtrim",
            "upper",
            "lower",
            "strcmp",
            "strcmpi",
            "strncmp",
            "strncmpi",
            "strfind",
            "strtok",
            "int2str",
            "str2double",
            "str2num",
            "mat2str",
            "isspace",
            "isletter",
            "blanks",
            "regexp",
            "regexprep",
            "input",
            "fopen",
            "fclose",
            "fgetl",
            "fgets",
            "fread",
            "fwrite",
            "feof",
            "fileread",
            "readmatrix",
            "writematrix",
            "csvread",
            "csvwrite",
            "delete",
            "save",
            "load",
        ] {
            assert!(r.contains_key(name), "'{name}' is missing");
        }
        // `exp(1)` is the MATLAB spelling; `e` is an Octave extension, and an
        // ordinary name here, free to be a variable.
        assert!(!r.contains_key("e"), "'e' is back in the registry");
        // Every name the complex gate lets through is a builtin.
        for name in TAKES_COMPLEX {
            assert!(r.contains_key(name), "'{name}' is on TAKES_COMPLEX only");
        }
    }

    /// Cycle 10: a complex argument reaches only the builtins that take one.
    #[test]
    fn the_gate_refuses_a_complex_argument_to_every_other_builtin() {
        let z = Value::Mat(Matrix::complex_parts(1, 2, vec![1.0, 3.0], vec![2.0, 0.0]));
        let e = complex_gate("sort", std::slice::from_ref(&z))
            .unwrap_err()
            .msg;
        assert_eq!(e, "Complex values are not supported by 'sort'.");
        assert!(complex_gate("abs", std::slice::from_ref(&z)).is_ok());
        assert!(complex_gate("sort", &[Value::Mat(Matrix::scalar(1.0))]).is_ok());
        // A builtin with its own complex arithmetic is on the list; one that
        // reads only real parts, such as `max` or `mod`, is not.
        for name in [
            "max", "min", "floor", "mod", "fzero", "eig", "roots", "complex",
        ] {
            assert!(!TAKES_COMPLEX.contains(&name), "{name}");
        }
    }

    #[test]
    fn every_entry_is_callable_and_documented() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        // A bare `save` writes `matlab.mat` and a bare `load` reads it (cycle
        // 11), so the calls run in a folder of their own, not the crate's.
        let dir = std::env::temp_dir().join(format!("splatcrab-registry-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        it.cwd = dir.clone();
        // With no terminal, a bare `input` is refused rather than waiting.
        it.input = crate::interp::InputSource::Refused;
        for (name, e) in &registry() {
            assert!(!e.help.is_empty(), "'{name}' has no help line");
            // Calling with no arguments must return or fail, never panic.
            let _ = (e.f)(&mut it, &[], 0);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
