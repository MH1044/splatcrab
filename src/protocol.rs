//! The evaluation protocol: `splatcrab --protocol`.
//!
//! JSON Lines over any reader and writer. Each line read is one request, a
//! JSON object; each line written is its response, one JSON object, flushed
//! before the next request is read so that a client can interleave. All of
//! them run against one [`Interp`], so a variable assigned by one `eval` is
//! there for the next. The operations, their fields and the order of the keys
//! in each response are fixed by `docs/modules/U0-ui-foundations.md`, for
//! the desktop's three and `workspace`'s `preview` by
//! `docs/modules/U2-ui-desktop.md`, for the editor's three and `eval`'s
//! `stack` by `docs/modules/U3-ui-editor.md`, and for the figures and the
//! current folder by `docs/modules/U4-ui-figures.md`:
//!
//! | `op` | needs | answers, after `id` and `ok` |
//! |---|---|---|
//! | `eval` | `code`; `stack`, optional | `out`, then `error` when it failed: `message`, `line`, and `stack` after `line` when `stack` is `true` |
//! | `complete` | `code` | `complete`, [`syntax::is_complete`] |
//! | `workspace` | `preview`, optional | `vars`, one `{name, size, class}` per variable, and `value`, [`env::preview`], after `class` when `preview` is `true` |
//! | `completions` | `prefix` | `items`, [`env::completions`] |
//! | `files` | `path` | `root`, `path`, `entries` (`{name, dir, size}`) and `truncated`, [`files::list`] under [`Interp::file_root`] |
//! | `history` | | `items`, the history file's entries, [`history::load`] |
//! | `history_add` | `entry` | `added`, whether [`history::remember`] kept it and it was appended |
//! | `read_file` | `path` | `path` and `text`, [`files::read_file`] |
//! | `write_file` | `path`, then `text` | `path` and `size`, [`files::write_file`], after which the file's parse and every lookup are dropped ([`Interp::file_written`]) |
//! | `run_file` | `path` | as `eval`: `out`, then `error` when it failed, whose `line` is `null` and whose `stack` is always there; [`files::run_file`] and [`Interp::run_file`] |
//! | `figures` | | `open`, the open figures' numbers ascending ([`Interp::figure_numbers`]), and `changed`, those changed since the last `figures` ([`Interp::take_changed_figures`]) |
//! | `figure` | `n`, a positive whole number | `n` and `svg`, [`Interp::figure_svg`], at most [`MAX_INLINE_SVG`] bytes of it |
//! | `cwd` | `path`, optional | `cwd`, [`Interp::cwd`] relative to the root ([`files::folder_relative`]) or `null`, after moving there when `path` is given ([`files::current_folder`]) |
//!
//! A stack is the error's frames, innermost first, each `{file, name,
//! line}`: `file` relative to the root with `/` separators ([`files::relative`]),
//! or `null` outside it or where the frame has no file; `name` as the trace
//! writes it; `line` or `null`.
//!
//! A request that cannot be acted on is answered with `"ok":false` and an
//! `error` whose `line` is `null`, and the loop goes on: a session is not a
//! script, and neither a failed evaluation nor a malformed line ends it.
//!
//! Nothing here writes anywhere but the writer it is handed. The interpreter
//! is built over two sinks, and `eval` and `run_file` swap its `out` and its
//! `err` for one buffer for the length of the call, so evaluation output and
//! warnings reach the client only inside a response, in the order they were
//! written.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::rc::Rc;

use crate::env;
use crate::error::{self, MError, R};
use crate::files;
use crate::history;
use crate::interp::{InputSource, Interp};
use crate::json::{self, Json, ParseError};
use crate::syntax;

/// The longest SVG text `figure` answers inline, 32 MiB: a longer figure
/// is refused with a word to save it with `saveas`. What the protocol
/// passes to `figure` as its bound, a parameter so a unit test reaches it.
pub const MAX_INLINE_SVG: usize = 32 * 1024 * 1024;

/// Serves requests from `input` until it ends, against a fresh interpreter
/// whose file root is fixed now, to the working directory.
///
/// `Ok` at end of input whatever the requests did; `Err` only when reading
/// the input or writing a response fails, which is the transport failing
/// rather than a request.
pub fn serve(input: impl BufRead, output: impl Write) -> io::Result<()> {
    let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
    it.file_root = Some(files::session_root());
    serve_with(&mut it, input, output)
}

/// [`serve`] against an interpreter the caller owns, so a test can look at
/// the session afterwards.
pub fn serve_with(
    it: &mut Interp,
    mut input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if input.read_until(b'\n', &mut raw)? == 0 {
            return Ok(());
        }
        // Decoded leniently, as script mode decodes a file: an undecodable
        // byte becomes U+FFFD inside a string, or a malformed request outside
        // one, never an I/O error that ends the session.
        let text = String::from_utf8_lossy(&raw);
        let line = text.strip_suffix('\n').unwrap_or(&text);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        let mut reply = String::new();
        respond(it, line).write(&mut reply);
        reply.push('\n');
        output.write_all(reply.as_bytes())?;
        output.flush()?;
    }
}

/// The response to one request line.
pub fn respond(it: &mut Interp, line: &str) -> Json {
    let req = match json::parse(line) {
        Ok(v @ Json::Object(_)) => v,
        Ok(_) => return refused(Json::Null, error::request_not_object()),
        Err(ParseError::TooDeep) => {
            return refused(Json::Null, error::request_too_deep());
        }
        Err(ParseError::Syntax(_)) => return refused(Json::Null, error::request_not_json()),
    };
    // An explicit `null` is the same as no id, which is how it is answered.
    let id = match req.get("id") {
        None | Some(Json::Null) => Json::Null,
        Some(v @ (Json::Number(_) | Json::String(_))) => v.clone(),
        Some(_) => return refused(Json::Null, error::request_bad_id()),
    };
    let op = match field(&req, "op") {
        Ok(op) => op,
        Err(e) => return refused(id, e),
    };
    let result = match op {
        "eval" => field(&req, "code").and_then(|code| {
            let stack = flag(&req, "stack", error::request_stack)?;
            Ok(eval(it, id.clone(), code, stack))
        }),
        "complete" => field(&req, "code").map(|code| {
            Json::object([
                ("id", id.clone()),
                ("ok", Json::Bool(true)),
                ("complete", Json::Bool(syntax::is_complete(code))),
            ])
        }),
        "workspace" => flag(&req, "preview", error::request_preview)
            .map(|preview| workspace(it, id.clone(), preview)),
        "completions" => field(&req, "prefix").map(|prefix| {
            let items = env::completions(prefix, it.vars(), it.builtins(), &it.path_dirs());
            items_answer(id.clone(), items)
        }),
        "files" => field(&req, "path").and_then(|path| list_files(it, id.clone(), path)),
        "history" => Ok(history_items(
            id.clone(),
            history::default_path().as_deref(),
        )),
        "history_add" => field(&req, "entry")
            .and_then(|entry| history_add(id.clone(), history::default_path().as_deref(), entry)),
        "read_file" => field(&req, "path").and_then(|path| read_file(it, id.clone(), path)),
        "write_file" => field(&req, "path").and_then(|path| {
            let text = field(&req, "text")?;
            write_file(it, id.clone(), path, text)
        }),
        "run_file" => field(&req, "path").and_then(|path| run_file(it, id.clone(), path)),
        "figures" => Ok(figures(it, id.clone())),
        "figure" => figure_number(&req).and_then(|n| figure(it, id.clone(), n, MAX_INLINE_SVG)),
        "cwd" => cwd(it, id.clone(), &req),
        other => Err(error::unknown_operation(other)),
    };
    result.unwrap_or_else(|e| refused(id, e))
}

/// `{"id", "ok", "items": [...]}`, the answer of `completions` and
/// `history`.
fn items_answer(id: Json, items: Vec<String>) -> Json {
    Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        (
            "items",
            Json::Array(items.into_iter().map(Json::String).collect()),
        ),
    ])
}

/// An optional flag, `workspace`'s `preview` or `eval`'s `stack`: absent
/// is `false`, and anything but a JSON boolean is refused, `null` included,
/// with `refusal`.
fn flag(req: &Json, name: &str, refusal: fn() -> MError) -> Result<bool, MError> {
    match req.get(name) {
        None => Ok(false),
        Some(Json::Bool(b)) => Ok(*b),
        Some(_) => Err(refusal()),
    }
}

/// The file root, for an operation on `path`. With no root, which only an
/// interpreter a test builds lacks, every path is outside it, the text
/// still judged first, so a malformed path says so.
fn root_for<'a>(it: &'a Interp, path: &str) -> Result<&'a Path, MError> {
    match &it.file_root {
        Some(root) => Ok(root),
        None => {
            files::normalise(path)?;
            Err(error::files_outside_root(path))
        }
    }
}

