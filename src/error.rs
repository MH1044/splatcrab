//! The interpreter's error type, and every message text it can produce.
//!
//! Two jobs in one file, on purpose. `MError` carries a message and the source
//! line it came from, and the constructors below are the only place a message
//! is spelled out, so the same wording can never drift apart between two call
//! sites. `Unrecognized function or variable '{}'.` is raised from two places in
//! `interp.rs` and `Dimensions of arrays being concatenated are not
//! consistent.` from two more; each is now one function.
//!
//! Adding a field is additive: cycle 05 gave `MError` its `stack` and the
//! `  in <fn> (line N)` trace, which needed no change at any call site because
//! every one of them goes through a constructor here.

use std::fmt;

use crate::lexer::Token;

/// An interpreter error: what went wrong, and where, once a line is known.
///
/// `line` is `None` until the error passes the statement that raised it; see
/// [`MError::at`]. Once the error has left a user function it is the line of
/// the statement in the *calling* code, so that at the top it is always a
/// line of the code that was run, never of another file; the lines inside
/// the functions it passed through are in [`MError::stack`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MError {
    pub msg: String,
    pub line: Option<u32>,
    /// The identifier and the stack, which most errors never have: `None`
    /// until one of them is set.
    ///
    /// Boxed together, because an `MError` rides in every `R<Value>` the
    /// evaluator returns, and every byte it has is paid again in every frame
    /// of a deep recursion. Cycle 05's stack, added as a bare `Vec`, cost
    /// the 10,000-level expression about a fifth more stack; boxing it with
    /// the identifier made the error smaller than it was before either.
    extra: Option<Box<Extra>>,
}

/// The rarely present parts of an [`MError`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Extra {
    /// The identifier `error('MyPkg:myid', ...)` attached, which a `catch`
    /// reads back as `e.identifier`. Empty for every error the interpreter
    /// raises itself, and for `error` called without one (cycle 04).
    identifier: String,
    /// The user functions (and path scripts) the error unwound out of,
    /// innermost first, each with the line it was on there (cycle 05).
    stack: Vec<StackEntry>,
}

/// One frame an error unwound out of: the function's name, the line of the
/// statement that failed in it, and since cycle 07 the file it came from,
/// empty when the function is local to the code that was run, whose file
/// the interpreter is not told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackEntry {
    pub name: String,
    pub line: Option<u32>,
    pub file: String,
}

/// Every fallible path in the interpreter returns this.
pub type R<T> = Result<T, MError>;

impl MError {
    /// An error with no line yet.
    pub fn new(msg: impl Into<String>) -> MError {
        MError {
            msg: msg.into(),
            line: None,
            extra: None,
        }
    }

    /// The identifier `error('MyPkg:myid', ...)` attached; empty for every
    /// error the interpreter raises itself, and for `error` called without
    /// one (cycle 04).
    pub fn identifier(&self) -> &str {
        self.extra.as_ref().map_or("", |x| x.identifier.as_str())
    }

    /// The user functions (and path scripts) the error unwound out of,
    /// innermost first, each with the line it was on there; empty for an
    /// error raised in the code that was run itself (cycle 05).
    pub fn stack(&self) -> &[StackEntry] {
        self.extra.as_ref().map_or(&[], |x| x.stack.as_slice())
    }

    /// The same error once it has unwound out of the user function `name`:
    /// the line it carried, which is a line of that function's file, moves
    /// into a new outermost [`StackEntry`], and `line` is cleared so that
    /// the calling statement records its own.
    pub fn leaving(self, name: &str) -> MError {
        self.leaving_file(name, "")
    }

    /// [`leaving`](MError::leaving), recording the file the function came
    /// from, which `e.stack` reports (cycle 07).
    pub fn leaving_file(mut self, name: &str, file: &str) -> MError {
        let entry = StackEntry {
            name: name.to_string(),
            line: self.line.take(),
            file: file.to_string(),
        };
        self.extra
            .get_or_insert_with(Box::default)
            .stack
            .push(entry);
        self
    }

    /// The trace printed after the message of an uncaught error: one
    /// `  in <fn> (line N)` line per frame, innermost first, each ending in a
    /// newline. Empty when the error never left the code that was run.
    pub fn trace(&self) -> String {
        let mut s = String::new();
        for entry in self.stack() {
            match entry.line {
                Some(n) => s.push_str(&format!("  in {} (line {})\n", entry.name, n)),
                None => s.push_str(&format!("  in {}\n", entry.name)),
            }
        }
        s
    }

