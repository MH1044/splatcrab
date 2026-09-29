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

/// A character the lexer has no use for. A printable one is quoted as
/// itself; one that would not show, a control character or an invisible
/// format character, is named by its code point instead, `unexpected
/// character U+0000`, so the message never carries a raw control byte to
/// the terminal (cycle 11; a UTF-16 file used to write a literal NUL).
pub fn unexpected_char(c: char) -> MError {
    let invisible = c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(
            c,
            '\u{00AD}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{FEFF}'
        );
    if invisible {
        MError::new(format!("unexpected character U+{:04X}", c as u32))
    } else {
        MError::new(format!("unexpected character '{}'", c))
    }
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

// ---- complex values (cycle 10) --------------------------------------
//
// Cycle 01d refused every result that would have been complex, and cycles
// 08 and 09 routed complex eigenvalues and roots through the same refusal;
// cycle 10 replaced all of them with the values. What is left is the other
// direction: a kernel that reads only real parts refuses a complex value
// rather than drop its imaginary part in silence. SplatCrab's own wording.

/// A complex argument to a builtin that does not take one, `sort([1+2i
/// 3])`, a complex value returned to a solver, a complex operand of `:`
/// and a complex value becoming a char.
pub fn complex_argument(name: &str) -> MError {
    MError::new(format!("Complex values are not supported by '{}'.", name))
}

/// A complex value where a logical is wanted: `if`, `while`, `&`, `|`,
/// `~`, `&&`, `||` and an assignment into a logical array.
pub fn complex_to_logical() -> MError {
    MError::new("Complex values cannot be converted to logicals.")
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

// ---- numerics (cycle 09) ---------------------------------------------
//
// SplatCrab's own wording throughout, as the spec asks where no source
// settles MATLAB's, except `fzero_endpoints`, whose sentence is MATLAB's as
// recalled and not confirmed against a MathWorks source.

/// An argument that must be a vector (or empty): `polyval([1 2; 3 4], 1)`.
pub fn arg_not_a_vector(pos: usize, name: &str) -> MError {
    MError::new(format!("Argument {} to '{}' must be a vector.", pos, name))
}

/// `polyfit(x, y, n)`, `interp1(x, v, xq)` and `trapz(x, y)` with a different
/// number of sample points and values.
pub fn sample_length(name: &str) -> MError {
    MError::new(format!(
        "The sample points and the values for '{}' must have the same length.",
        name
    ))
}

/// A degree, an order, a count or an element that must be a non-negative
/// integer: `polyfit(x, y, 1.5)`, `diff(x, -1)`, `factorial(2.5)`,
/// `isprime(-3)`, `nchoosek(4.5, 2)`.
pub fn arg_nonneg_int(pos: usize, name: &str) -> MError {
    MError::new(format!(
        "Argument {} to '{}' must be a non-negative integer.",
        pos, name
    ))
}

/// `gcd(2.5, 5)`, `lcm(4, NaN)`: elements that must be integers.
pub fn arg_integers(pos: usize, name: &str) -> MError {
    MError::new(format!(
        "Argument {} to '{}' must hold integers.",
        pos, name
    ))
}

/// `nchoosek(3, 5)`.
pub fn nchoosek_k() -> MError {
    MError::new("K must be an integer between 0 and N for 'nchoosek'.")
}

/// `filter(1, [0 1], x)` and `deconv(b, [0 1])`: a leading coefficient
/// that is zero, or no coefficient at all.
pub fn leading_zero(name: &str) -> MError {
    MError::new(format!(
        "The first coefficient of the denominator for '{}' must be non-zero.",
        name
    ))
}

/// `conv(u, v, 'middle')`.
pub fn conv_shape() -> MError {
    MError::new("Shape for 'conv' must be 'full', 'same' or 'valid'.")
}

/// `interp1(x, v, xq, 'spline')`: a method this cycle does not provide.
pub fn interp_method() -> MError {
    MError::new("Method for 'interp1' must be 'linear', 'nearest', 'previous' or 'next'.")
}

/// `interp1(x, v, xq, 'linear', 'x')`.
pub fn interp_extrap() -> MError {
    MError::new("Extrapolation for 'interp1' must be 'extrap' or a scalar.")
}

/// `interp1([1 1 2], v, xq)`, a `NaN` sample point, or fewer than two.
pub fn interp_points() -> MError {
    MError::new(
        "The sample points for 'interp1' must be at least two distinct values, none of them NaN.",
    )
}

/// `histc(x, [3 1 2])`.
pub fn histc_edges() -> MError {
    MError::new("Edges for 'histc' must be monotonically non-decreasing, with no NaN.")
}

/// `std(x, 2)`: a weight other than `0`, `1` or `[]`.
pub fn std_weight(name: &str) -> MError {
    MError::new(format!("Weight for '{}' must be 0 or 1.", name))
}

/// `unique({1, 'a'})`: a cell holding anything but character vectors.
pub fn set_cell_contents(name: &str) -> MError {
    MError::new(format!(
        "Cell arrays for '{}' must hold character vectors only.",
        name
    ))
}

/// `ismember(1, {'a'})`: a cell of character vectors with a numeric array.
pub fn set_mixed(name: &str) -> MError {
    MError::new(format!(
        "A cell array of character vectors for '{}' can be combined only with another one or with a character vector.",
        name
    ))
}

/// `unique(x, 'rows')` or any other option the set functions do not take.
pub fn set_option(name: &str) -> MError {
    MError::new(format!(
        "Option for '{}' must be 'sorted' or 'stable'.",
        name
    ))
}

/// A solver's function that returned the wrong kind of value: `what` says
/// what it should have returned.
/// A solver's function returned something other than one real number.
pub fn solver_not_scalar(name: &str) -> MError {
    solver_output(name, "a real scalar")
}

/// `integral`'s function returned fewer or more values than points: it was
/// written with `*` where it needed `.*`.
pub fn integral_not_elementwise() -> MError {
    solver_output(
        "integral",
        "a value for every point of its input; write it with element-wise operators such as .* and ./",
    )
}

/// `ode45`'s function returned a vector of the wrong length.
pub fn ode_value_length() -> MError {
    solver_output("ode45", "a vector with one element per component of y0")
}

fn solver_output(name: &str, what: &str) -> MError {
    MError::new(format!(
        "The function passed to '{}' must return {}.",
        name, what
    ))
}

/// A solver's function that returned `NaN` (or, where a value must be
/// finite, `Inf`).
pub fn solver_nonfinite(name: &str) -> MError {
    MError::new(format!(
        "The function passed to '{}' returned NaN or Inf.",
        name
    ))
}

/// `fzero(f, x0)` whose search for an interval found no sign change before
/// the function or the interval stopped being finite, or before its cap.
pub fn fzero_no_sign_change() -> MError {
    MError::new("'fzero' found no sign change of the function in its search for an interval.")
}

/// `fzero(f, [a b])` where `f(a)` and `f(b)` have the same sign.
/// SplatCrab's own text: the wording follows MATLAB's as recalled, which no
/// MathWorks source here confirms (cycle 09's review).
pub fn fzero_endpoints() -> MError {
    MError::new("The function values at the interval endpoints must differ in sign.")
}

/// `fzero(f, [1 2 3])`.
pub fn fzero_start() -> MError {
    MError::new("The starting point for 'fzero' must be a scalar or a two-element interval.")
}

/// `integral` that reached its subinterval cap, or subintervals too narrow
/// to split, without meeting its tolerance: a divergent or singular
/// integral, `integral(@(x) 1 ./ x, 0, 1)`.
pub fn integral_limit(max: usize) -> MError {
    MError::new(format!(
        "'integral' reached its limit of {} subintervals without meeting the tolerance; the integral may not exist.",
        max
    ))
}

/// `ode45` past its step cap.
pub fn ode_step_limit(max: usize) -> MError {
    MError::new(format!(
        "'ode45' reached its limit of {} steps before the end of the time span.",
        max
    ))
}

/// `ode45` whose step fell below `16 * eps(t)`. `t` arrives rendered.
pub fn ode_min_step(t: &str) -> MError {
    MError::new(format!(
        "'ode45' cannot meet the tolerances without a step below the smallest allowed, at t = {}.",
        t
    ))
}

/// `ode45(f, [0 0], y0)`, `ode45(f, [0 2 1], y0)`, `ode45(f, 1, y0)`.
pub fn ode_tspan() -> MError {
    MError::new(
        "The time span for 'ode45' must be a vector of at least two distinct, monotonic, finite times.",
    )
}

/// A name-value option a builtin does not know: `odeset('Foo', 1)`,
/// `integral(f, 0, 1, 'Waypoints', 1)`.
pub fn unrecognized_option(opt: &str, name: &str) -> MError {
    MError::new(format!("Unrecognized option '{}' for '{}'.", opt, name))
}

/// `odeset('RelTol')`: an option name with no value after it.
pub fn option_pairs(name: &str) -> MError {
    MError::new(format!("Options for '{}' must be name-value pairs.", name))
}

/// `odeset('RelTol', -1)`, `integral(f, 0, 1, 'AbsTol', 'x')`: a
/// tolerance that is negative or not a real number, a step or a refinement
/// that is not positive, a refinement that is not an integer, or an
/// `AbsTol` with neither one value nor one per component.
pub fn option_value(opt: &str, name: &str) -> MError {
    MError::new(format!(
        "The value of option '{}' for '{}' is not valid.",
        opt, name
    ))
}

// ---- strings, regular expressions and files (cycle 11) ---------------

/// `regexp('aa', '(a)\1')`: a backreference, which no engine can match in
/// linear time. SplatCrab's own text.
pub fn regex_backreference() -> MError {
    MError::new("Backreferences are not supported in regular expressions.")
}

/// `(?=...)`, `(?!...)`, `(?<=...)` and `(?<!...)`. SplatCrab's own text.
pub fn regex_lookaround() -> MError {
    MError::new("Lookahead and lookbehind are not supported in regular expressions.")
}

/// A construct of Perl's the regular-expression engine does not take,
/// beside backreferences and lookaround.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegexUnsupported {
    Possessive,
    Atomic,
    Conditional,
    InlineFlag,
}

/// A way a pattern fails to parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegexFault {
    NothingToRepeat,
    MissingParen,
    UnmatchedParen,
    MissingBracket,
    TrailingBackslash,
    RepeatOrder,
    GroupName,
    GroupNameEnd,
    BadCode,
    CodeTooLarge,
    RangeOrder,
}

