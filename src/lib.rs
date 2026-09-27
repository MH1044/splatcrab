//! SplatCrab: a MATLAB-compatible numerical language.
//!
//! The pipeline is `lexer` -> `parser` -> `interp`, with `value` holding the
//! column-major matrix runtime. The binary in `main.rs` is the CLI and REPL;
//! it is the only place in the project allowed to use `print!`.
//!
//! `error` holds `MError` and every message text the other modules raise.

pub mod builtins;
pub mod error;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod value;