/// `files`: one folder of the file root, at most [`files::MAX_ENTRIES`]
/// entries of it.
fn list_files(it: &Interp, id: Json, path: &str) -> Result<Json, MError> {
    let root = root_for(it, path)?;
    let listing = files::list(root, path, files::MAX_ENTRIES)?;
    let entries = listing
        .entries
        .into_iter()
        .map(|e| {
            let size = match e.size {
                Some(n) => Json::Number(n as f64),
                None => Json::Null,
            };
            Json::object([
                ("name", Json::String(e.name)),
                ("dir", Json::Bool(e.dir)),
                ("size", size),
            ])
        })
        .collect();
    Ok(Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("root", Json::String(listing.root)),
        ("path", Json::String(listing.path)),
        ("entries", Json::Array(entries)),
        ("truncated", Json::Bool(listing.truncated)),
    ]))
}

/// `history`: the entries of the history file at `path`, the one the
/// terminal's line editor reads, oldest first; none when there is no file
/// or no path at all.
fn history_items(id: Json, path: Option<&Path>) -> Json {
    items_answer(id, path.map(history::load).unwrap_or_default())
}

/// `history_add`: the entry is appended to the history file at `path` when
/// [`history::remember`] keeps it against the file's entries. No history
/// path at all adds nothing; a file that cannot be written is refused with
/// no operating-system text, so the answer does not depend on the platform.
fn history_add(id: Json, path: Option<&Path>, entry: &str) -> Result<Json, MError> {
    let added = match path {
        None => false,
        Some(path) => {
            let mut entries = history::load(path);
            let new = history::remember(&mut entries, entry);
            if new {
                history::append(path, entry).map_err(|_| error::history_not_written())?;
            }
            new
        }
    };
    Ok(Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("added", Json::Bool(added)),
    ]))
}

/// `read_file`: the text of one file under the root, at most
/// [`files::MAX_TEXT`] bytes of it.
fn read_file(it: &Interp, id: Json, path: &str) -> Result<Json, MError> {
    let root = root_for(it, path)?;
    let file = files::read_file(root, path, files::MAX_TEXT)?;
    Ok(Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("path", Json::String(file.path)),
        ("text", Json::String(file.text)),
    ]))
}

/// `write_file`: `text` written to one file under the root, at most
/// [`files::MAX_TEXT`] bytes of it. The interpreter then drops its parse
/// of the file, under whatever path it read it, and every lookup, so a
/// function saved now runs its new text at the next call, however soon.
fn write_file(it: &mut Interp, id: Json, path: &str, text: &str) -> Result<Json, MError> {
    let root = root_for(it, path)?;
    let written = files::write_file(root, path, text, files::MAX_TEXT)?;
    it.file_written(&written.file);
    Ok(Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("path", Json::String(written.path)),
        ("size", Json::Number(written.size as f64)),
    ]))
}

/// `run_file`: one `.m` file under the root run as `run` runs a file named
/// by its full path, answered as `eval` is, its error's `line` `null`,
/// since no code was submitted, and its `stack` always there. A refused
/// path is a refusal like any other, with nothing run.
fn run_file(it: &mut Interp, id: Json, path: &str) -> Result<Json, MError> {
    let root = root_for(it, path)?;
    let target = files::run_file(root, path)?;
    let (result, out) = captured(it, |it| it.run_file(&target.path, &target.name));
    Ok(evaluated(it, id, result, out, false, true))
}

/// `figures`: the open figures' numbers, ascending, and those of them
/// changed since the last `figures`, ascending, which asking forgets.
fn figures(it: &mut Interp, id: Json) -> Json {
    let numbers =
        |ns: Vec<u32>| Json::Array(ns.into_iter().map(|n| Json::Number(f64::from(n))).collect());
    let open = numbers(it.figure_numbers());
    let changed = numbers(it.take_changed_figures());
    Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("open", open),
        ("changed", changed),
    ])
}

/// `figure`'s `n`: required, and a JSON number holding a positive whole
/// number. Anything else, `0`, a negative, a fraction, an infinity, a
/// string, `null` or a boolean, is refused.
fn figure_number(req: &Json) -> Result<f64, MError> {
    match req.get("n") {
        None => Err(error::request_missing("n")),
        Some(Json::Number(n)) if n.is_finite() && *n >= 1.0 && n.fract() == 0.0 => Ok(*n),
        Some(_) => Err(error::request_figure_number()),
    }
}

/// `figure`: figure `n` as SVG text, the text `saveas(n, 'f.svg')` writes,
/// at most `bound` bytes of it. `n` is a positive whole number; one past
/// any figure's, a `u32`'s included, is no open figure. The render's own
/// cost is bounded by the figure's point budget, [`crate::plot::figure::MAX_POINTS`].
fn figure(it: &Interp, id: Json, n: f64, bound: usize) -> Result<Json, MError> {
    // A whole number is written with no decimal point and no exponent:
    // `1e12` is `1000000000000`.
    let shown = n.to_string();
    let svg = (n <= f64::from(u32::MAX))
        .then_some(n as u32)
        .and_then(|k| it.figure_svg(k))
        .ok_or_else(|| error::figure_not_open(&shown))?;
    if svg.len() > bound {
        return Err(error::figure_too_large(&shown));
    }
    Ok(Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("n", Json::Number(n)),
        ("svg", Json::String(svg)),
    ]))
}

/// `cwd`: with no `path`, the current folder relative to the root; with
/// one, a string judged as `files` judges a folder, the current folder
/// moved there first, as `cd` moves it. The answer is [`Interp::cwd`]
/// relative to the root with `/` separators, judged on its canonical path:
/// `""` at the root, and `null` should it lie outside, which `cd`'s
/// confinement prevents, or be gone.
fn cwd(it: &mut Interp, id: Json, req: &Json) -> Result<Json, MError> {
    match req.get("path") {
        None => {}
        Some(Json::String(path)) => {
            let root = root_for(it, path)?;
            let folder = files::current_folder(root, path)?;
            it.enter_folder(folder);
        }
        Some(_) => return Err(error::request_not_string("path")),
    }
    let cwd = it
        .file_root
        .as_deref()
        .and_then(|root| files::folder_relative(root, &it.cwd))
        .map_or(Json::Null, Json::String);
    Ok(Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("cwd", cwd),
    ]))
}

/// A string field the operation needs.
fn field<'a>(req: &'a Json, name: &str) -> Result<&'a str, MError> {
    match req.get(name) {
        None => Err(error::request_missing(name)),
        Some(v) => v.as_str().ok_or_else(|| error::request_not_string(name)),
    }
}

/// A line as the protocol writes one: a number, or `null` when none is
/// known.
fn line_json(line: Option<u32>) -> Json {
    match line {
        Some(n) => Json::Number(f64::from(n)),
        None => Json::Null,
    }
}

/// `{"message": ..., "line": ...}`: the message exactly as the REPL prints it
/// after `Error: `, without the `Line N: ` that script mode adds, and the
/// line apart, one-based within the submitted code.
fn error_object(e: &MError) -> Json {
    Json::object([
        ("message", Json::String(e.msg.clone())),
        ("line", line_json(e.line)),
    ])
}

/// The error's frames, innermost first, as the protocol's `stack` writes
/// them: `{file, name, line}`. Each distinct file is canonicalised once,
/// against the root, however many frames name it; a frame with no file,
/// a function local to the submitted code, has `null`, as has one whose
/// file is outside the root.
fn stack_json(it: &Interp, e: &MError) -> Json {
    let mut seen: HashMap<&str, Json> = HashMap::new();
    let frames = e
        .stack()
        .iter()
        .map(|frame| {
            let file = if frame.file.is_empty() {
                Json::Null
            } else {
                seen.entry(frame.file.as_str())
                    .or_insert_with(|| {
                        it.file_root
                            .as_deref()
                            .and_then(|root| files::relative(root, Path::new(&frame.file)))
                            .map_or(Json::Null, Json::String)
                    })
                    .clone()
            };
            Json::object([
                ("file", file),
                ("name", Json::String(frame.name.clone())),
                ("line", line_json(frame.line)),
            ])
        })
        .collect();
    Json::Array(frames)
}

/// The answer to a request that could not be acted on.
fn refused(id: Json, e: MError) -> Json {
    Json::object([
        ("id", id),
        ("ok", Json::Bool(false)),
        ("error", error_object(&e)),
    ])
}