/// `a*+`, `(?>a)`, `(?(1)a)`, `(?i)a`.
pub fn regex_unsupported(what: RegexUnsupported) -> MError {
    let what = match what {
        RegexUnsupported::Possessive => "a possessive quantifier",
        RegexUnsupported::Atomic => "an atomic group",
        RegexUnsupported::Conditional => "a conditional",
        RegexUnsupported::InlineFlag => "an inline flag",
    };
    MError::new(format!(
        "Invalid regular expression: {} is not supported.",
        what
    ))
}

/// A pattern that does not parse, and why.
pub fn regex_syntax(fault: RegexFault) -> MError {
    let what = match fault {
        RegexFault::NothingToRepeat => "nothing to repeat",
        RegexFault::MissingParen => "missing ')'",
        RegexFault::UnmatchedParen => "an unmatched ')'",
        RegexFault::MissingBracket => "missing ']'",
        RegexFault::TrailingBackslash => "a trailing backslash",
        RegexFault::RepeatOrder => "a repetition's maximum is below its minimum",
        RegexFault::GroupName => {
            "a group name must be a letter followed by letters, digits or underscores"
        }
        RegexFault::GroupNameEnd => "missing '>' after a group name",
        RegexFault::BadCode => "a malformed \\x or \\o escape",
        RegexFault::CodeTooLarge => "a character code past U+FFFF",
        RegexFault::RangeOrder => "a class range out of order",
    };
    MError::new(format!("Invalid regular expression: {}.", what))
}

