//! The interpreter's error type, and every message text it can produce.
//!
//! Two jobs in one file, on purpose. `MError` carries a message and the source
//! line it came from, and the constructors below are the only place a message
//! is spelled out, so the same wording can never drift apart between two call
//! sites. `Unrecognized function or variable '{}'.` is raised from two places in
//! `interp.rs` and `Dimensions of arrays being concatenated are not
//! consistent.` from two more; each is now one function.
//!
//! Adding a field later is additive: cycle 05 gives `MError` a `stack` and the
//! `  in <fn> (line N)` trace, which needs no change at any call site because
//! every one of them goes through a constructor here.

use std::fmt;

use crate::lexer::Token;

/// An interpreter error: what went wrong, and where, once a line is known.
///
/// `line` is `None` until the error passes the statement that raised it; see
/// [`MError::at`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MError {
    pub msg: String,
    pub line: Option<u32>,
}

/// Every fallible path in the interpreter returns this.
pub type R<T> = Result<T, MError>;

impl MError {
    /// An error with no line yet.
    pub fn new(msg: impl Into<String>) -> MError {
        MError {
            msg: msg.into(),
            line: None,
        }
    }

    /// Records `line`, unless a line is already known.
    ///
    /// The first statement to see the error wins, which is the innermost one:
    /// an error inside a `for` body reports the body's line rather than the
    /// line of the `for` that is still unwinding around it.
    pub fn at(mut self, line: u32) -> MError {
        if self.line.is_none() {
            self.line = Some(line);
        }
        self
    }
}

/// `Line N: <msg>` when the line is known, and the bare message otherwise.
/// Script mode prints this after `Error: `; the REPL prints `msg` alone,
/// since there is only ever one line there.
impl fmt::Display for MError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(n) => write!(f, "Line {}: {}", n, self.msg),
            None => f.write_str(&self.msg),
        }
    }
}

/// Lets a `?` on a `Result<_, String>` keep compiling while a site is
/// converted, and lets `error('...')` raise a message the user composed.
impl From<String> for MError {
    fn from(msg: String) -> MError {
        MError::new(msg)
    }
}

impl From<&str> for MError {
    fn from(msg: &str) -> MError {
        MError::new(msg)
    }
}

/// `bail!(e)` is `return Err(e.into())`, the way `?` would leave a function.
///
/// The argument is a constructor from this module rather than a format string,
/// which is what keeps every message text on this side of the crate.
#[macro_export]
macro_rules! bail {
    ($e:expr $(,)?) => {
        return ::core::result::Result::Err(::core::convert::From::from($e))
    };
}

// ---- lexer -----------------------------------------------------------

pub fn invalid_number(text: &str) -> MError {
    MError::new(format!("invalid number '{}'", text))
}

pub fn unterminated_string() -> MError {
    MError::new("unterminated string")
}

pub fn unexpected_char(c: char) -> MError {
    MError::new(format!("unexpected character '{}'", c))
}

// ---- parser ----------------------------------------------------------

// Every one of these renders the offending token with `{}`, not `{:?}`.
// `Token`'s `Display` is the human spelling (`';'`, `')'`, `end of input`);
// its `Debug` is the Rust variant name, which is what used to reach the user
// as `unexpected Semi in expression`.

pub fn expected_token(want: &Token, found: &Token) -> MError {
    MError::new(format!("expected {} but found {}", want, found))
}

pub fn unexpected_token(found: &Token) -> MError {
    MError::new(format!("unexpected {}", found))
}

pub fn unexpected_in_expression(found: &Token) -> MError {
    MError::new(format!("unexpected {} in expression", found))
}

pub fn expected_end_of_if(found: &Token) -> MError {
    MError::new(format!("expected 'end' to close 'if', found {}", found))
}

pub fn expected_loop_variable(found: &Token) -> MError {
    MError::new(format!(
        "expected loop variable after 'for', found {}",
        found
    ))
}

pub fn block_with_no_opener(found: &Token) -> MError {
    MError::new(format!("unexpected {} with no matching block", found))
}

pub fn invalid_assignment_target() -> MError {
    MError::new("invalid assignment target")
}

pub fn unterminated_matrix() -> MError {
    MError::new("unterminated matrix literal: missing ']'")
}