/// A shared byte buffer, so the output can be read back once the
/// interpreter's boxed writer is swapped out again.
struct Capture(Rc<RefCell<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Runs `code` as a REPL entry is run, with its output captured. The code is
/// run as sent, even when a block is left open: that is a parse error in the
/// answer, not a wait for more input, and a client asks `complete` first.
/// With `stack` the error carries its frames after its line; without it
/// the answer is U0's, byte for byte.
fn eval(it: &mut Interp, id: Json, code: &str, stack: bool) -> Json {
    let (result, out) = captured(it, |it| it.run_command(code));
    evaluated(it, id, result, out, true, stack)
}

/// Runs `run` with the interpreter's output and warnings captured, and
/// `input` refused, for the length of the call; the result and what it
/// wrote.
fn captured(it: &mut Interp, run: impl FnOnce(&mut Interp) -> R<()>) -> (R<()>, Json) {
    let buf = Rc::new(RefCell::new(Vec::new()));
    // Both sinks write into the one buffer, so a warning sits in `out` where
    // it was raised, between the output before it and the output after, and
    // nothing reaches stderr.
    let saved = std::mem::replace(&mut it.out, Box::new(Capture(buf.clone())));
    let saved_err = std::mem::replace(&mut it.err, Box::new(Capture(buf.clone())));
    // Standard input is the protocol's channel, or under `--ui` and
    // `--http-stdio` there is no terminal at all: `input` is refused for
    // the length of the call rather than swallow the next request (cycle
    // 11).
    let saved_input = std::mem::replace(&mut it.input, InputSource::Refused);
    let result = run(it);
    it.out = saved;
    it.err = saved_err;
    it.input = saved_input;
    let out = Json::String(String::from_utf8_lossy(&buf.borrow()).into_owned());
    (result, out)
}

/// The answer to an evaluation, `eval`'s or `run_file`'s: `out`, then the
/// `error` when it failed, whose `line` is written when `line` is set and
/// `null` otherwise, and which has its `stack` after the line when `stack`
/// is set.
fn evaluated(it: &Interp, id: Json, result: R<()>, out: Json, line: bool, stack: bool) -> Json {
    let e = match result {
        Ok(()) => return Json::object([("id", id), ("ok", Json::Bool(true)), ("out", out)]),
        Err(e) => e,
    };
    let mut error = vec![
        ("message".to_string(), Json::String(e.msg.clone())),
        (
            "line".to_string(),
            line_json(if line { e.line } else { None }),
        ),
    ];
    if stack {
        error.push(("stack".to_string(), stack_json(it, &e)));
    }
    Json::object([
        ("id", id),
        ("ok", Json::Bool(false)),
        ("out", out),
        ("error", Json::Object(error)),
    ])
}

/// One `{name, size, class}` per variable, sorted by name, and with
/// `preview` a `value` after `class`: [`env::preview`] in the session's
/// display format. `size` holds every dimension, `[2,3,4]` for an N-D
/// array (cycle 14).
fn workspace(it: &Interp, id: Json, preview: bool) -> Json {
    let mut names: Vec<&String> = it.vars().keys().collect();
    names.sort();
    let vars = names
        .into_iter()
        .map(|name| {
            let v = &it.vars()[name];
            let size = v.dims().iter().map(|&d| Json::Number(d as f64)).collect();
            let mut pairs = vec![
                ("name".to_string(), Json::String(name.clone())),
                ("size".to_string(), Json::Array(size)),
                (
                    "class".to_string(),
                    Json::String(v.class_name().to_string()),
                ),
            ];
            if preview {
                pairs.push((
                    "value".to_string(),
                    Json::String(env::preview(v, it.format)),
                ));
            }
            Json::Object(pairs)
        })
        .collect();
    Json::object([
        ("id", id),
        ("ok", Json::Bool(true)),
        ("vars", Json::Array(vars)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serves `input` in a fresh session and returns the response lines.
    fn session(input: &str) -> Vec<String> {
        let mut out = Vec::new();
        serve(input.as_bytes(), &mut out).expect("in-memory I/O cannot fail");
        String::from_utf8(out)
            .expect("responses are UTF-8")
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn one(input: &str) -> String {
        let lines = session(input);
        assert_eq!(lines.len(), 1, "{lines:?}");
        lines.into_iter().next().unwrap()
    }

    #[test]
    fn eval_captures_display_and_the_session_persists() {
        let got = session(
            "{\"id\":1,\"op\":\"eval\",\"code\":\"x = 1 + 2\"}\n\
             {\"id\":2,\"op\":\"eval\",\"code\":\"disp(x * 2)\"}\n",
        );
        assert_eq!(
            got,
            [
                "{\"id\":1,\"ok\":true,\"out\":\"x =\\n\\n     3\\n\\n\"}",
                "{\"id\":2,\"ok\":true,\"out\":\"     6\\n\"}",
            ]
        );
    }

    #[test]
    fn an_eval_error_is_an_answer_with_its_line_and_the_output_before_it() {
        let got = session(
            "{\"id\":3,\"op\":\"eval\",\"code\":\"disp(1)\\ny = nosuchname\"}\n\
             {\"id\":4,\"op\":\"eval\",\"code\":\"z = 5;\"}\n",
        );
        assert_eq!(
            got,
            [
                "{\"id\":3,\"ok\":false,\"out\":\"     1\\n\",\"error\":{\"message\":\
                 \"Unrecognized function or variable 'nosuchname'.\",\"line\":2}}",
                "{\"id\":4,\"ok\":true,\"out\":\"\"}",
            ]
        );
    }

    #[test]
    fn variables_assigned_before_an_error_survive_it() {
        let got = session(
            "{\"id\":5,\"op\":\"eval\",\"code\":\"a = 7; b = a(3)\"}\n\
             {\"id\":6,\"op\":\"eval\",\"code\":\"disp(a)\"}\n",
        );
        assert!(got[0].starts_with("{\"id\":5,\"ok\":false,\"out\":\"\",\"error\":"));
        assert!(got[0].ends_with(",\"line\":1}}"));
        assert_eq!(got[1], "{\"id\":6,\"ok\":true,\"out\":\"     7\\n\"}");
    }

    #[test]
    fn an_unclosed_block_is_run_as_sent_and_fails() {
        let got = one("{\"id\":16,\"op\":\"eval\",\"code\":\"for k = 1:3\"}");
        assert_eq!(
            got,
            "{\"id\":16,\"ok\":false,\"out\":\"\",\"error\":{\"message\":\
             \"expected 'end' but found end of input\",\"line\":1}}"
        );
    }

    /// Cycle 04's acceptance test 17: a warning is part of `out`, where it
    /// was raised, and the error sink is put back afterwards.
    #[test]
    fn a_warning_is_captured_in_out_in_order() {
        let got = one("{\"id\":1,\"op\":\"eval\",\"code\":\"disp(0); warning('w'); disp(1)\"}");
        assert_eq!(
            got,
            "{\"id\":1,\"ok\":true,\"out\":\"     0\\nWarning: w\\n     1\\n\"}"
        );
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        let r = respond(&mut it, "{\"op\":\"eval\",\"code\":\"warning('a')\"}");
        assert_eq!(r.get("out"), Some(&Json::String("Warning: a\n".into())));
        let r = respond(&mut it, "{\"op\":\"eval\",\"code\":\"disp(2)\"}");
        assert_eq!(r.get("out"), Some(&Json::String("     2\n".into())));
    }

    #[test]
    fn eval_restores_the_output_sink() {
        let mut it = Interp::with_output(Box::new(io::sink()));
        let r = respond(&mut it, "{\"op\":\"eval\",\"code\":\"disp(1)\"}");
        assert_eq!(r.get("out"), Some(&Json::String("     1\n".into())));
        // The next evaluation starts from an empty buffer, not the last one.
        let r = respond(&mut it, "{\"op\":\"eval\",\"code\":\"disp(2)\"}");
        assert_eq!(r.get("out"), Some(&Json::String("     2\n".into())));
    }

    #[test]
    fn complete_answers_the_repl_question() {
        let got = session(
            "{\"id\":2,\"op\":\"complete\",\"code\":\"for k = 1:3\"}\n\
             {\"id\":3,\"op\":\"complete\",\"code\":\"for k = 1:3\\ndisp(k)\\nend\"}\n",
        );
        assert_eq!(
            got,
            [
                "{\"id\":2,\"ok\":true,\"complete\":false}",
                "{\"id\":3,\"ok\":true,\"complete\":true}",
            ]
        );
    }

    #[test]
    fn workspace_lists_name_size_and_class_sorted_by_name() {
        let got = session(
            "{\"op\":\"eval\",\"code\":\"x = 1:3; s = 'ab'; t = true;\"}\n\
             {\"id\":7,\"op\":\"workspace\"}\n",
        );
        assert_eq!(
            got,
            [
                "{\"id\":null,\"ok\":true,\"out\":\"\"}",
                "{\"id\":7,\"ok\":true,\"vars\":[\
                 {\"name\":\"s\",\"size\":[1,2],\"class\":\"char\"},\
                 {\"name\":\"t\",\"size\":[1,1],\"class\":\"logical\"},\
                 {\"name\":\"x\",\"size\":[1,3],\"class\":\"double\"}]}",
            ]
        );
        assert_eq!(
            one("{\"id\":\"abc\",\"op\":\"workspace\"}"),
            "{\"id\":\"abc\",\"ok\":true,\"vars\":[]}"
        );
    }

    #[test]
    fn completions_merge_variables_and_builtins_once() {
        let got = session(
            "{\"id\":8,\"op\":\"completions\",\"prefix\":\"dis\"}\n\
             {\"op\":\"eval\",\"code\":\"display_count = 1; disp = 3;\"}\n\
             {\"id\":8,\"op\":\"completions\",\"prefix\":\"dis\"}\n\
             {\"id\":9,\"op\":\"completions\",\"prefix\":\"disp\"}\n",
        );
        assert_eq!(got[0], "{\"id\":8,\"ok\":true,\"items\":[\"disp\"]}");
        assert_eq!(
            got[2],
            "{\"id\":8,\"ok\":true,\"items\":[\"disp\",\"display_count\"]}"
        );
        assert_eq!(
            got[3],
            "{\"id\":9,\"ok\":true,\"items\":[\"disp\",\"display_count\"]}"
        );
    }

    #[test]
    fn malformed_lines_are_answered_and_the_session_goes_on() {
        let refused = |id: &str, msg: &str| {
            format!(
                "{{\"id\":{id},\"ok\":false,\"error\":{{\"message\":\"{msg}\",\"line\":null}}}}"
            )
        };
        let good = "{\"id\":99,\"op\":\"eval\",\"code\":\"disp(1)\"}";
        let good_reply = "{\"id\":99,\"ok\":true,\"out\":\"     1\\n\"}";
        for (line, id, msg) in [
            ("not json", "null", "Malformed request: not valid JSON."),
            ("[1, 2]", "null", "Malformed request: not a JSON object."),
            ("{\"id\":10}", "10", "Malformed request: no 'op' field."),
            (
                "{\"id\":11,\"op\":\"fly\"}",
                "11",
                "Unknown operation 'fly'.",
            ),
            (
                "{\"id\":12,\"op\":\"eval\"}",
                "12",
                "Malformed request: no 'code' field.",
            ),
            (
                "{\"id\":13,\"op\":7}",
                "13",
                "Malformed request: 'op' must be a string.",
            ),
            (
                "{\"id\":14,\"op\":\"eval\",\"code\":[]}",
                "14",
                "Malformed request: 'code' must be a string.",
            ),
            (
                "{\"id\":15,\"op\":\"completions\"}",
                "15",
                "Malformed request: no 'prefix' field.",
            ),
            (
                "{\"id\":[1],\"op\":\"workspace\"}",
                "null",
                "Malformed request: 'id' must be a number or a string.",
            ),
            (
                "{\"op\":\"fly\"} trailing",
                "null",
                "Malformed request: not valid JSON.",
            ),
        ] {
            let got = session(&format!("{line}\n{good}\n"));
            assert_eq!(got, [refused(id, msg), good_reply.to_string()], "{line}");
        }
    }

    #[test]
    fn a_request_nested_far_past_the_limit_is_malformed_not_an_overflow() {
        let got = one(&"[".repeat(100_000));
        assert_eq!(
            got,
            "{\"id\":null,\"ok\":false,\"error\":{\"message\":\
             \"Malformed request: nested too deeply.\",\"line\":null}}"
        );
    }

    #[test]
    fn blank_lines_and_carriage_returns_are_skipped() {
        let got = session(
            "\n\r\n   \n{\"id\":1,\"op\":\"workspace\"}\r\n\n{\"id\":2,\"op\":\"workspace\"}",
        );
        assert_eq!(
            got,
            [
                "{\"id\":1,\"ok\":true,\"vars\":[]}",
                "{\"id\":2,\"ok\":true,\"vars\":[]}",
            ]
        );
        assert!(session("").is_empty());
    }

    #[test]
    fn output_is_escaped_and_non_ascii_is_raw() {
        let got = session(
            "{\"id\":13,\"op\":\"eval\",\"code\":\"disp('a\\\"b\\\\c')\"}\n\
             {\"id\":14,\"op\":\"eval\",\"code\":\"fprintf('x\\\\ty\\\\n')\"}\n\
             {\"id\":15,\"op\":\"eval\",\"code\":\"v = true(1, 2)\"}\n",
        );
        assert_eq!(got[0], "{\"id\":13,\"ok\":true,\"out\":\"a\\\"b\\\\c\\n\"}");
        assert_eq!(got[1], "{\"id\":14,\"ok\":true,\"out\":\"x\\ty\\n\"}");
        assert!(got[2].contains("1×2 logical array"), "{}", got[2]);
    }

    #[test]
    fn ids_are_echoed_as_given() {
        assert!(
            one("{\"id\":\"abc\",\"op\":\"workspace\"}").starts_with("{\"id\":\"abc\",\"ok\":true")
        );
        assert!(one("{\"id\":2.5,\"op\":\"workspace\"}").starts_with("{\"id\":2.5,"));
        assert!(one("{\"id\":null,\"op\":\"workspace\"}").starts_with("{\"id\":null,"));
        assert!(one("{\"op\":\"workspace\"}").starts_with("{\"id\":null,"));
    }

    /// Cycle 13: `exit` and `quit` are statements now, and the session
    /// belongs to the client, so they are a clean error in an `eval` and
    /// the session answers the next request.
    #[test]
    fn exit_is_refused_in_an_eval_and_the_session_goes_on() {
        let lines = session(concat!(
            "{\"id\":1,\"op\":\"eval\",\"code\":\"exit\"}\n",
            "{\"id\":2,\"op\":\"eval\",\"code\":\"try, quit(2), catch e, disp(1), end\"}\n",
            "{\"id\":3,\"op\":\"eval\",\"code\":\"disp(3)\"}\n",
        ));
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[0].contains("\"ok\":false"), "{}", lines[0]);
        assert!(
            lines[0].contains("exit is not available here: the client ends the session."),
            "{}",
            lines[0]
        );
        assert_eq!(lines[1], "{\"id\":2,\"ok\":true,\"out\":\"     1\\n\"}");
        assert_eq!(lines[2], "{\"id\":3,\"ok\":true,\"out\":\"     3\\n\"}");
    }

    /// Cycle U2: `preview` adds `value` after `class`; absent or `false`
    /// gives U0's bytes exactly; anything but a boolean is refused.
    #[test]
    fn workspace_preview_adds_value_after_class() {
        let got = session(concat!(
            "{\"op\":\"eval\",\"code\":\"x = 3; s = 'it''s';\"}\n",
            "{\"id\":1,\"op\":\"workspace\",\"preview\":true}\n",
            "{\"id\":2,\"op\":\"workspace\",\"preview\":false}\n",
            "{\"id\":3,\"op\":\"workspace\"}\n",
            "{\"id\":4,\"op\":\"workspace\",\"preview\":1}\n",
            "{\"id\":5,\"op\":\"workspace\",\"preview\":null}\n",
            "{\"id\":6,\"op\":\"workspace\",\"preview\":\"yes\"}\n",
        ));
        assert_eq!(
            got[1],
            "{\"id\":1,\"ok\":true,\"vars\":[\
             {\"name\":\"s\",\"size\":[1,4],\"class\":\"char\",\"value\":\"'it''s'\"},\
             {\"name\":\"x\",\"size\":[1,1],\"class\":\"double\",\"value\":\"3\"}]}"
        );
        let plain = "\"ok\":true,\"vars\":[\
                     {\"name\":\"s\",\"size\":[1,4],\"class\":\"char\"},\
                     {\"name\":\"x\",\"size\":[1,1],\"class\":\"double\"}]}";
        assert_eq!(got[2], format!("{{\"id\":2,{plain}"));
        assert_eq!(got[3], format!("{{\"id\":3,{plain}"));
        for (k, id) in (4..=6).enumerate() {
            assert_eq!(
                got[4 + k],
                format!(
                    "{{\"id\":{id},\"ok\":false,\"error\":{{\"message\":\
                     \"Malformed request: 'preview' must be true or false.\",\"line\":null}}}}"
                )
            );
        }
    }

    /// Cycle 14: `size` holds every dimension of an N-D array, and the
    /// preview names them.
    #[test]
    fn workspace_answers_every_dimension() {
        let got = session(concat!(
            "{\"op\":\"eval\",\"code\":\"A = zeros(2, 3, 4);\"}\n",
            "{\"id\":1,\"op\":\"workspace\",\"preview\":true}\n",
        ));
        assert_eq!(
            got[1],
            "{\"id\":1,\"ok\":true,\"vars\":[\
             {\"name\":\"A\",\"size\":[2,3,4],\"class\":\"double\",\"value\":\"2×3×4 double\"}]}"
        );
    }

    /// The preview is made in the session's display format.
    #[test]
    fn a_preview_follows_format() {
        let got = session(concat!(
            "{\"op\":\"eval\",\"code\":\"p = pi; format long\"}\n",
            "{\"id\":1,\"op\":\"workspace\",\"preview\":true}\n",
        ));
        assert!(
            got[1].contains("\"value\":\"3.141592653589793\""),
            "{}",
            got[1]
        );
    }

    /// A folder under the temporary folder, removed when dropped.
    struct Dir(std::path::PathBuf);

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(name: &str) -> Dir {
        let d = std::env::temp_dir().join(format!(
            "splatcrab-protocol-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    /// Cycle U2: `files` answers `root`, `path`, `entries` and `truncated`
    /// after `id` and `ok`, each entry `name`, `dir` and `size`, and every
    /// refusal is an answer with `"line":null`.
    #[test]
    fn files_answers_its_keys_in_order() {
        let d = scratch("files");
        let root = d.0.join("desk");
        std::fs::create_dir_all(root.join("tree").join("sub")).unwrap();
        std::fs::write(root.join("tree").join("a.txt"), "hello").unwrap();
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        it.file_root = Some(std::fs::canonicalize(&root).unwrap());
        let ask = |it: &mut Interp, line: &str| respond(it, line).to_string();
        assert_eq!(
            ask(&mut it, "{\"id\":1,\"op\":\"files\",\"path\":\"tree\"}"),
            "{\"id\":1,\"ok\":true,\"root\":\"desk\",\"path\":\"tree\",\"entries\":[\
             {\"name\":\"sub\",\"dir\":true,\"size\":null},\
             {\"name\":\"a.txt\",\"dir\":false,\"size\":5}],\"truncated\":false}"
        );
        assert_eq!(
            ask(&mut it, "{\"id\":2,\"op\":\"files\",\"path\":\"\"}"),
            "{\"id\":2,\"ok\":true,\"root\":\"desk\",\"path\":\"\",\"entries\":[\
             {\"name\":\"tree\",\"dir\":true,\"size\":null}],\"truncated\":false}"
        );
        let refusal = |id: u32, msg: &str| {
            format!(
                "{{\"id\":{id},\"ok\":false,\"error\":{{\"message\":\"{msg}\",\"line\":null}}}}"
            )
        };
        for (id, line, msg) in [
            (3, "\"path\":\"..\"", "Path '..' is outside the file root."),
            (
                4,
                "\"path\":\"tree\\\\sub\"",
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
            (
                5,
                "\"path\":\"tree/a.txt\"",
                "Path 'tree/a.txt' is not a folder.",
            ),
            (
                6,
                "\"path\":3",
                "Malformed request: 'path' must be a string.",
            ),
            (7, "\"x\":1", "Malformed request: no 'path' field."),
        ] {
            let got = ask(&mut it, &format!("{{\"id\":{id},\"op\":\"files\",{line}}}"));
            assert_eq!(got, refusal(id, msg), "{line}");
        }
        // `cd` moves the current folder, never the root.
        ask(&mut it, "{\"op\":\"eval\",\"code\":\"cd tree\"}");
        assert!(
            ask(&mut it, "{\"id\":8,\"op\":\"files\",\"path\":\"tree/sub\"}")
                .starts_with("{\"id\":8,\"ok\":true,\"root\":\"desk\",\"path\":\"tree/sub\"")
        );
        // With no root at all, every well-formed path is outside it.
        let mut bare = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        assert_eq!(
            ask(&mut bare, "{\"id\":9,\"op\":\"files\",\"path\":\"\"}"),
            refusal(9, "Path '' is outside the file root.")
        );
        assert_eq!(
            ask(&mut bare, "{\"id\":10,\"op\":\"files\",\"path\":\"/x\"}"),
            refusal(
                10,
                "Malformed request: 'path' must be a relative path with '/' separators."
            )
        );
    }

    /// Cycle U2: `history` and `history_add` over a file of the test's own,
    /// never the user's: the answers' keys, `remember`'s rule, and a
    /// multi-line entry kept whole.
    #[test]
    fn history_and_history_add_follow_remember() {
        let d = scratch("history");
        let path = d.0.join("history");
        let id = || Json::Number(1.0);
        let items = |p: Option<&Path>| history_items(id(), p).to_string();
        let add = |entry: &str| history_add(id(), Some(&path), entry).unwrap().to_string();
        assert_eq!(items(Some(&path)), "{\"id\":1,\"ok\":true,\"items\":[]}");
        assert_eq!(add("x = 1"), "{\"id\":1,\"ok\":true,\"added\":true}");
        assert_eq!(add("x = 1"), "{\"id\":1,\"ok\":true,\"added\":false}");
        assert_eq!(add("   "), "{\"id\":1,\"ok\":true,\"added\":false}");
        assert_eq!(add("a\nb"), "{\"id\":1,\"ok\":true,\"added\":true}");
        assert_eq!(
            items(Some(&path)),
            "{\"id\":1,\"ok\":true,\"items\":[\"x = 1\",\"a\\nb\"]}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x = 1\na\\nb\n");
        // No history path: nothing to read and nothing added.
        assert_eq!(items(None), "{\"id\":1,\"ok\":true,\"items\":[]}");
        assert_eq!(
            history_add(id(), None, "y").unwrap().to_string(),
            "{\"id\":1,\"ok\":true,\"added\":false}"
        );
    }

    /// Acceptance test 15: a history path that is a folder cannot be
    /// written, and says so with no operating-system text.
    #[test]
    fn history_add_against_a_folder_is_refused() {
        let d = scratch("history-folder");
        let e = history_add(Json::Number(3.0), Some(&d.0), "x = 1").unwrap_err();
        assert_eq!(e.msg, "The history file could not be written.");
        assert_eq!(
            refused(Json::Number(3.0), e).to_string(),
            "{\"id\":3,\"ok\":false,\"error\":{\"message\":\
             \"The history file could not be written.\",\"line\":null}}"
        );
        // Reading a folder is simply no history.
        assert_eq!(
            history_items(Json::Null, Some(&d.0)).to_string(),
            "{\"id\":null,\"ok\":true,\"items\":[]}"
        );
    }

    /// An interpreter whose file root is a fresh folder `desk` holding
    /// `prog/s1.m` and `prog/s2.m`, the spec's fixture, and `crlf.txt`.
    fn editor_session(name: &str) -> (Dir, Interp) {
        let d = scratch(name);
        let root = d.0.join("desk");
        std::fs::create_dir_all(root.join("prog")).unwrap();
        std::fs::write(
            root.join("prog").join("s1.m"),
            "disp('start')\nb = 2;\nhelper(3)\nfunction helper(x)\n  y = x + 1;\n  z = nosuch(y);\nend\n",
        )
        .unwrap();
        std::fs::write(
            root.join("prog").join("s2.m"),
            "q = 1;\nw = undefined_thing + 1;\n",
        )
        .unwrap();
        std::fs::write(root.join("prog").join("crlf.txt"), b"a\r\nb\r\n").unwrap();
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        it.file_root = Some(std::fs::canonicalize(&root).unwrap());
        it.cwd = root;
        (d, it)
    }

    fn ask(it: &mut Interp, line: &str) -> String {
        respond(it, line).to_string()
    }

    /// Cycle U3: `read_file` answers `path` and `text` after `id` and `ok`,
    /// the path normalised and the text exact.
    #[test]
    fn read_file_answers_its_keys_in_order() {
        let (_d, mut it) = editor_session("read-file");
        assert_eq!(
            ask(&mut it, r#"{"id":1,"op":"read_file","path":"prog/s2.m"}"#),
            r#"{"id":1,"ok":true,"path":"prog/s2.m","text":"q = 1;\nw = undefined_thing + 1;\n"}"#
        );
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":2,"op":"read_file","path":"prog/./crlf.txt"}"#
            ),
            r#"{"id":2,"ok":true,"path":"prog/crlf.txt","text":"a\r\nb\r\n"}"#
        );
        for (line, msg) in [
            (r#""path":"..""#, "Path '..' is outside the file root."),
            (
                r#""path":"/etc""#,
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
            (r#""path":"""#, "Path '' is not a file."),
            (r#""path":"prog""#, "Path 'prog' is not a file."),
            (r#""path":"nope.m""#, "Path 'nope.m' is not a file."),
            (r#""x":1"#, "Malformed request: no 'path' field."),
            (
                r#""path":[]"#,
                "Malformed request: 'path' must be a string.",
            ),
        ] {
            let got = ask(&mut it, &format!(r#"{{"id":3,"op":"read_file",{line}}}"#));
            assert_eq!(
                got,
                format!(r#"{{"id":3,"ok":false,"error":{{"message":"{msg}","line":null}}}}"#),
                "{line}"
            );
        }
    }

    /// Cycle U3: `write_file` answers `path` and `size` after `id` and
    /// `ok`; its fields are judged `path` then `text`; and what it writes
    /// is what the next call of the function runs, however soon.
    #[test]
    fn write_file_answers_its_keys_in_order_and_the_next_call_reads_it() {
        let (d, mut it) = editor_session("write-file");
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":1,"op":"write_file","path":"scratch_f.m","text":"function scratch_f\ndisp(1)\nend\n"}"#
            ),
            r#"{"id":1,"ok":true,"path":"scratch_f.m","size":31}"#
        );
        assert_eq!(
            ask(&mut it, r#"{"id":2,"op":"eval","code":"scratch_f"}"#),
            r#"{"id":2,"ok":true,"out":"     1\n"}"#
        );
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":3,"op":"write_file","path":"./scratch_f.m","text":"function scratch_f\ndisp(2)\nend\n"}"#
            ),
            r#"{"id":3,"ok":true,"path":"scratch_f.m","size":31}"#
        );
        assert_eq!(
            ask(&mut it, r#"{"id":4,"op":"eval","code":"scratch_f"}"#),
            r#"{"id":4,"ok":true,"out":"     2\n"}"#
        );
        assert_eq!(
            std::fs::read_to_string(d.0.join("desk").join("scratch_f.m")).unwrap(),
            "function scratch_f\ndisp(2)\nend\n"
        );
        let refusal = |id: u32, msg: &str| {
            format!(r#"{{"id":{id},"ok":false,"error":{{"message":"{msg}","line":null}}}}"#)
        };
        for (id, fields, msg) in [
            (5, r#""text":"x""#, "Malformed request: no 'path' field."),
            (6, "", "Malformed request: no 'path' field."),
            (7, r#""path":"a.m""#, "Malformed request: no 'text' field."),
            (
                8,
                r#""path":"a.m","text":3"#,
                "Malformed request: 'text' must be a string.",
            ),
            (
                9,
                r#""path":3,"text":3"#,
                "Malformed request: 'path' must be a string.",
            ),
            (
                10,
                r#""path":"..","text":"x""#,
                "Path '..' is outside the file root.",
            ),
            (
                11,
                r#""path":"nofolder/x.m","text":"x""#,
                "Path 'nofolder/x.m' is not in a folder of the file root.",
            ),
            (
                12,
                r#""path":"prog","text":"x""#,
                "Path 'prog' is a folder.",
            ),
            (
                13,
                r#""path":"NUL.m","text":"x""#,
                "Path 'NUL.m' is not a file.",
            ),
        ] {
            let sep = if fields.is_empty() { "" } else { "," };
            let got = ask(
                &mut it,
                &format!(r#"{{"id":{id},"op":"write_file"{sep}{fields}}}"#),
            );
            assert_eq!(got, refusal(id, msg), "{fields}");
        }
        assert!(!d.0.join("desk").join("a.m").exists());
    }

    /// Cycle U3: `run_file` answers as `eval` does, its error's `line`
    /// `null` and its `stack` always there, each frame `file`, `name` and
    /// `line`, the file relative to the root; and it runs from the root
    /// wherever `cd` has gone.
    #[test]
    fn run_file_answers_as_eval_with_a_stack() {
        let (_d, mut it) = editor_session("run-file");
        let s1 = concat!(
            r#"{"id":1,"ok":false,"out":"start\n","error":{"message":"#,
            r#""Unrecognized function or variable 'nosuch'.","line":null,"stack":["#,
            r#"{"file":"prog/s1.m","name":"helper","line":6},"#,
            r#"{"file":"prog/s1.m","name":"s1","line":3}]}}"#
        );
        assert_eq!(
            ask(&mut it, r#"{"id":1,"op":"run_file","path":"prog/s1.m"}"#),
            s1
        );
        let s2 = concat!(
            r#"{"id":2,"ok":false,"out":"","error":{"message":"#,
            r#""Unrecognized function or variable 'undefined_thing'.","line":null,"stack":["#,
            r#"{"file":"prog/s2.m","name":"s2","line":2}]}}"#
        );
        assert_eq!(
            ask(&mut it, r#"{"id":2,"op":"run_file","path":"prog/s2.m"}"#),
            s2
        );
        // It ran in the base workspace.
        assert!(it.vars().contains_key("q") && it.vars().contains_key("b"));
        ask(&mut it, r#"{"op":"eval","code":"cd prog"}"#);
        assert_eq!(
            ask(&mut it, r#"{"id":2,"op":"run_file","path":"prog/s2.m"}"#),
            s2
        );
        assert!(it.cwd.ends_with("prog"));
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":3,"op":"run_file","path":"prog/crlf.txt"}"#
            ),
            r#"{"id":3,"ok":false,"error":{"message":"Path 'prog/crlf.txt' is not a .m file.","line":null}}"#
        );
        ask(&mut it, r#"{"op":"eval","code":"cd .."}"#);
        // A clean run, output and a warning captured in order.
        ask(
            &mut it,
            r#"{"op":"write_file","path":"ok.m","text":"disp(7)\nwarning('w')\ndisp(8)\n"}"#,
        );
        assert_eq!(
            ask(&mut it, r#"{"id":4,"op":"run_file","path":"ok.m"}"#),
            r#"{"id":4,"ok":true,"out":"     7\nWarning: w\n     8\n"}"#
        );
        // `input` is refused, as in an `eval`.
        ask(
            &mut it,
            r#"{"op":"write_file","path":"asks.m","text":"v = input('? ');\n"}"#,
        );
        let got = ask(&mut it, r#"{"id":5,"op":"run_file","path":"asks.m"}"#);
        assert!(got.starts_with(r#"{"id":5,"ok":false,"out":"#), "{got}");
        assert!(
            got.ends_with(r#""line":null,"stack":[{"file":"asks.m","name":"asks","line":1}]}}"#),
            "{got}"
        );
    }

    /// Cycle U3: `eval`'s `stack` adds the frames after `line`; without it,
    /// or with `false`, the answer is U0's byte for byte; anything but a
    /// boolean is refused.
    #[test]
    fn eval_with_stack_adds_the_frames_after_line() {
        let (_d, mut it) = editor_session("eval-stack");
        let frames = concat!(
            r#""stack":[{"file":"prog/s1.m","name":"helper","line":6},"#,
            r#"{"file":"prog/s1.m","name":"s1","line":3}]"#
        );
        let head = r#""ok":false,"out":"start\n","error":{"message":"Unrecognized function or variable 'nosuch'.","line":1"#;
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":1,"op":"eval","code":"run('prog/s1.m')","stack":true}"#
            ),
            format!(r#"{{"id":1,{head},{frames}}}}}"#)
        );
        for (id, flag) in [(2, ""), (3, r#","stack":false"#)] {
            assert_eq!(
                ask(
                    &mut it,
                    &format!(r#"{{"id":{id},"op":"eval","code":"run('prog/s1.m')"{flag}}}"#)
                ),
                format!(r#"{{"id":{id},{head}}}}}"#)
            );
        }
        for (id, value) in [(4, "1"), (5, r#""yes""#), (6, "null")] {
            assert_eq!(
                ask(
                    &mut it,
                    &format!(r#"{{"id":{id},"op":"eval","code":"disp(1)","stack":{value}}}"#)
                ),
                format!(
                    r#"{{"id":{id},"ok":false,"error":{{"message":"Malformed request: 'stack' must be true or false.","line":null}}}}"#
                )
            );
        }
        // Nothing ran for a refused flag, and a success has no stack.
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":7,"op":"eval","code":"disp(1)","stack":true}"#
            ),
            r#"{"id":7,"ok":true,"out":"     1\n"}"#
        );
        // No frames, and a frame with no file.
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":8,"op":"eval","code":"x = nosuchname","stack":true}"#
            ),
            r#"{"id":8,"ok":false,"out":"","error":{"message":"Unrecognized function or variable 'nosuchname'.","line":1,"stack":[]}}"#
        );
        assert_eq!(
            ask(
                &mut it,
                r#"{"id":9,"op":"eval","code":"f = @(n) nosuch(n); f(1)","stack":true}"#
            ),
            r#"{"id":9,"ok":false,"out":"","error":{"message":"Unrecognized function or variable 'nosuch'.","line":1,"stack":[{"file":null,"name":"@(n)nosuch(n)","line":null}]}}"#
        );
    }

    /// Cycle U3: a function found on the path is recorded by the path it
    /// was found at, which is not canonical (and on Windows the root is a
    /// verbatim path); the stack names it relative to the root all the
    /// same, and a file outside the root is `null`.
    #[test]
    fn the_stack_names_a_path_function_relative_to_the_root() {
        let (d, mut it) = editor_session("stack-path");
        let root = d.0.join("desk");
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(
            root.join("lib").join("inner.m"),
            "function inner\n\nerror('boom')\nend\n",
        )
        .unwrap();
        let outside = d.0.join("elsewhere");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("outer.m"), "function outer\ninner\nend\n").unwrap();
        // The folders as spelled, not canonical, with a `.` and a `..`.
        it.cwd = root.join("lib").join("..").join(".");
        let got = ask(
            &mut it,
            &format!(
                r#"{{"id":1,"op":"eval","code":"addpath('lib'); addpath('{}'); outer","stack":true}}"#,
                outside.display().to_string().replace('\\', "\\\\")
            ),
        );
        assert!(
            got.ends_with(concat!(
                r#""message":"boom","line":1,"stack":["#,
                r#"{"file":"lib/inner.m","name":"inner","line":3},"#,
                r#"{"file":null,"name":"outer","line":2}]}}"#
            )),
            "{got}"
        );
    }

    /// A refusal as every operation writes one: `"line":null`.
    fn refusal(id: u32, msg: &str) -> String {
        format!(r#"{{"id":{id},"ok":false,"error":{{"message":"{msg}","line":null}}}}"#)
    }

    /// Cycle U4: `figures` answers `open` then `changed` after `id` and
    /// `ok`, ascending, and asking forgets what it answered as changed;
    /// `figure(n)`, a close and `close all` follow cycle 12's rule.
    #[test]
    fn figures_answers_open_then_changed() {
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        let mut ask = |line: &str| respond(&mut it, line).to_string();
        assert_eq!(
            ask(r#"{"id":1,"op":"figures"}"#),
            r#"{"id":1,"ok":true,"open":[],"changed":[]}"#
        );
        ask(r#"{"op":"eval","code":"figure(3); figure(1);"}"#);
        assert_eq!(
            ask(r#"{"id":2,"op":"figures"}"#),
            r#"{"id":2,"ok":true,"open":[1,3],"changed":[1,3]}"#
        );
        assert_eq!(
            ask(r#"{"id":3,"op":"figures"}"#),
            r#"{"id":3,"ok":true,"open":[1,3],"changed":[]}"#
        );
        ask(r#"{"op":"eval","code":"figure(3); plot(1:3); figure(5); close(3);"}"#);
        assert_eq!(
            ask(r#"{"id":"s","op":"figures"}"#),
            r#"{"id":"s","ok":true,"open":[1,5],"changed":[5]}"#
        );
        ask(r#"{"op":"eval","code":"close all"}"#);
        assert_eq!(
            ask(r#"{"op":"figures"}"#),
            r#"{"id":null,"ok":true,"open":[],"changed":[]}"#
        );
    }

    /// Cycle U4: `figure` answers `n` then `svg` after `id` and `ok`; the
    /// empty figure's text is exactly the four lines the spec records;
    /// and `n` is judged by the spec's table.
    #[test]
    fn figure_answers_n_then_svg_and_judges_n() {
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        let mut ask = |line: &str| respond(&mut it, line).to_string();
        ask(r#"{"op":"eval","code":"figure(5);"}"#);
        let empty = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"560\" ",
            "height=\"420\" viewBox=\"0 0 560 420\" font-family=\"Helvetica, Arial, sans-serif\">\n",
            "<rect class=\"figure\" x=\"0\" y=\"0\" width=\"560\" height=\"420\" fill=\"#ffffff\" stroke=\"none\"/>\n",
            "</svg>\n"
        );
        let mut want = String::from(r#"{"id":1,"ok":true,"n":5,"svg":"#);
        json::write_string(&mut want, empty);
        want.push('}');
        assert_eq!(ask(r#"{"id":1,"op":"figure","n":5}"#), want);
        for (id, n, msg) in [
            (2, None, "Malformed request: no 'n' field."),
            (
                3,
                Some("0"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                4,
                Some("-1"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                5,
                Some("1.5"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                6,
                Some(r#""5""#),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                7,
                Some("null"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                8,
                Some("true"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                9,
                Some("[5]"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                10,
                Some("-0"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (
                11,
                Some("1e400"),
                "Malformed request: 'n' must be a figure number.",
            ),
            (12, Some("7"), "Figure 7 is not open."),
            (13, Some("1e12"), "Figure 1000000000000 is not open."),
            (14, Some("4294967295"), "Figure 4294967295 is not open."),
            (15, Some("4294967296"), "Figure 4294967296 is not open."),
            (
                16,
                Some("1e300"),
                &format!("Figure 1{} is not open.", "0".repeat(300)),
            ),
        ] {
            let field = n.map_or(String::new(), |n| format!(r#","n":{n}"#));
            assert_eq!(
                ask(&format!(r#"{{"id":{id},"op":"figure"{field}}}"#)),
                refusal(id, msg),
                "{n:?}"
            );
        }
        // A whole number written with a decimal point or an exponent names
        // the same figure.
        assert!(
            ask(r#"{"id":17,"op":"figure","n":5.0}"#).starts_with(r#"{"id":17,"ok":true,"n":5,"#)
        );
        assert!(
            ask(r#"{"id":18,"op":"figure","n":0.5e1}"#).starts_with(r#"{"id":18,"ok":true,"n":5,"#)
        );
    }

    /// Acceptance test 8: a plotted figure's `svg` is the very text
    /// `saveas` writes, and holds its title.
    #[test]
    fn a_plotted_figures_svg_is_what_saveas_writes() {
        let d = scratch("figure-saveas");
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        it.cwd = d.0.clone();
        let got = respond(
            &mut it,
            r#"{"op":"eval","code":"plot(1:4, [3 1 4 1], 'r--o'); title('Crab & co'); xlabel('t'); saveas(1, 'f.svg')"}"#,
        )
        .to_string();
        assert_eq!(got, r#"{"id":null,"ok":true,"out":""}"#);
        let saved = std::fs::read_to_string(d.0.join("f.svg")).unwrap();
        let r = respond(&mut it, r#"{"id":1,"op":"figure","n":1}"#);
        assert_eq!(r.get("n"), Some(&Json::Number(1.0)));
        let svg = r.get("svg").and_then(Json::as_str).expect("an svg");
        assert_eq!(svg, saved);
        assert!(svg.contains(">Crab &amp; co<"), "{svg}");
        assert!(svg.contains("<polyline"), "{svg}");
        assert!(svg.starts_with("<?xml "));
    }

    /// Acceptance test 8: an SVG text past the bound is refused, and one
    /// exactly at it answered; the bound is a parameter, so a figure of a
    /// few kilobytes reaches it.
    #[test]
    fn the_inline_bound_holds_at_and_past_its_length() {
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        respond(&mut it, r#"{"op":"eval","code":"figure(2); plot(1:100);"}"#);
        let len = it.figure_svg(2).expect("figure 2 is open").len();
        assert!(len > 1000, "{len}");
        let at = figure(&it, Json::Number(1.0), 2.0, len)
            .unwrap()
            .to_string();
        assert!(
            at.starts_with(r#"{"id":1,"ok":true,"n":2,"svg":"<?xml "#),
            "{at}"
        );
        let past = figure(&it, Json::Number(1.0), 2.0, len - 1).unwrap_err();
        assert_eq!(
            refused(Json::Number(1.0), past).to_string(),
            refusal(
                1,
                "Figure 2 is too large to show inline; save it with saveas."
            )
        );
        // The protocol's own bound is 32 MiB.
        assert_eq!(MAX_INLINE_SVG, 33_554_432);
        assert!(
            respond(&mut it, r#"{"op":"figure","n":2}"#)
                .get("svg")
                .is_some()
        );
    }

    /// A root `U4` under a scratch folder, holding `tree/a.txt` and
    /// `tree/sub/deep.txt`, the spec's fixture, as a client mode fixes it,
    /// and the current folder there as the process's working directory
    /// would be spelled.
    fn folder_session(name: &str) -> (Dir, Interp) {
        let d = scratch(name);
        let root = d.0.join("U4");
        std::fs::create_dir_all(root.join("tree").join("sub")).unwrap();
        std::fs::write(root.join("tree").join("a.txt"), "ab").unwrap();
        std::fs::write(root.join("tree").join("sub").join("deep.txt"), "abc").unwrap();
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        it.file_root = Some(std::fs::canonicalize(&root).unwrap());
        it.cwd = root;
        (d, it)
    }

    /// Cycle U4: `cwd` answers `cwd` after `id` and `ok`; with a path it
    /// moves the current folder, judged as `files` judges a folder, and a
    /// refusal leaves the folder where it was; `cd` in an `eval` moves
    /// what `cwd` answers, and cannot leave the root.
    #[test]
    fn cwd_answers_and_moves_the_current_folder() {
        let (_d, mut it) = folder_session("cwd");
        let mut ask = |line: &str| respond(&mut it, line).to_string();
        assert_eq!(
            ask(r#"{"id":1,"op":"cwd"}"#),
            r#"{"id":1,"ok":true,"cwd":""}"#
        );
        assert_eq!(
            ask(r#"{"id":2,"op":"cwd","path":"tree"}"#),
            r#"{"id":2,"ok":true,"cwd":"tree"}"#
        );
        assert_eq!(
            ask(r#"{"id":3,"op":"eval","code":"ls"}"#),
            r#"{"id":3,"ok":true,"out":"a.txt\nsub\n"}"#
        );
        assert_eq!(
            ask(r#"{"id":4,"op":"cwd","path":"tree/./sub/"}"#),
            r#"{"id":4,"ok":true,"cwd":"tree/sub"}"#
        );
        for (id, field, msg) in [
            (5, r#""path":"..""#, "Path '..' is outside the file root."),
            (
                6,
                r#""path":"tree/a.txt""#,
                "Path 'tree/a.txt' is not a folder.",
            ),
            (7, r#""path":"nope""#, "Path 'nope' is not a folder."),
            (
                8,
                r#""path":"/etc""#,
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
            (
                9,
                r#""path":"tree\\sub""#,
                "Malformed request: 'path' must be a relative path with '/' separators.",
            ),
            (
                10,
                r#""path":3"#,
                "Malformed request: 'path' must be a string.",
            ),
            (
                11,
                r#""path":null"#,
                "Malformed request: 'path' must be a string.",
            ),
        ] {
            assert_eq!(
                ask(&format!(r#"{{"id":{id},"op":"cwd",{field}}}"#)),
                refusal(id, msg),
                "{field}"
            );
            assert_eq!(
                ask(r#"{"id":12,"op":"cwd"}"#),
                r#"{"id":12,"ok":true,"cwd":"tree/sub"}"#,
                "unmoved by {field}"
            );
        }
        assert_eq!(
            ask(r#"{"id":13,"op":"cwd","path":""}"#),
            r#"{"id":13,"ok":true,"cwd":""}"#
        );
        // `cd` moves what `cwd` answers, and stops at the root.
        assert_eq!(
            ask(r#"{"id":14,"op":"eval","code":"cd .."}"#),
            r#"{"id":14,"ok":false,"out":"","error":{"message":"Cannot CD to ..: it is outside the file root.","line":1}}"#
        );
        ask(r#"{"op":"eval","code":"cd tree"}"#);
        assert_eq!(
            ask(r#"{"id":15,"op":"cwd"}"#),
            r#"{"id":15,"ok":true,"cwd":"tree"}"#
        );
        assert_eq!(
            ask(r#"{"id":16,"op":"eval","code":"cd ../.."}"#),
            r#"{"id":16,"ok":false,"out":"","error":{"message":"Cannot CD to ../..: it is outside the file root.","line":1}}"#
        );
        assert_eq!(
            ask(r#"{"id":17,"op":"eval","code":"cd nope"}"#),
            r#"{"id":17,"ok":false,"out":"","error":{"message":"Cannot CD to nope (Name is nonexistent or not a directory).","line":1}}"#
        );
        ask(r#"{"op":"eval","code":"cd .."}"#);
        assert_eq!(
            ask(r#"{"id":18,"op":"cwd"}"#),
            r#"{"id":18,"ok":true,"cwd":""}"#
        );
    }

    /// `cwd` with no root, which only a test builds: the folder is outside
    /// any root, so `null`, and a path is judged on its text and then
    /// refused as outside, as `files` refuses it. A current folder that is
    /// gone is `null` too.
    #[test]
    fn cwd_without_a_root_or_with_the_folder_gone() {
        let mut bare = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        assert_eq!(
            respond(&mut bare, r#"{"id":1,"op":"cwd"}"#).to_string(),
            r#"{"id":1,"ok":true,"cwd":null}"#
        );
        assert_eq!(
            respond(&mut bare, r#"{"id":2,"op":"cwd","path":"tree"}"#).to_string(),
            refusal(2, "Path 'tree' is outside the file root.")
        );
        assert_eq!(
            respond(&mut bare, r#"{"id":3,"op":"cwd","path":"/x"}"#).to_string(),
            refusal(
                3,
                "Malformed request: 'path' must be a relative path with '/' separators."
            )
        );
        let (d, mut it) = folder_session("cwd-gone");
        respond(&mut it, r#"{"op":"cwd","path":"tree/sub"}"#);
        std::fs::remove_dir_all(d.0.join("U4").join("tree").join("sub")).unwrap();
        assert_eq!(
            respond(&mut it, r#"{"id":4,"op":"cwd"}"#).to_string(),
            r#"{"id":4,"ok":true,"cwd":null}"#
        );
    }

    /// Acceptance test 8: the folder `cwd` moves to is written in the
    /// root's plain form, so `pwd` afterwards shows what `cd` to the same
    /// folder shows, never the `\\?\` a canonical root has on Windows; and
    /// the move makes a file there callable at once.
    #[test]
    fn pwd_after_cwd_is_the_plain_form_cd_gives() {
        let (d, mut it) = folder_session("cwd-plain");
        let root = it.file_root.clone().unwrap();
        if cfg!(windows) {
            assert!(
                root.to_string_lossy().starts_with(r"\\?\"),
                "{}",
                root.display()
            );
        }
        std::fs::write(
            d.0.join("U4").join("tree").join("sub").join("here.m"),
            "disp(9)\n",
        )
        .unwrap();
        let pwd = |it: &mut Interp| {
            let r = respond(it, r#"{"op":"eval","code":"disp(pwd)"}"#);
            r.get("out")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string()
        };
        respond(&mut it, r#"{"op":"cwd","path":"tree/sub"}"#);
        let moved = pwd(&mut it);
        assert!(!moved.starts_with(r"\\?\"), "{moved}");
        assert!(moved.trim_end().ends_with("sub"), "{moved}");
        assert_eq!(
            respond(&mut it, r#"{"op":"eval","code":"here"}"#).to_string(),
            r#"{"id":null,"ok":true,"out":"     9\n"}"#
        );
        // `cd` up and back down to the same folder shows the same text.
        respond(&mut it, r#"{"op":"eval","code":"cd ../..; cd tree/sub"}"#);
        assert_eq!(pwd(&mut it), moved);
        // And so does the root.
        respond(&mut it, r#"{"op":"cwd","path":""}"#);
        let top = pwd(&mut it);
        assert!(!top.starts_with(r"\\?\"), "{top}");
        assert_eq!(
            std::fs::canonicalize(top.trim_end()).unwrap(),
            root,
            "{top}"
        );
        assert_eq!(it.cwd, std::path::PathBuf::from(top.trim_end()));
    }

    /// `history_add` needs a string `entry`, judged before any file.
    #[test]
    fn history_add_needs_a_string_entry() {
        let got = session(concat!(
            "{\"id\":1,\"op\":\"history_add\"}\n",
            "{\"id\":2,\"op\":\"history_add\",\"entry\":[]}\n",
        ));
        assert_eq!(
            got,
            [
                "{\"id\":1,\"ok\":false,\"error\":{\"message\":\
                 \"Malformed request: no 'entry' field.\",\"line\":null}}",
                "{\"id\":2,\"ok\":false,\"error\":{\"message\":\
                 \"Malformed request: 'entry' must be a string.\",\"line\":null}}",
            ]
        );
    }

    /// Every response is flushed before the next request is read, so a client
    /// that waits for each answer before sending the next line never stalls.
    #[test]
    fn each_response_is_flushed_before_the_next_line_is_read() {
        struct Lines {
            lines: Vec<&'static [u8]>,
            flushed: Rc<RefCell<usize>>,
            reads: usize,
        }
        impl io::Read for Lines {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                unreachable!("read through BufRead only")
            }
        }
        impl BufRead for Lines {
            fn fill_buf(&mut self) -> io::Result<&[u8]> {
                // By the time line k is asked for, k responses are flushed.
                assert_eq!(*self.flushed.borrow(), self.reads);
                Ok(self.lines.first().copied().unwrap_or(b""))
            }
            fn consume(&mut self, n: usize) {
                if n > 0 {
                    self.lines.remove(0);
                    self.reads += 1;
                }
            }
        }
        struct Counting(Rc<RefCell<usize>>, Vec<u8>);
        impl Write for Counting {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.1.extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                *self.0.borrow_mut() += 1;
                Ok(())
            }
        }
        let flushed = Rc::new(RefCell::new(0));
        let input = Lines {
            lines: vec![b"{\"op\":\"workspace\"}\n", b"{\"op\":\"workspace\"}\n"],
            flushed: flushed.clone(),
            reads: 0,
        };
        serve(input, Counting(flushed.clone(), Vec::new())).unwrap();
        assert_eq!(*flushed.borrow(), 2);
    }
}