/// A pattern past the engine's bounds: nested too deeply, a repetition
/// count past 1000, or a compiled program past its size limit, the ranges
/// of its character classes counted in it.
pub fn regex_too_large() -> MError {
    MError::new("The regular expression is too large.")
}

/// `input` under `--protocol`, `--ui` or `--http-stdio`, where standard
/// input is the protocol's channel or there is no terminal at all.
pub fn input_unavailable() -> MError {
    MError::new("input is not available in this session: there is no terminal to read from.")
}

/// `input` after standard input has ended.
pub fn input_ended() -> MError {
    MError::new("'input' reached the end of standard input.")
}

/// `input('', 'x')`: the second argument is not `'s'`.
pub fn input_option() -> MError {
    MError::new("The second argument to 'input' must be 's'.")
}

/// A file identifier that names no open file: `fprintf(7, 'x')`, and every
/// file function given `-1` after a failed `fopen`. MATLAB's first sentence.
pub fn invalid_fid() -> MError {
    MError::new("Invalid file identifier.")
}

/// `fopen('f', 'q')`: a permission that is not one of `r w a r+ w+ a+`,
/// with an optional `t` or `b`.
pub fn fopen_permission(mode: &str) -> MError {
    MError::new(format!("Invalid permission '{}' for 'fopen'.", mode))
}