    /// The same error carrying `identifier`, as `error('id:x', fmt, ...)`
    /// raises it.
    pub fn with_identifier(mut self, identifier: impl Into<String>) -> MError {
        let identifier = identifier.into();
        if !identifier.is_empty() || self.extra.is_some() {
            self.extra.get_or_insert_with(Box::default).identifier = identifier;
        }
        self
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
    expected_end_of("if", found)
}

/// A block keyword that its statement does not allow where it was found:
/// `if` without its `end`, or anything but `case`, `otherwise` or `end`
/// between the arms of a `switch` (cycle 04).
pub fn expected_end_of(keyword: &str, found: &Token) -> MError {
    MError::new(format!(
        "expected 'end' to close '{}', found {}",
        keyword, found
    ))
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

/// A function handle as an element of a bracket, `[@(x) x+1]` in the source
/// or `[f 1]` at run time (cycle 06): a handle is one function, never an
/// array of them. The wording is MATLAB's as recalled, for concatenating
/// handles, not confirmed against a MathWorks source.
pub fn handle_concatenation() -> MError {
    MError::new("Nonscalar arrays of function handles are not allowed; use cell arrays instead.")
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

// ---- cells and structs (cycle 07) ---------------------------------------

/// MATLAB's wording, recorded by the spec: a cs-list, `c{:}` of a cell with
/// two elements or `p.name` of a 1x2 struct array, where one value is
/// needed. SplatCrab says the same for a cs-list of none, `c{:}` of `{}`.
pub fn cs_list_count(n: usize) -> MError {
    MError::new(format!(
        "Expected one output from a curly brace or dot indexing expression, but there were {} results.",
        n
    ))
}

/// MATLAB's wording, recorded by the spec: `x = 1; x.a = 2`, a field
/// assigned into a value that is not a struct.
pub fn dot_assign_unsupported() -> MError {
    MError::new(
        "Unable to perform assignment because dot indexing is not supported for variables of this type.",
    )
}

/// `x = 1; x{1} = 2`: the brace counterpart of [`dot_assign_unsupported`],
/// in the same form; not confirmed against a MathWorks source.
pub fn brace_assign_unsupported() -> MError {
    MError::new(
        "Unable to perform assignment because brace indexing is not supported for variables of this type.",
    )
}

/// MATLAB R2020a's wording, recorded by the spec: a binary operator with an
/// operand that is not an array, `c + 1` of a cell, `f + 1` of a handle,
/// `s * 2` of a struct or `e + 1` of an `MException`. `class` is the first
/// such operand's.
pub fn operator_unsupported(op: &str, class: &str) -> MError {
    MError::new(format!(
        "Operator '{}' is not supported for operands of type '{}'.",
        op, class
    ))
}

/// `s.b` of a struct with no field `b`. MATLAB's wording as recalled, not
/// confirmed against a MathWorks source.
pub fn no_such_field(name: &str) -> MError {
    MError::new(format!("Unrecognized field name \"{}\".", name))
}

/// A value stored where it cannot go: `c(2) = 5` of a cell, `x(2) = {1}`
/// of a non-empty matrix, `[s 1]`. MATLAB's form as recalled.
pub fn conversion(to: &str, from: &str) -> MError {
    MError::new(format!(
        "Conversion to {} from {} is not possible.",
        to, from
    ))
}

/// `p(2) = q` where `q` has other fields than `p`. MATLAB's wording as
/// recalled.
pub fn dissimilar_structs() -> MError {
    MError::new("Subscripted assignment between dissimilar structures.")
}

/// `[s1 s2]` where the two have other fields. SplatCrab's wording.
pub fn struct_concat_fields() -> MError {
    MError::new("Structures being concatenated must have the same field names.")
}

/// `p.name = 'x'` where `p` is a struct array of more than one element.
/// MATLAB's wording as recalled.
pub fn scalar_struct_required() -> MError {
    MError::new("Scalar structure required for this assignment.")
}

/// A field name that is not a MATLAB identifier: `s.('1a') = 1`,
/// `struct('a b', 1)`, `setfield(s, '', 1)`. MATLAB's form as recalled.
pub fn invalid_field_name(name: &str) -> MError {
    MError::new(format!("Invalid field name: '{}'.", name))
}

/// `s.(5)`: a dynamic field name that is not text. SplatCrab's wording.
pub fn dynamic_field_not_text() -> MError {
    MError::new("A dynamic field name must be a character vector.")
}

/// `struct('a')`: a field name with no value. SplatCrab's wording.
pub fn struct_pairs() -> MError {
    MError::new("Field names and values to 'struct' must come in pairs.")
}

/// `struct('a', {1 2}, 'b', {1 2 3})`: cell values that make struct arrays
/// of two sizes. SplatCrab's wording.
pub fn struct_cell_dims() -> MError {
    MError::new("The cell values given to 'struct' must all have one size, or be 1x1.")
}

/// `cell2mat({{1}})`: a cell holding something that is not an array.
/// SplatCrab's wording.
pub fn cell2mat_contents() -> MError {
    MError::new("cell2mat does not support cells holding cells, structs, handles or MExceptions.")
}

/// `[a, b] = deal(1, 2, 3)`: neither one input nor one per output.
/// MATLAB's wording as recalled.
pub fn deal_count() -> MError {
    MError::new("The number of outputs should match the number of inputs.")
}

/// `cellfun(f, 5)`: an input that must be a cell. SplatCrab's wording, in
/// the form of [`arg_not_a_string`].
pub fn arg_not_a_cell(pos: usize, name: &str) -> MError {
    MError::new(format!(
        "Argument {} to '{}' must be a cell array.",
        pos, name
    ))
}

/// `fieldnames(5)`: an input that must be a struct. SplatCrab's wording,
/// in the form of [`arg_not_a_cell`].
pub fn arg_not_a_struct(pos: usize, name: &str) -> MError {
    MError::new(format!("Argument {} to '{}' must be a struct.", pos, name))
}

/// A function that set `varargout` to something other than a cell and was
/// asked for the outputs it holds. SplatCrab's wording.
pub fn varargout_not_a_cell() -> MError {
    MError::new("The variable varargout must be a cell array.")
}

/// A function with `varargout` asked for more outputs than it put in
/// `varargout`: [`output_not_assigned`] naming the missing element.
pub fn varargout_not_assigned(k: usize, func: &str) -> MError {
    output_not_assigned(&format!("varargout{{{}}}", k), func)
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

// ---- functions (cycle 05) ----------------------------------------------

/// MATLAB's wording: a statement after a local function in a script. The
/// first sentence is confirmed by the title of a MathWorks Answers thread.
pub fn functions_at_end() -> MError {
    MError::new("Function definitions in a script must appear at the end of the file.")
}

/// MATLAB's wording: a `function` block typed at the REPL, sent in a
/// protocol `eval` or from the browser page, or opened inside another block.
/// Confirmed, like [`functions_at_end`], by a MathWorks Answers title.
pub fn function_not_supported_here() -> MError {
    MError::new("Function definitions are not supported in this context.")
}

/// MATLAB's wording: a user function asked for an output it never assigned.
/// `name` is the first such output, `func` the function.
pub fn output_not_assigned(name: &str, func: &str) -> MError {
    MError::new(format!(
        "Output argument \"{}\" (and maybe others) not assigned during call to \"{}\".",
        name, func
    ))
}

/// MATLAB's wording: one call more than the recursion limit allows. It is a
/// clean error rather than the stack overflow it would otherwise become.
pub fn recursion_limit(limit: usize) -> MError {
    MError::new(format!("Maximum recursion limit of {} reached.", limit))
}

/// `nargin` or `nargout` outside every function, where there is no call for
/// them to describe. The wording is MATLAB's as recalled, not confirmed
/// against a MathWorks source, so its case pins it as SplatCrab's own.
pub fn nargin_outside_function() -> MError {
    MError::new("You can only call nargin/nargout from within a MATLAB function.")
}

/// A function file on the path that could not be read. `main.rs` words the
/// same failure for the script it is given the same way.
pub fn cannot_read(path: &str, e: &std::io::Error) -> MError {
    MError::new(format!("Cannot read {}: {}", path, e))
}

/// The warning `addpath` gives for a folder that does not exist, which it
/// then leaves off the path (MATLAB's first sentence).
pub fn addpath_not_a_folder(dir: &str) -> String {
    format!("Name is nonexistent or not a directory: {}", dir)
}

/// The warning `rmpath` gives for a folder that is not on the path.
pub fn rmpath_not_on_path(dir: &str) -> String {
    format!("\"{}\" not found in path.", dir)
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

pub fn solve_dims(ar: usize, ac: usize, br: usize, bc: usize) -> MError {
    MError::new(format!(
        "Matrix dimensions must agree for '\\' ({}x{} \\ {}x{}).",
        ar, ac, br, bc
    ))
}

/// The warning a square system, `inv` and `A^-1` give for a matrix with a
/// pivot at or below `Matrix::singular_tol` (cycle 08; an error before it,
/// QA D26). MATLAB's wording. The caller writes it through
/// [`warning_line`] to `Interp.err` and goes on with the result.
pub fn singular_warning() -> String {
    "Matrix is singular to working precision.".to_string()
}

/// The warning a non-square `\` or `/` gives when the column-pivoted QR
/// finds fewer independent columns than the smaller dimension (cycle 08).
/// SplatCrab's own wording: MATLAB's is understood to name the tolerance as
/// well, which no source at hand settles.
pub fn rank_deficient_warning(rank: usize) -> String {
    format!(
        "Matrix is rank deficient to working precision (rank {}).",
        rank
    )
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

/// `func2str(5)` or `arrayfun(5, v)`: an argument that must be a function
/// handle (cycle 06). SplatCrab's wording, in the form of
/// [`arg_not_a_string`].
pub fn arg_not_a_handle(pos: usize, name: &str) -> MError {
    MError::new(format!(
        "Argument {} to '{}' must be a function handle.",
        pos, name
    ))
}

/// `arrayfun(f, A, B)` with `A` and `B` of different sizes (cycle 06).
/// MATLAB's first sentence as recalled, not confirmed against a MathWorks
/// source.
pub fn arrayfun_size() -> MError {
    MError::new("All of the input arguments must be of the same size and shape.")
}

/// A call inside `arrayfun` that returned something other than a scalar,
/// which a uniform output cannot hold (cycle 06). `index` is the element,
/// `output` the output, both one-based. MATLAB's two sentences as recalled,
/// on one line, not confirmed against a MathWorks source.
pub fn arrayfun_nonscalar(index: usize, output: usize) -> MError {
    MError::new(format!(
        "Non-scalar in Uniform output, at index {}, output {}. Set 'UniformOutput' to false.",
        index, output
    ))
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

/// `error('...')` with a message the user composed, and the identifier it
/// named, empty when it named none.
pub fn raised(msg: String, identifier: String) -> MError {
    MError::new(msg).with_identifier(identifier)
}

/// `warning('...')`: the line written to the error sink. `Warning: ` and the
/// message, MATLAB's form; the identifier is not shown.
pub fn warning_line(msg: &str) -> String {
    format!("Warning: {}\n", msg)
}

/// `error(x)` with no format string: MATLAB's bare fallback text.
pub fn raised_default() -> MError {
    MError::new("error")
}

/// MATLAB's wording: `assert(cond)` with a false condition and no message.
pub fn assertion_failed() -> MError {
    MError::new("Assertion failed.")
}

/// MATLAB's wording: `switch [1 2]`, a switch on a value that is neither a
/// scalar nor a character vector (cycle 04).
pub fn switch_expression() -> MError {
    MError::new("SWITCH expression must be a scalar or a character vector.")
}

/// MATLAB's wording for a function that has no method for its argument's
/// class: `rethrow(5)`, which takes an `MException` only.
pub fn no_method(name: &str, class: &str) -> MError {
    MError::new(format!(
        "Undefined function '{}' for input arguments of type '{}'.",
        name, class
    ))
}

/// A value that is not an array where an array is needed: an `MException`
/// used in arithmetic, indexed, or handed to a numeric builtin. SplatCrab's
/// own wording; MATLAB names the operator or function in each case.
pub fn not_an_array(class: &str) -> MError {
    MError::new(format!(
        "This operation is not supported for a value of class '{}'.",
        class
    ))
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

pub fn sort_vectors_only() -> MError {
    MError::new("'sort' currently supports vectors only.")
}

pub fn dot_size_mismatch() -> MError {
    ab_size_mismatch("dot")
}

/// `dot` and `cross` with operands of different sizes.
pub fn ab_size_mismatch(name: &str) -> MError {
    MError::new(format!("A and B must be the same size for '{}'.", name))
}

// ---- linear algebra (cycle 08) -----------------------------------------
//
// SplatCrab's own wording throughout, except `not_positive_definite`,
// which is the text the spec records for `chol`.

/// `eig`, `chol` and `inv`'s neighbours: a matrix that must be square. The
/// same form as [`nonsquare_trace`].
pub fn nonsquare_for(name: &str) -> MError {
    MError::new(format!("Matrix must be square for '{}'.", name))
}

/// `chol` of a matrix that is not symmetric positive definite, a `NaN` on
/// the diagonal included.
pub fn not_positive_definite() -> MError {
    MError::new("Matrix must be positive definite.")
}

/// `eig`, `svd` and the builtins built on `svd` (`rank`, `pinv`, `null`,
/// `orth`, `cond`, and the matrix 2-norm) refuse a `NaN` or an `Inf` rather
/// than iterating on it.
pub fn nonfinite_input(name: &str) -> MError {
    MError::new(format!("Input to '{}' must not contain NaN or Inf.", name))
}

/// An iterative method that reached its cap (invariant 6: every iteration
/// has one). `name` is the builtin the user called.
pub fn no_convergence(name: &str) -> MError {
    MError::new(format!(
        "'{}' did not converge within its iteration limit.",
        name
    ))
}

/// `eig` of a real matrix with a complex pair, until cycle 10. Opens with
/// the sentence every complex refusal shares.
pub fn complex_eigenvalues() -> MError {
    MError::new(
        "Complex results are not supported. \
         The eigenvalues of this matrix are complex.",
    )
}

/// `norm(A, p)` of a matrix for a `p` other than 1, 2, `Inf` and `'fro'`.
pub fn matrix_norm_type() -> MError {
    MError::new("Matrix norm type for 'norm' must be 1, 2, Inf or 'fro'.")
}

/// `cross(a, b)` where no dimension has length 3.
pub fn cross_length() -> MError {
    MError::new("A and B must have a dimension of length 3 for 'cross'.")
}

/// `magic(n)` with an `n` that is not a real scalar.
pub fn magic_order() -> MError {
    MError::new("Order for 'magic' must be a real scalar.")
}

/// `rank(A, tol)` and `pinv(A, tol)` with a `tol` that is not a real scalar.
pub fn tolerance_arg(name: &str) -> MError {
    MError::new(format!("Tolerance for '{}' must be a real scalar.", name))
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

// ---- protocol --------------------------------------------------------

// A request the protocol cannot act on is answered, not fatal. Every text
// but the unknown operation's is `Malformed request: <what is wrong>.`, the
// form docs/modules/U0-ui-foundations.md fixes; what follows the colon is
// SplatCrab's own wording.

fn malformed(what: &str) -> MError {
    MError::new(format!("Malformed request: {}.", what))
}

/// A line that is not one JSON value.
pub fn request_not_json() -> MError {
    malformed("not valid JSON")
}

/// A line nested past [`crate::json::MAX_DEPTH`] arrays or objects: refused
/// by the depth limit before it could recurse far enough to overflow.
pub fn request_too_deep() -> MError {
    malformed("nested too deeply")
}

/// JSON, but not an object: `[1, 2]`, `"eval"`, `3`.
pub fn request_not_object() -> MError {
    malformed("not a JSON object")
}

/// An `id` that is neither a number nor a string.
pub fn request_bad_id() -> MError {
    malformed("'id' must be a number or a string")
}

/// A field the operation needs, `op` included, that is absent.
pub fn request_missing(field: &str) -> MError {
    malformed(&format!("no '{}' field", field))
}

/// A field present with a value that is not a string.
pub fn request_not_string(field: &str) -> MError {
    malformed(&format!("'{}' must be a string", field))
}

pub fn unknown_operation(op: &str) -> MError {
    MError::new(format!("Unknown operation '{}'.", op))
}

// ---- UI server -------------------------------------------------------

/// The status line text of every status `http.rs` can answer, which is also
/// the whole body of an error response (docs/modules/U1-ui-server.md): the
/// code, a space and the reason phrase of RFC 9110, `413 Content Too Large`
/// included rather than the older `Payload Too Large`.
pub const HTTP_STATUS: [(u16, &str); 9] = [
    (200, "200 OK"),
    (400, "400 Bad Request"),
    (403, "403 Forbidden"),
    (404, "404 Not Found"),
    (405, "405 Method Not Allowed"),
    (413, "413 Content Too Large"),
    (415, "415 Unsupported Media Type"),
    (431, "431 Request Header Fields Too Large"),
    (501, "501 Not Implemented"),
];

/// `HTTP_STATUS`'s text for `code`. Every code `http.rs` uses is in the
/// table, which a unit test there checks, so the fallback is never written.
pub fn http_status(code: u16) -> &'static str {
    HTTP_STATUS
        .iter()
        .find(|(c, _)| *c == code)
        .map_or("500 Internal Server Error", |(_, text)| text)
}

// The command-line options of `--ui` and `--http-stdio`, which `main.rs`
// reports as `Error: <msg>` on stderr before exiting 1.

/// `--port` or `--token` as the last argument, with nothing after it.
pub fn option_needs_value(opt: &str) -> MError {
    MError::new(format!("Option '{}' needs a value.", opt))
}

pub fn bad_port(text: &str) -> MError {
    MError::new(format!(
        "Option '--port' needs a port number from 0 to 65535, not '{}'.",
        text
    ))
}

/// A token that could not travel in a URL fragment and a header unchanged.
pub fn bad_token() -> MError {
    MError::new("Option '--token' needs letters, digits, '.', '_', '~' or '-' only.")
}

pub fn unknown_option(mode: &str, opt: &str) -> MError {
    MError::new(format!("Unknown option '{}' for {}.", opt, mode))
}

pub fn missing_option(mode: &str, opt: &str) -> MError {
    MError::new(format!("{} needs the option '{}'.", mode, opt))
}

/// Binding the loopback port failed, most often because it is in use.
pub fn cannot_listen(port: u16, e: &std::io::Error) -> MError {
    MError::new(format!("Cannot listen on 127.0.0.1:{}: {}", port, e))
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

    /// Cycle 05: leaving a function moves the line into the trace, and the
    /// trace lists the frames innermost first.
    #[test]
    fn the_trace_lists_each_frame_left_innermost_first() {
        let e = undefined("x").at(8).leaving("g3");
        assert_eq!(e.line, None);
        assert_eq!(e.trace(), "  in g3 (line 8)\n");
        let e = e.at(4).leaving("outer").at(2);
        assert_eq!(
            e.to_string(),
            "Line 2: Unrecognized function or variable 'x'."
        );
        assert_eq!(e.trace(), "  in g3 (line 8)\n  in outer (line 4)\n");
        assert_eq!(undefined("x").trace(), "");
        assert_eq!(too_many_outputs().leaving("f").trace(), "  in f\n");
        // Cycle 07: an entry also records the file, which `e.stack` reports
        // and the trace does not show.
        let e = undefined("x")
            .at(3)
            .leaving_file("h", "/p/h.m")
            .leaving("g");
        let files: Vec<&str> = e.stack().iter().map(|s| s.file.as_str()).collect();
        assert_eq!(files, ["/p/h.m", ""]);
        assert_eq!(e.trace(), "  in h (line 3)\n  in g\n");
    }

    /// Cycle 07's texts that take arguments.
    #[test]
    fn the_container_messages() {
        assert_eq!(
            cs_list_count(2).msg,
            "Expected one output from a curly brace or dot indexing expression, but there were 2 results."
        );
        assert_eq!(
            operator_unsupported("+", "cell").msg,
            "Operator '+' is not supported for operands of type 'cell'."
        );
        assert_eq!(
            conversion("cell", "double").msg,
            "Conversion to cell from double is not possible."
        );
        assert_eq!(no_such_field("b").msg, "Unrecognized field name \"b\".");
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
        const FILES: [(&str, &str); 14] = [
            ("lexer.rs", include_str!("lexer.rs")),
            ("parser.rs", include_str!("parser.rs")),
            ("interp.rs", include_str!("interp.rs")),
            ("value.rs", include_str!("value.rs")),
            ("json.rs", include_str!("json.rs")),
            ("syntax.rs", include_str!("syntax.rs")),
            ("env.rs", include_str!("env.rs")),
            ("protocol.rs", include_str!("protocol.rs")),
            ("http.rs", include_str!("http.rs")),
            ("server.rs", include_str!("server.rs")),
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
