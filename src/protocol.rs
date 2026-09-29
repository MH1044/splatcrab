//! The evaluation protocol: `splatcrab --protocol`.
//!
//! JSON Lines over any reader and writer. Each line read is one request, a
//! JSON object; each line written is its response, one JSON object, flushed
//! before the next request is read so that a client can interleave. All of
//! them run against one [`Interp`], so a variable assigned by one `eval` is
//! there for the next. The operations, their fields and the order of the keys
//! in each response are fixed by `docs/modules/U0-ui-foundations.md` and,
//! for the desktop's three and `workspace`'s `preview`,
//! `docs/modules/U2-ui-desktop.md`:
//!
//! | `op` | needs | answers, after `id` and `ok` |
//! |---|---|---|
//! | `eval` | `code` | `out`, then `error` when it failed |
//! | `complete` | `code` | `complete`, [`syntax::is_complete`] |
//! | `workspace` | `preview`, optional | `vars`, one `{name, size, class}` per variable, and `value`, [`env::preview`], after `class` when `preview` is `true` |
//! | `completions` | `prefix` | `items`, [`env::completions`] |
//! | `files` | `path` | `root`, `path`, `entries` (`{name, dir, size}`) and `truncated`, [`files::list`] under [`Interp::file_root`] |
//! | `history` | | `items`, the history file's entries, [`history::load`] |
//! | `history_add` | `entry` | `added`, whether [`history::remember`] kept it and it was appended |
//!
//! A request that cannot be acted on is answered with `"ok":false` and an
//! `error` whose `line` is `null`, and the loop goes on: a session is not a
//! script, and neither a failed evaluation nor a malformed line ends it.
//!
//! Nothing here writes anywhere but the writer it is handed. The interpreter
//! is built over two sinks, and `eval` swaps its `out` and its `err` for one
//! buffer for the length of the call, so evaluation output and warnings reach
//! the client only inside a response, in the order they were written.

use std::cell::RefCell;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::rc::Rc;

use crate::env;
use crate::error::{self, MError};
use crate::files;
use crate::history;
use crate::interp::{InputSource, Interp};
use crate::json::{self, Json, ParseError};
use crate::syntax;

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
        "eval" => field(&req, "code").map(|code| eval(it, id.clone(), code)),
        "complete" => field(&req, "code").map(|code| {
            Json::object([
                ("id", id.clone()),
                ("ok", Json::Bool(true)),
                ("complete", Json::Bool(syntax::is_complete(code))),
            ])
        }),
        "workspace" => preview_flag(&req).map(|preview| workspace(it, id.clone(), preview)),
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

/// `workspace`'s optional `preview`: absent is `false`, and anything but a
/// JSON boolean is refused, `null` included.
fn preview_flag(req: &Json) -> Result<bool, MError> {
    match req.get("preview") {
        None => Ok(false),
        Some(Json::Bool(b)) => Ok(*b),
        Some(_) => Err(error::request_preview()),
    }
}

/// `files`: one folder of the file root, at most [`files::MAX_ENTRIES`]
/// entries of it. With no root, which only an interpreter a test builds
/// lacks, every path is outside it.
fn list_files(it: &Interp, id: Json, path: &str) -> Result<Json, MError> {
    let Some(root) = &it.file_root else {
        // The text is still judged first, so a malformed path says so.
        files::normalise(path)?;
        return Err(error::files_outside_root(path));
    };
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

/// A string field the operation needs.
fn field<'a>(req: &'a Json, name: &str) -> Result<&'a str, MError> {
    match req.get(name) {
        None => Err(error::request_missing(name)),
        Some(v) => v.as_str().ok_or_else(|| error::request_not_string(name)),
    }
}

/// `{"message": ..., "line": ...}`: the message exactly as the REPL prints it
/// after `Error: `, without the `Line N: ` that script mode adds, and the
/// line apart, one-based within the submitted code.
fn error_object(e: &MError) -> Json {
    let line = match e.line {
        Some(n) => Json::Number(f64::from(n)),
        None => Json::Null,
    };
    Json::object([("message", Json::String(e.msg.clone())), ("line", line)])
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
fn eval(it: &mut Interp, id: Json, code: &str) -> Json {
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
    let result = it.run_command(code);
    it.out = saved;
    it.err = saved_err;
    it.input = saved_input;
    let out = Json::String(String::from_utf8_lossy(&buf.borrow()).into_owned());
    match result {
        Ok(()) => Json::object([("id", id), ("ok", Json::Bool(true)), ("out", out)]),
        Err(e) => Json::object([
            ("id", id),
            ("ok", Json::Bool(false)),
            ("out", out),
            ("error", error_object(&e)),
        ]),
    }
}

/// One `{name, size, class}` per variable, sorted by name, and with
/// `preview` a `value` after `class`: [`env::preview`] in the session's
/// display format.
fn workspace(it: &Interp, id: Json, preview: bool) -> Json {
    let mut names: Vec<&String> = it.vars().keys().collect();
    names.sort();
    let vars = names
        .into_iter()
        .map(|name| {
            let v = &it.vars()[name];
            let (rows, cols) = v.dims();
            let mut pairs = vec![
                ("name".to_string(), Json::String(name.clone())),
                (
                    "size".to_string(),
                    Json::Array(vec![Json::Number(rows as f64), Json::Number(cols as f64)]),
                ),
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