/// A read from a file opened only for writing.
pub fn file_not_readable() -> MError {
    MError::new("The file is not open for reading.")
}

/// A write to a file opened only for reading.
pub fn file_not_writable() -> MError {
    MError::new("The file is not open for writing.")
}

/// The reason an operation on a file failed, in words that are the same on
/// every platform, so a golden case can pin them.
pub fn io_reason(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::NotFound => "No such file or directory",
        std::io::ErrorKind::PermissionDenied => "Permission denied",
        std::io::ErrorKind::AlreadyExists => "The file already exists",
        std::io::ErrorKind::IsADirectory => "It is a directory",
        _ => "The operation failed",
    }
}

/// `fileread`, `readmatrix`, `csvread` or `load` of a file that cannot be
/// read.
pub fn cannot_read_file(name: &str, e: &std::io::Error) -> MError {
    MError::new(format!("Unable to read file '{}': {}.", name, io_reason(e)))
}

/// `writematrix`, `csvwrite`, `save`, `fprintf(fid, ...)` or `fwrite`
/// failing to write.
pub fn cannot_write_file(name: &str, e: &std::io::Error) -> MError {
    MError::new(format!(
        "Unable to write file '{}': {}.",
        name,
        io_reason(e)
    ))
}

/// `delete('*.txt')`: SplatCrab's `delete` takes each file by its full
/// name, which is safer than expanding a pattern (a recorded deviation).
/// SplatCrab's own text.
pub fn delete_wildcard() -> MError {
    MError::new("Wildcards are not supported by 'delete'.")
}

/// The warning `delete` gives for a file that is not there, which it then
/// goes on from, as MATLAB does.
pub fn delete_not_found(name: &str) -> String {
    format!("File '{}' not found.", name)
}

/// `delete` of a file that is there and could not be removed.
pub fn cannot_delete(name: &str, e: &std::io::Error) -> MError {
    MError::new(format!(
        "Unable to delete file '{}': {}.",
        name,
        io_reason(e)
    ))
}

/// `fread(fid, n, 'int9')` or `fwrite(fid, x, 'int9')`.
pub fn bad_precision(precision: &str, name: &str) -> MError {
    MError::new(format!("Invalid precision '{}' for '{}'.", precision, name))
}