/// An expression or a block nested past [`crate::parser::MAX_DEPTH`].
///
/// The parser and the evaluator raise the same message from the same limit:
/// the two recursions are the same shape, so a program the parser accepts is
/// one the evaluator can walk. Without it, about 96,000 nested parentheses
/// exhausted even the 256 MB interpreter stack and aborted the process with
/// exit 134 (QA D4), which no `Result` can catch.
pub fn nesting_too_deep(limit: usize) -> MError {
    MError::new(format!(
        "Nesting is too deep. The maximum nesting depth is {}.",
        limit
    ))
}

/// A block left open when the input ran out: the REPL's end of file, which
/// used to discard the half-typed block and exit 0 (QA D36).
///
/// Lower case like the other messages the parse raises, and worded to cover
/// both openers the REPL keeps reading for, a block and a bracket.
pub fn unterminated_block() -> MError {
    MError::new("unterminated block: the input ended before its 'end' or closing bracket.")
}

// ---- evaluator -------------------------------------------------------

pub fn end_outside_index() -> MError {
    MError::new("'end' is only valid inside an index expression.")
}

/// `break` with no enclosing loop (QA D8). It used to unwind out of the whole
/// script, so the statements after it never ran and the process still exited
/// 0. Raised when the statement runs, not when it parses, so everything the
/// script printed before it is still printed.
pub fn break_outside_loop() -> MError {
    MError::new("'break' is only valid inside a loop.")
}

/// `continue` with no enclosing loop; see [`break_outside_loop`].
pub fn continue_outside_loop() -> MError {
    MError::new("'continue' is only valid inside a loop.")
}

/// MATLAB's wording, for `if NaN`, `NaN & 1` and `~NaN` (QA D5). A `NaN` is
/// neither true nor false, and taking it as true is silent and wrong.
pub fn nan_to_logical() -> MError {
    MError::new("NaN's cannot be converted to logicals.")
}

/// MATLAB's wording, for `[1 1] && 1` and `[] || 1`. The short-circuit
/// operators need one value to branch on, so an array or an empty is an
/// error rather than "all non-zero".
pub fn logical_scalar_operand() -> MError {
    MError::new(
        "Operands to the logical AND (&&) and OR (||) operators must be \
         convertible to logical scalar values.",
    )
}

pub fn colon_outside_index() -> MError {
    MError::new("':' on its own is only valid inside an index expression.")
}

/// `what` names the thing that had to be a scalar, such as `range start`.
pub fn not_a_scalar(what: &str) -> MError {
    MError::new(format!("{} must be a scalar.", what))
}

pub fn matrix_exponent() -> MError {
    MError::new("Matrix exponent is not supported; use '.^' for element-wise power.")
}

pub fn nonsquare_power() -> MError {
    MError::new("Matrix must be square for '^'. Use '.^' for element-wise power.")
}

pub fn fractional_matrix_power() -> MError {
    MError::new("Only integer matrix powers are supported.")
}

/// `x()`, an index with no subscripts at all. Since cycle 03 accepted
/// trailing singleton subscripts (QA D22), this is the one form that reaches
/// it; a third subscript that would need an N-D array is
/// [`nd_unsupported`] instead.
pub fn indexing_rank() -> MError {
    MError::new("Only 1-D and 2-D indexing is supported.")
}

/// MATLAB's wording. The ending names logical values since cycle 03, when
/// a logical index became a mask rather than a refusal.
pub fn index_not_positive_integer(pos: usize) -> MError {
    MError::new(format!(
        "Index in position {} is invalid. Array indices must be positive integers or logical values.",
        pos
    ))
}

pub fn index_exceeds_numel(limit: usize) -> MError {
    MError::new(format!(
        "Index exceeds the number of array elements. Index must not exceed {}.",
        limit
    ))
}

pub fn index_exceeds_bound(pos: usize, limit: usize) -> MError {
    MError::new(format!(
        "Index in position {} exceeds array bounds. Index must not exceed {}.",
        pos, limit
    ))
}

/// MATLAB's wording: `A(1, 2) = []`, a deletion with two subscripts that
/// each select part of their dimension. Only a whole row or column set can
/// be removed from a matrix.
pub fn null_assignment_indices() -> MError {
    MError::new("A null assignment can have only one non-colon index.")
}

