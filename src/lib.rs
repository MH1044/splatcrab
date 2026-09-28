//! SplatCrab: a MATLAB-compatible numerical language.
//!
//! The pipeline is `lexer` -> `parser` -> `interp`, with `value` holding the
//! column-major matrix runtime. The binary in `main.rs` is the CLI and REPL;
//! it is the only place in the project allowed to use `print!`.
//!
//! `error` holds `MError` and every message text the other modules raise.
//!
//! Since cycle U0, `protocol` is the evaluation protocol behind
//! `splatcrab --protocol`, built on `json` (a hand-written JSON value, parser
//! and writer), `syntax` (whether an entry is complete, shared with the REPL)
//! and `env` (name completions).
//!
//! Since cycle U1, `http` turns the bytes of one HTTP request into the bytes
//! of its response, and `server` is the loopback socket around it behind
//! `splatcrab --ui`; the page it serves is embedded from `src/ui/`.

pub mod builtins;
pub mod env;
pub mod error;
pub mod http;
pub mod interp;
pub mod json;
pub mod lexer;
pub mod parser;
pub mod protocol;
pub mod server;
pub mod syntax;
pub mod value;