/// A way a MAT file fails to load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatFault {
    /// It ends inside its header or inside a data element.
    Truncated,
    /// It ends right after its header, with no variable.
    HeaderOnly,
    /// Its header is not a version 5 header.
    NotVersion5,
    /// It holds compressed data (MATLAB's default since v7).
    Compressed,
    /// A length, a type, a size or a name in it is inconsistent.
    Corrupt,
    /// An array of more than two dimensions.
    NDims,
    /// Cells and structs nested past `mat::MAX_DEPTH`.
    TooDeep,
    /// A sparse array.
    Sparse,
    /// An object or a function handle.
    Object,
    /// A struct array with no fields and more elements than
    /// `mat::MAX_FIELDLESS`, the bound it carries.
    FieldlessTooLarge(usize),
}

/// A MAT file that cannot be loaded, and why.
pub fn mat_file(name: &str, fault: MatFault) -> MError {
    let bound;
    let what = match fault {
        MatFault::FieldlessTooLarge(limit) => {
            bound = format!(
                "it holds a struct array with no fields and more than {} elements",
                limit
            );
            &bound
        }
        MatFault::Truncated => "the file is truncated",
        MatFault::HeaderOnly => "the file is truncated after its header",
        MatFault::NotVersion5 => "it is not a MAT-file of version 5",
        MatFault::Compressed => {
            "it holds compressed data, which SplatCrab does not read (save with -v6)"
        }
        MatFault::Corrupt => "its data is corrupt",
        MatFault::NDims => "an array in it has more than two dimensions",
        MatFault::TooDeep => "its cells and structs are nested too deeply",
        MatFault::Sparse => "it holds a sparse array, which SplatCrab does not read",
        MatFault::Object => {
            "it holds an object or a function handle, which SplatCrab does not read"
        }
    };
    MError::new(format!("Unable to read MAT-file '{}': {}.", name, what))
}

/// `save('f.mat', 'q')` with no variable `q`.
pub fn save_no_variable(name: &str) -> MError {
    MError::new(format!("Variable '{}' not found.", name))
}

/// `save` with no variable to write, in an empty workspace: a MAT-file of
/// a header alone is one `load` refuses as truncated (spec item 16), so
/// `save` does not write one. SplatCrab's own text.
pub fn save_nothing() -> MError {
    MError::new("There are no variables to save.")
}

/// The warning `load('f.mat', 'q')` gives for a variable the file does not
/// hold, which it then goes on from.
pub fn load_no_variable(name: &str) -> String {
    format!("Variable '{}' not found.", name)
}

/// `save` of a value the MAT writer has no form for: a function handle
/// or an `MException`.
pub fn save_unsupported(var: &str, class: &str) -> MError {
    MError::new(format!(
        "Unable to save variable '{}': a value of class '{}' cannot be saved.",
        var, class
    ))
}

/// `save` of a variable holding more than the 4 GiB a MAT-file of version
/// 5 can give one element, whose length is a 32-bit count: a cell of
/// several 2 GB arrays. SplatCrab's own text.
pub fn save_too_large(var: &str) -> MError {
    MError::new(format!(
        "Unable to save variable '{}': it is larger than the 4 GiB a MAT-file of version 5 can hold in one element.",
        var
    ))
}

/// `save` of a struct array with no fields and more than `limit`
/// (`mat::MAX_FIELDLESS`) elements, which `load` would refuse. SplatCrab's
/// own text.
pub fn save_fieldless_too_large(var: &str, limit: usize) -> MError {
    MError::new(format!(
        "Unable to save variable '{}': a struct array with no fields and more than {} elements cannot be saved.",
        var, limit
    ))
}

/// `save` of cells or structs nested past `mat::MAX_DEPTH` levels.
pub fn save_too_deep(var: &str) -> MError {
    MError::new(format!(
        "Unable to save variable '{}': its cells and structs are nested too deeply.",
        var
    ))
}