/// MATLAB's wording: `x{1}` where `x` is a matrix. It stays right for a
/// matrix after cycle 07 gives cells a brace index of their own.
pub fn brace_indexing_unsupported() -> MError {
    MError::new("Brace indexing is not supported for variables of this type.")
}

/// MATLAB's wording: `x.a` or `x.(n)` where `x` is a matrix. It stays right
/// for a matrix after cycle 07 gives structs their fields.
pub fn dot_indexing_unsupported() -> MError {
    MError::new("Dot indexing is not supported for variables of this type.")
}

/// MATLAB's wording: `[a, b] = 5`, more targets than a value that is not a
/// call can supply.
pub fn insufficient_outputs() -> MError {
    MError::new(
        "Insufficient number of outputs from right hand side of equal sign to satisfy assignment.",
    )
}

pub fn assignment_size(lhs: usize, rhs: usize) -> MError {
    MError::new(format!(
        "Unable to perform assignment because the left side has {} elements and the right side has {}.",
        lhs, rhs
    ))
}

pub fn ambiguous_growth() -> MError {
    MError::new("Attempt to grow array along ambiguous dimension.")
}

/// MATLAB R2020a and later. The wording before it was `Undefined function or
/// variable`, which is what this printed until cycle 01e settled QA D33; see
/// the message-text policy in that cycle's Design notes.
pub fn undefined(name: &str) -> MError {
    MError::new(format!("Unrecognized function or variable '{}'.", name))
}

pub fn too_many_outputs() -> MError {
    MError::new("Too many output arguments.")
}

pub fn concat_dims() -> MError {
    MError::new("Dimensions of arrays being concatenated are not consistent.")
}

/// Output could not be written. Nothing user-facing produces this; it is the
/// sink failing underneath `Interp::emit`.
pub fn output(e: std::io::Error) -> MError {
    MError::new(e.to_string())
}

// ---- matrix arithmetic -----------------------------------------------

pub fn operator_dims(op: &str, ar: usize, ac: usize, br: usize, bc: usize) -> MError {
    MError::new(format!(
        "Arrays have incompatible sizes for operator '{}' ({}x{} vs {}x{}).",
        op, ar, ac, br, bc
    ))
}

pub fn matmul_dims(ar: usize, ac: usize, br: usize, bc: usize) -> MError {
    MError::new(format!(
        "Incorrect dimensions for matrix multiplication ({}x{} * {}x{}). \
         Use '.*' for element-wise multiplication.",
        ar, ac, br, bc
    ))
}

pub fn nonsquare_system() -> MError {
    MError::new("Only square systems are supported by '\\' and '/' for now (no least squares yet).")
}

pub fn solve_dims(ar: usize, ac: usize, br: usize, bc: usize) -> MError {
    MError::new(format!(
        "Matrix dimensions must agree for '\\' ({}x{} \\ {}x{}).",
        ar, ac, br, bc
    ))
}

pub fn singular() -> MError {
    MError::new("Matrix is singular to working precision.")
}

pub fn nonsquare_inverse() -> MError {
    MError::new("Matrix must be square to invert.")
}

// ---- results that would be complex -----------------------------------
//
// Cycle 01d turns each of these from a silent `NaN` into a refusal; cycle 10
// replaces the refusal with the complex value itself. Every one of them opens
// with the same sentence, so a golden case can match on it alone.

/// `sqrt(-4)`, `log(-1)`, `log2(-8)`, `log10(-10)`.
pub fn complex_negative(name: &str) -> MError {
    MError::new(format!(
        "Complex results are not supported. '{}' of a negative number is complex.",
        name
    ))
}

/// `asin(2)`, `acos(-2)`.
pub fn complex_outside_unit(name: &str) -> MError {
    MError::new(format!(
        "Complex results are not supported. '{}' of a value outside [-1, 1] is complex.",
        name
    ))
}

/// `(-8)^(1/3)`, `(-8).^(1/3)`, `power(-2, 0.5)`.
pub fn complex_power() -> MError {
    MError::new(
        "Complex results are not supported. \
         A negative number raised to a fractional power is complex.",
    )
}

pub fn nonsquare_determinant() -> MError {
    MError::new("Matrix must be square to compute a determinant.")
}

// ---- builtin arguments -----------------------------------------------

pub fn not_enough_args(name: &str) -> MError {
    MError::new(format!("Not enough input arguments for '{}'.", name))
}

pub fn too_many_args() -> MError {
    MError::new("Too many input arguments.")
}

pub fn arg_not_a_scalar(pos: usize, name: &str) -> MError {
    MError::new(format!("Argument {} to '{}' must be a scalar.", pos, name))
}

pub fn arg_not_a_string(pos: usize, name: &str) -> MError {
    MError::new(format!(
        "Argument {} to '{}' must be a character vector.",
        pos, name
    ))
}

pub fn bad_dim_arg(name: &str) -> MError {
    MError::new(format!(
        "Dimension argument to '{}' must be a positive integer scalar.",
        name
    ))
}

pub fn bad_size_arg(name: &str) -> MError {
    MError::new(format!(
        "Size arguments to '{}' must be non-negative integers.",
        name
    ))
}

/// The one message for a shape that would not fit in memory. `range` in
/// `interp.rs` reaches it through `args::check_shape` rather than inventing a
/// second wording for the `:` operator.
///
/// The two dimensions arrive already rendered, because a requested size can
/// be past `usize` (`zeros(1e300)`) and `args::fmt_dim` decides how such a
/// size is printed.
pub fn size_overflow(rows: &str, cols: &str) -> MError {
    MError::new(format!(
        "Requested {}x{} array exceeds the maximum array size.",
        rows, cols
    ))
}

/// A third or later size other than `1`: `zeros(2, 3, 4)`.
pub fn nd_unsupported() -> MError {
    MError::new("N-D arrays are not supported.")
}

/// A single size argument that is a column or a matrix: `zeros([2; 3])`.
pub fn size_vector_not_row(name: &str) -> MError {
    MError::new(format!("Size vector for '{}' must be a row vector.", name))
}

// ---- builtins --------------------------------------------------------

/// `error('...')` with a message the user composed.
pub fn raised(msg: String) -> MError {
    MError::new(msg)
}

/// `error(x)` with no format string: MATLAB's bare fallback text.
pub fn raised_default() -> MError {
    MError::new("error")
}

pub fn format_not_a_string() -> MError {
    MError::new("The first argument must be a format string.")
}

pub fn invalid_format_spec() -> MError {
    MError::new("Invalid format specifier.")
}

pub fn unsupported_format_spec(c: char) -> MError {
    MError::new(format!("Unsupported format specifier '%{}'.", c))
}

/// A field that is absurd rather than merely large: `%.65536f` used to panic
/// inside Rust's formatter and `%2147483647d` used to build a two-gigabyte
/// pad. Both numbers are judged before anything is formatted or allocated.
pub fn format_field_too_large(limit: usize) -> MError {
    MError::new(format!(
        "The width or precision in a format specifier must be at most {}.",
        limit
    ))
}

pub fn nonsquare_trace() -> MError {
    MError::new("Matrix must be square for 'trace'.")
}

pub fn norm_vectors_only() -> MError {
    MError::new("'norm' currently supports vectors only.")
}

pub fn sort_vectors_only() -> MError {
    MError::new("'sort' currently supports vectors only.")
}

pub fn dot_size_mismatch() -> MError {
    MError::new("A and B must be the same size for 'dot'.")
}

pub fn reshape_numel(have: usize, rows: usize, cols: usize) -> MError {
    MError::new(format!(
        "To reshape the number of elements must not change ({} vs {}x{}).",
        have, rows, cols
    ))
}

/// MATLAB's wording. `known` is rendered by `args::fmt_dim`, since the
/// product of the known sizes can be past `usize`.
pub fn reshape_not_divisible(known: &str, total: usize) -> MError {
    MError::new(format!(
        "Product of known dimensions, {}, not divisible into total number of elements, {}.",
        known, total
    ))
}

/// MATLAB's wording.
pub fn reshape_two_unknowns() -> MError {
    MError::new("Size can only have one unknown dimension.")
}

pub fn sort_direction() -> MError {
    MError::new("Sort direction for 'sort' must be 'ascend' or 'descend'.")
}

pub fn find_count() -> MError {
    MError::new("Number of elements for 'find' must be a positive integer scalar.")
}

pub fn find_direction() -> MError {
    MError::new("Search direction for 'find' must be 'first' or 'last'.")
}