/// `save('f.txt', 'c', '-ascii')` of a value that is not a numeric,
/// logical or char array.
pub fn save_ascii_unsupported(var: &str, class: &str) -> MError {
    MError::new(format!(
        "Unable to save variable '{}' as text: a value of class '{}' is not a numeric array.",
        var, class
    ))
}

/// A text file `load -ascii` cannot read: a value that is not a number
/// on line `line`.
pub fn text_not_number(name: &str, line: usize) -> MError {
    MError::new(format!(
        "Unable to read file '{}': line {} holds a value that is not a number.",
        name, line
    ))
}

/// A text file whose line `line` has a different number of values from
/// the lines before it, which `load -ascii` cannot make a matrix of.
pub fn text_ragged(name: &str, line: usize) -> MError {
    MError::new(format!(
        "Unable to read file '{}': line {} does not have as many values as the lines before it.",
        name, line
    ))
}

/// A file name argument that is empty.
pub fn empty_file_name(name: &str) -> MError {
    MError::new(format!("The file name given to '{}' is empty.", name))
}

/// `writematrix(c, 'f.csv')` of a value that is not an array.
pub fn write_unsupported(class: &str, name: &str) -> MError {
    MError::new(format!(
        "'{}' cannot write a value of class '{}'.",
        name, class
    ))
}

/// `strjoin({1, 'a'})`, `strcat({'a'}, {2})` and the other string
/// functions given a cell that holds something other than text.
pub fn cell_not_text(name: &str) -> MError {
    MError::new(format!(
        "Every element of a cell argument to '{}' must be a character vector.",
        name
    ))
}

/// `strjoin({'a', 'b', 'c'}, {'-'})`: a cell of delimiters that is not
/// one shorter than the cell it joins.
pub fn strjoin_delimiters() -> MError {
    MError::new(
        "A cell of delimiters for 'strjoin' must have one fewer element than the cell it joins.",
    )
}

/// `strcmp({'a', 'b'}, {'a', 'b', 'c'})`: two cells of different sizes,
/// neither of them one element. MATLAB's text as recalled.
pub fn strcmp_sizes() -> MError {
    MError::new("Inputs must be the same size or either one can be a scalar.")
}

/// `strcat` of char arrays whose row counts differ.
pub fn strcat_rows() -> MError {
    MError::new("All the character arrays given to 'strcat' must have the same number of rows.")
}

/// `strcat` of cells whose sizes differ.
pub fn strcat_cells() -> MError {
    MError::new("All the cell arrays given to 'strcat' must be the same size or have one element.")
}

/// `mat2str(A, 0)`: a precision that is not a positive integer.
pub fn precision_arg(name: &str) -> MError {
    MError::new(format!(
        "Precision for '{}' must be a positive integer.",
        name
    ))
}

/// `mat2str` of a cell, a struct, a handle or an `MException`.
pub fn mat2str_input() -> MError {
    MError::new("Input to 'mat2str' must be a numeric, logical or char matrix.")
}

/// `strncmp(a, b, -1)`: a count that is not a non-negative integer.
pub fn strncmp_count(name: &str) -> MError {
    MError::new(format!(
        "The number of characters to compare in '{}' must be a non-negative integer.",
        name
    ))
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
        const FILES: [(&str, &str); 22] = [
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
            ("builtins/numerics.rs", include_str!("builtins/numerics.rs")),
            ("builtins/sets.rs", include_str!("builtins/sets.rs")),
            ("builtins/solvers.rs", include_str!("builtins/solvers.rs")),
            ("builtins/strings.rs", include_str!("builtins/strings.rs")),
            ("builtins/regex.rs", include_str!("builtins/regex.rs")),
            ("builtins/printf.rs", include_str!("builtins/printf.rs")),
            ("builtins/io.rs", include_str!("builtins/io.rs")),
            ("builtins/mat.rs", include_str!("builtins/mat.rs")),
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