pub fn norm_type() -> MError {
    MError::new("Norm type for 'norm' must be a positive real scalar, Inf, -Inf or 'fro'.")
}

/// MATLAB's wording.
pub fn diag_offset() -> MError {
    MError::new("K-th diagonal input must be an integer scalar.")
}

pub fn num2str_precision() -> MError {
    MError::new("Precision for 'num2str' must be a positive integer.")
}

pub fn round_digits() -> MError {
    MError::new("Number of digits for 'round' must be an integer scalar.")
}

pub fn round_significant() -> MError {
    MError::new("Number of significant digits for 'round' must be a positive integer scalar.")
}

pub fn round_type() -> MError {
    MError::new("Rounding type for 'round' must be 'decimals' or 'significant'.")
}

/// MATLAB's wording: a bare `toc` with no earlier bare `tic`.
pub fn toc_without_tic() -> MError {
    MError::new(
        "You must call TIC without an output argument before calling TOC without an input argument.",
    )
}

pub fn eps_class() -> MError {
    MError::new("Only 'double' is supported as a class name for 'eps'.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_recorded_once_and_the_innermost_one_wins() {
        let e = undefined("y");
        assert_eq!(e.line, None);
        let e = e.at(4);
        assert_eq!(e.line, Some(4));
        // A statement further out must not overwrite it, which is what makes
        // an error inside a `for` body report the body's line.
        assert_eq!(e.at(1).line, Some(4));
    }

    #[test]
    fn display_adds_the_line_prefix_only_when_one_is_known() {
        assert_eq!(
            undefined("y").to_string(),
            "Unrecognized function or variable 'y'."
        );
        assert_eq!(
            undefined("y").at(3).to_string(),
            "Line 3: Unrecognized function or variable 'y'."
        );
    }

    #[test]
    fn a_string_converts_so_error_can_raise_what_the_user_composed() {
        let e: MError = "boom".to_string().into();
        assert_eq!(e.msg, "boom");
        assert_eq!(e.line, None);
    }

    #[test]
    fn bail_returns_the_error() {
        fn f(fail: bool) -> R<u8> {
            if fail {
                bail!(too_many_outputs());
            }
            Ok(1)
        }
        assert_eq!(f(false).unwrap(), 1);
        assert_eq!(f(true).unwrap_err(), too_many_outputs());
    }

    /// Acceptance test 15: every message the interpreter can raise is built
    /// here. A message formatted at the raising site is how the same wording
    /// ends up spelled two ways, so the check is mechanical: outside this
    /// file, nothing that produces an error may hand it a literal or a
    /// `format!`.
    #[test]
    fn no_source_file_builds_an_error_message_of_its_own() {
        const FILES: [(&str, &str); 8] = [
            ("lexer.rs", include_str!("lexer.rs")),
            ("parser.rs", include_str!("parser.rs")),
            ("interp.rs", include_str!("interp.rs")),
            ("value.rs", include_str!("value.rs")),
            ("builtins/args.rs", include_str!("builtins/args.rs")),
            ("builtins/core.rs", include_str!("builtins/core.rs")),
            ("builtins/math.rs", include_str!("builtins/math.rs")),
            ("builtins/linalg.rs", include_str!("builtins/linalg.rs")),
        ];
        // Anything whose argument becomes the error value.
        const MAKERS: [&str; 5] = ["Err(", "ok_or(", "ok_or_else(||", "map_err(|e|", "bail!("];

        let mut checked = 0;
        for (name, src) in FILES {
            // Unit tests legitimately write message text: they assert it.
            let body = src.split("#[cfg(test)]").next().expect("non-test body");
            for maker in MAKERS {
                let mut rest = body;
                while let Some(k) = rest.find(maker) {
                    rest = &rest[k + maker.len()..];
                    let tail = rest.trim_start();
                    assert!(
                        !tail.starts_with('"') && !tail.starts_with("format!"),
                        "{name}: `{maker}` builds its own message text; \
                         add a constructor to error.rs instead:\n  {}",
                        tail.lines().next().unwrap_or("")
                    );
                    checked += 1;
                }
            }
        }
        // The scan is worthless if it matched nothing.
        assert!(checked > 40, "only {checked} error sites found");
    }
}
