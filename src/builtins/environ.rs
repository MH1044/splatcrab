//! The environment (cycle 13): the working folder (`cd`, `pwd`, `ls`,
//! `dir`), the functions that describe names (`help`, `which`), the
//! display format, running text and files (`eval`, `evalc`, `run`), the
//! clock (`now`, `clock`, `datestr`, `pause`), the process's surroundings
//! (`getenv`, `system`, `version`), and `exit` and `quit`.
//!
//! The working folder is `Interp::cwd`, never the process's: `cd` moves the
//! interpreter alone, which is what keeps a golden case in its own folder
//! and the interface's file pane inside its root. `who`, `whos` and `clc`
//! stay in `core.rs` beside the rest of the workspace.

use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use super::args::{at_most, mat, need, scalar, string};
use super::strings::char_rows;
use super::{Registry, add, none, one, one_as, one_mat};
use crate::bail;
use crate::error::{self, MError};
use crate::interp::{Interp, R, is_identifier};
use crate::value::{Format, Matrix, StructArray, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "cd", cd, "cd, cd(folder), old = cd(folder) - show or change the current folder.");
    add(r, "pwd", pwd, "pwd - the current folder, as text.");
    add(r, "ls", ls, "ls, ls(folder), ls('*.m') - list the files in a folder, one name per line.");
    add(r, "dir", dir, "dir, dir(folder), s = dir(...) - list a folder; s holds name, folder, bytes and isdir.");
    add(r, "help", help, "help name - the help line of a builtin, or the leading comment block of a file.");
    add(r, "which", which, "which name, p = which(name) - where a name resolves: a variable, a file or a built-in.");
    add(r, "format", format, "format, format short, format long - four or fifteen decimals in every display.");
    add(r, "eval", eval, "eval(code), eval(code, fallback), v = eval(expr) - run text as code in the workspace.");
    add(r, "evalc", evalc, "s = evalc(code) - run text as code and return what it printed.");
    add(r, "run", run, "run(script) - run a script file, by name or path, in the workspace.");
    add(r, "datestr", datestr, "datestr(d) - a date number or date vector as 'dd-mmm-yyyy HH:MM:SS'.");
    add(r, "now", now, "now - the local date and time as a date number, days from year 0.");
    add(r, "clock", clock, "clock - the local date and time as [year month day hour minute seconds].");
    add(r, "pause", pause, "pause(n) - wait n seconds; pause - wait for Enter at a terminal.");
    add(r, "getenv", getenv, "getenv(name) - an environment variable, or '' when it is not set.");
    add(r, "system", system, "system(cmd), [status, out] = system(cmd) - run a shell command.");
    add(r, "version", version, "version - the SplatCrab version, as text.");
    add(r, "exit", exit, "exit, exit(n) - end the session, with exit code n (0 by default).");
    add(r, "quit", quit, "quit, quit(n) - end the session, with exit code n (0 by default).");
}

// ---- the working folder ----------------------------------------------

fn cwd_text(it: &Interp) -> String {
    it.cwd.display().to_string()
}

/// `cd` shows the current folder, `cd(folder)` moves there, and
/// `old = cd(folder)` returns the folder it left.
fn cd(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(args, 1, "cd")?;
    let old = cwd_text(it);
    if args.is_empty() {
        if nargout > 0 {
            return one(Value::str(&old));
        }
        it.emit(&format!("{}\n", old))?;
        return none();
    }
    let dir = string(args, 0, "cd")?;
    it.set_cwd(&dir)?;
    if nargout > 0 {
        return one(Value::str(&old));
    }
    none()
}

fn pwd(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "pwd")?;
    one(Value::str(&cwd_text(it)))
}

/// One entry of a folder listing.
struct Entry {
    name: String,
    dir: bool,
    bytes: u64,
}

/// The folder `ls` or `dir` lists, and its entries in byte order, `.` and
/// `..` left out. The argument is a folder, a file, or a name pattern
/// whose last part holds `*` or `?`, each resolved against
/// `Interp::cwd`.
fn listing(it: &Interp, args: &[Value], name: &str) -> R<(PathBuf, Vec<Entry>)> {
    at_most(args, 1, name)?;
    let (folder, pattern) = match args.first() {
        None => (it.cwd.clone(), None),
        Some(_) => {
            let spec = string(args, 0, name)?;
            let p = it.resolve_path(&spec);
            let last = p
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            if p.is_dir() {
                (p, None)
            } else if last.contains(['*', '?']) || p.is_file() {
                let parent = p.parent().map(Path::to_path_buf).unwrap_or_default();
                (parent, Some(last))
            } else {
                bail!(error::cannot_list(&spec));
            }
        }
    };
    let read = std::fs::read_dir(&folder)
        .map_err(|_| error::cannot_list(&folder.display().to_string()))?;
    let mut entries: Vec<Entry> = read
        .filter_map(Result::ok)
        .map(|e| {
            let meta = e.metadata().ok();
            Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                dir: meta.as_ref().is_some_and(|m| m.is_dir()),
                bytes: meta
                    .as_ref()
                    .map_or(0, |m| if m.is_dir() { 0 } else { m.len() }),
            }
        })
        .filter(|e| pattern.as_deref().is_none_or(|p| wildcard(p, &e.name)))
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((folder, entries))
}

/// True when `name` matches `pattern`, where `*` is any run of characters
/// and `?` any one. Linear in the two lengths' product at worst: the one
/// backtrack point is the last `*`.
pub(crate) fn wildcard(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut i, mut j) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some((i, j));
            i += 1;
        } else if let Some((si, sj)) = star {
            i = si + 1;
            j = sj + 1;
            star = Some((si, sj + 1));
        } else {
            return false;
        }
    }
    p[i..].iter().all(|&c| c == '*')
}

/// The names of a listing, one per line.
fn names_text(entries: &[Entry]) -> String {
    entries.iter().map(|e| format!("{}\n", e.name)).collect()
}

/// `ls` prints the names one per line; `s = ls` is a char matrix of them,
/// one name per row, padded with spaces.
fn ls(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    let (_, entries) = listing(it, args, "ls")?;
    if nargout > 0 {
        let rows: Vec<Vec<f64>> = entries
            .iter()
            .map(|e| e.name.encode_utf16().map(f64::from).collect())
            .collect();
        return one_as(char_rows(&rows));
    }
    it.emit(&names_text(&entries))?;
    none()
}

/// `dir` prints as `ls` does; `s = dir` is an Nx1 struct array of `name`,
/// `folder`, `bytes` and `isdir`.
fn dir(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    let (folder, entries) = listing(it, args, "dir")?;
    if nargout == 0 {
        it.emit(&names_text(&entries))?;
        return none();
    }
    let fields = ["name", "folder", "bytes", "isdir"]
        .map(String::from)
        .to_vec();
    let folder = folder.display().to_string();
    let elems: Vec<Vec<Value>> = entries
        .iter()
        .map(|e| {
            vec![
                Value::str(&e.name),
                Value::str(&folder),
                Value::Mat(Matrix::scalar(e.bytes as f64)),
                Value::Mat(Matrix::from_bool(e.dir)),
            ]
        })
        .collect();
    let n = elems.len();
    one(Value::strukt(StructArray::new(n, 1, fields, elems)))
}

// ---- names -----------------------------------------------------------

/// `help name`: a file on the path first, as a call would find it, and
/// then a builtin's help line; see [`file_help`]. A bare `help` is the help
/// of `help`.
fn help(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "help")?;
    let name = match args.first() {
        None => "help".to_string(),
        Some(_) => string(args, 0, "help")?,
    };
    let text = if let Some(path) = it.find_file(&name) {
        let bytes = std::fs::read(&path)
            .map_err(|e| error::cannot_read(&path.display().to_string(), &e))?;
        file_help(&crate::lexer::decode_source(&bytes))
            .unwrap_or_else(|| error::no_help_text(&name))
    } else if let Some(e) = it.builtins().get(name.as_str()) {
        format!("{}\n", e.help)
    } else {
        error::not_found_text(&name)
    };
    it.emit(&text)?;
    none()
}

/// A file's help text: its leading block of `%` comment lines, each less
/// the `%`, so `% MYF does things` is ` MYF does things`. Blank lines
/// before the block are skipped, and so is a `function` line, so the block
/// may come before the definition or straight after it. A `%{` block
/// comment is not a help block. `None` when there is no block.
pub(crate) fn file_help(src: &str) -> Option<String> {
    let mut lines = src
        .lines()
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .skip_while(|l| l.trim().is_empty())
        .peekable();
    let is_function = |l: &str| {
        let t = l.trim_start();
        t.strip_prefix("function")
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))
    };
    if lines.peek().is_some_and(|l| is_function(l)) {
        lines.next();
    }
    let text: String = lines
        .map_while(|l| {
            let t = l.trim_start();
            let rest = t.strip_prefix('%')?;
            (!rest.starts_with('{')).then(|| format!("{}\n", rest))
        })
        .collect();
    (!text.is_empty()).then_some(text)
}

/// `which name` prints where a call of `name` resolves: a variable, a file
/// on the path (its full path) or a built-in, `built-in (sum)`. With an
/// output it returns that text without the line end: `'variable'` for a
/// variable, and `''` for a name that is nothing.
fn which(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(args, 1, "which")?;
    at_most(args, 1, "which")?;
    let name = string(args, 0, "which")?;
    let (shown, value) = if it.vars().contains_key(&name) {
        (error::which_variable_text(&name), "variable".to_string())
    } else if let Some(path) = it.find_file(&name) {
        let p = path.display().to_string();
        (format!("{}\n", p), p)
    } else if it.builtins().contains_key(name.as_str()) {
        let b = error::which_builtin_text(&name);
        (format!("{}\n", b), b)
    } else {
        (error::not_found_text(&name), String::new())
    };
    if nargout > 0 {
        return one(Value::str(&value));
    }
    it.emit(&shown)?;
    none()
}

/// `format`, `format short` and `format long`: the decimals of every
/// display from here on. A bare `format` is `short`.
fn format(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "format")?;
    it.format = match args.first() {
        None => Format::Short,
        Some(_) => {
            let opt = string(args, 0, "format")?;
            match opt.to_ascii_lowercase().as_str() {
                "short" => Format::Short,
                "long" => Format::Long,
                _ => bail!(error::format_unknown(&opt)),
            }
        }
    };
    none()
}

// ---- running text and files --------------------------------------------

/// `eval(code)` and `v = eval(expr)`; see `Interp::eval_code`. With a
/// second text, that text runs when the first fails, as a `catch` would,
/// and `lasterr` is the first one's message; an `exit` in the first is
/// not a failure.
fn eval(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(args, 1, "eval")?;
    at_most(args, 2, "eval")?;
    let code = string(args, 0, "eval")?;
    let fallback = match args.get(1) {
        Some(_) => Some(string(args, 1, "eval")?),
        None => None,
    };
    match (it.eval_code(&code, nargout), fallback) {
        (Err(e), Some(other)) if e.exit_code().is_none() => {
            it.last_err = e.msg;
            it.eval_code(&other, nargout)
        }
        (r, _) => r,
    }
}

/// A shared byte buffer that the interpreter's boxed sinks write into and
/// `evalc` reads back once they are put back.
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

/// `s = evalc(code)`: `code` run as `eval` runs it, with both sinks swapped
/// for one buffer for the length of the run, as a protocol `eval` captures
/// (cycle U0), so a warning lands where it was raised and nothing reaches
/// the terminal; `clc` inside writes nothing. The text is what was printed
/// less one final line end, which is what the spec records for
/// `evalc('disp(1)')`. An error inside is raised after the sinks are back.
fn evalc(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "evalc")?;
    at_most(args, 1, "evalc")?;
    let code = string(args, 0, "evalc")?;
    let buf = Rc::new(RefCell::new(Vec::new()));
    let out = std::mem::replace(&mut it.out, Box::new(Capture(buf.clone())));
    let err = std::mem::replace(&mut it.err, Box::new(Capture(buf.clone())));
    let tty = std::mem::replace(&mut it.stdout_tty, false);
    let result = it.eval_code(&code, 0);
    it.out = out;
    it.err = err;
    it.stdout_tty = tty;
    result?;
    let mut text = String::from_utf8_lossy(&buf.borrow()).into_owned();
    if text.ends_with('\n') {
        text.pop();
    }
    one(Value::str(&text))
}

/// `run(script)`: a path to a `.m` file, with or without the `.m`, against
/// `Interp::cwd`, or else a name on the path; see `Interp::run_path`.
fn run(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "run")?;
    at_most(args, 1, "run")?;
    let name = string(args, 0, "run")?;
    let file = if name.ends_with(".m") {
        name.clone()
    } else {
        format!("{}.m", name)
    };
    let direct = it.resolve_path(&file);
    let path = if direct.is_file() {
        direct
    } else if is_identifier(&name) {
        it.find_file(&name)
            .ok_or_else(|| error::run_not_found(&name))?
    } else {
        bail!(error::run_not_found(&name));
    };
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    it.run_path(&path, &stem)?;
    none()
}

// ---- the clock ---------------------------------------------------------

/// The date number of 1970-01-01, days from MATLAB's year 0.
const UNIX_EPOCH_DATENUM: f64 = 719_529.0;

/// Days from 1970-01-01 to a proleptic Gregorian date; Howard Hinnant's
/// `days_from_civil`, exact for every year.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date `days` from 1970-01-01: year, month and day; the inverse of
/// [`days_from_civil`].
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A date vector `[y mo d h mi s]` as a date number. The month and day
/// may run past their ranges, and carry, as MATLAB's `datenum` lets them.
pub(crate) fn datenum(v: [f64; 6]) -> f64 {
    let (y, mo) = (v[0].floor() as i64, v[1].floor() as i64);
    let (y, mo) = (y + (mo - 1).div_euclid(12), (mo - 1).rem_euclid(12) + 1);
    let days = days_from_civil(y, mo, 1) as f64 + (v[2] - 1.0);
    UNIX_EPOCH_DATENUM + days + (v[3] * 3600.0 + v[4] * 60.0 + v[5]) / 86_400.0
}

/// The largest date number, and the largest date-vector component,
/// `datestr` accepts: ten billion days, about 27 million years either side
/// of year 0. Past it the integer day and second arithmetic would overflow,
/// which panicked (exit 101) for `datestr([1e17 1 1 0 0 0])` and printed a
/// nonsense year for `datestr(1e300)` (cycle 13's review). The bound keeps
/// every intermediate below 2^63 with room to spare: 1e10 days is 8.64e14
/// seconds.
pub const MAX_DATE: f64 = 1e10;

/// A date number in `datestr`'s default form, `dd-mmm-yyyy HH:MM:SS`,
/// rounded to the second.
pub(crate) fn date_text(d: f64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let secs = (d * 86_400.0).round() as i64;
    let (days, secs) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, day) = civil_from_days(days - UNIX_EPOCH_DATENUM as i64);
    format!(
        "{:02}-{}-{:04} {:02}:{:02}:{:02}",
        day,
        MONTHS[(m - 1) as usize],
        y,
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// The local date and time now, as a date vector.
fn local_now() -> [f64; 6] {
    local::now().unwrap_or_else(|| {
        // UTC, where the platform's local time is not reachable.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64());
        let days = (t / 86_400.0).floor();
        let (y, m, d) = civil_from_days(days as i64);
        let s = t - days * 86_400.0;
        [
            y as f64,
            m as f64,
            d as f64,
            (s / 3600.0).floor(),
            (s / 60.0).floor() % 60.0,
            s % 60.0,
        ]
    })
}

/// Local time through the platform's own calls, which the standard library
/// does not expose: raw declarations, as `main.rs` declares
/// `SetConsoleOutputCP`, because the crate takes no dependencies.
#[cfg(windows)]
mod local {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }

    pub fn now() -> Option<[f64; 6]> {
        let mut t = SystemTime::default();
        // SAFETY: `t` is a valid, writable SYSTEMTIME, the only argument
        // GetLocalTime takes, and it cannot fail.
        unsafe { GetLocalTime(&mut t) };
        Some([
            f64::from(t.year),
            f64::from(t.month),
            f64::from(t.day),
            f64::from(t.hour),
            f64::from(t.minute),
            f64::from(t.second) + f64::from(t.milliseconds) / 1000.0,
        ])
    }
}

#[cfg(all(unix, target_pointer_width = "64"))]
mod local {
    /// `struct tm`: the nine `int` fields POSIX fixes the order of, then
    /// room for the fields each C library adds after them (`tm_gmtoff`,
    /// `tm_zone`), more than any of them has.
    #[repr(C)]
    #[derive(Default)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        rest: [u64; 8],
    }

    unsafe extern "C" {
        fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
    }

    pub fn now() -> Option<[f64; 6]> {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?;
        let secs = i64::try_from(t.as_secs()).ok()?;
        let mut tm = Tm::default();
        // SAFETY: `time_t` is a 64-bit integer on every 64-bit Unix, `secs`
        // outlives the call, and `tm` is larger than any C library's
        // `struct tm`, whose leading fields it declares in POSIX's order.
        let r = unsafe { localtime_r(&secs, &mut tm) };
        if r.is_null() {
            return None;
        }
        Some([
            f64::from(tm.year) + 1900.0,
            f64::from(tm.mon) + 1.0,
            f64::from(tm.mday),
            f64::from(tm.hour),
            f64::from(tm.min),
            f64::from(tm.sec) + f64::from(t.subsec_millis()) / 1000.0,
        ])
    }
}

#[cfg(not(any(windows, all(unix, target_pointer_width = "64"))))]
mod local {
    pub fn now() -> Option<[f64; 6]> {
        None
    }
}

fn now(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "now")?;
    one_mat(Matrix::scalar(datenum(local_now())))
}

fn clock(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "clock")?;
    one_mat(Matrix::row(local_now().to_vec()))
}

/// `datestr(d)`: each date number of `d` as one row of text; a 1x6 row is
/// a date vector instead, as `clock` returns.
fn datestr(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "datestr")?;
    at_most(args, 1, "datestr")?;
    let m = mat(args, 0, "datestr")?;
    if m.is_char() || m.data.iter().any(|v| !v.is_finite()) {
        bail!(error::datestr_input());
    }
    if m.data.iter().any(|v| v.abs() > MAX_DATE) {
        bail!(error::datestr_range(MAX_DATE));
    }
    let days: Vec<f64> = if (m.rows, m.cols) == (1, 6) {
        let v: [f64; 6] = std::array::from_fn(|k| m.data[k]);
        vec![datenum(v)]
    } else {
        m.data.clone()
    };
    // A vector's components are each in range, but their sum need not be.
    if days.iter().any(|d| !d.is_finite() || d.abs() > MAX_DATE) {
        bail!(error::datestr_range(MAX_DATE));
    }
    let rows: Vec<Vec<f64>> = days
        .iter()
        .map(|&d| date_text(d).encode_utf16().map(f64::from).collect())
        .collect();
    one_as(char_rows(&rows))
}

/// The longest `pause(n)` waits, a day: long enough for any script, and a
/// bound, so no argument can hang a session for good.
pub const MAX_PAUSE: f64 = 86_400.0;

/// `pause(n)` waits `n` seconds, from 0 to [`MAX_PAUSE`]. A bare `pause`
/// waits for Enter when standard input is a terminal, and is a clean error
/// anywhere else, so a piped script never hangs. Output is flushed first,
/// so what was printed shows during the wait.
fn pause(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "pause")?;
    it.out.flush().map_err(error::output)?;
    if args.is_empty() {
        if !it.stdin_tty {
            bail!(error::pause_no_terminal());
        }
        it.read_input_line()?;
        return none();
    }
    // A char or a logical is not a number of seconds: `pause('a')` used to
    // sleep 97 seconds, the code of 'a' (cycle 13's review).
    if let Value::Mat(m) = &args[0] {
        if m.is_char() || m.class == crate::value::Class::Logical {
            bail!(error::pause_time(MAX_PAUSE));
        }
    }
    let n = scalar(args, 0, "pause")?;
    if !(0.0..=MAX_PAUSE).contains(&n) {
        bail!(error::pause_time(MAX_PAUSE));
    }
    std::thread::sleep(Duration::from_secs_f64(n));
    none()
}

// ---- the process's surroundings ----------------------------------------

/// `getenv(name)`: the variable's value, or `''` when it is not set or the
/// name is not one a variable can have.
fn getenv(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "getenv")?;
    at_most(args, 1, "getenv")?;
    let name = string(args, 0, "getenv")?;
    if name.is_empty() || name.contains(['=', '\0']) {
        return one(Value::str(""));
    }
    let value = std::env::var_os(&name)
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default();
    one(Value::str(&value))
}

/// The shell command `cmd` runs as: `cmd /C` on Windows, handed the text
/// verbatim, and `sh -c` elsewhere.
fn shell(cmd: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        c.arg("/C").raw_arg(cmd);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    }
}

/// `system(cmd)`: runs `cmd` in the shell, in `Interp::cwd`, with no
/// standard input, so it can never read a script's input or a protocol's
/// requests. Its exit status is the first output, `-1` when it was ended
/// by a signal; its standard output and then its standard error are the
/// second. Asked for fewer than two outputs, it prints them instead.
fn system(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(args, 1, "system")?;
    at_most(args, 1, "system")?;
    let cmd = string(args, 0, "system")?;
    it.out.flush().map_err(error::output)?;
    let done = shell(&cmd)
        .current_dir(&it.cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| error::system_failed(&e))?;
    let status = done.status.code().map_or(-1.0, f64::from);
    let mut text = String::from_utf8_lossy(&done.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&done.stderr));
    let status = Value::Mat(Matrix::scalar(status));
    if nargout < 2 {
        it.emit(&text)?;
        return Ok(vec![status]);
    }
    Ok(vec![status, Value::str(&text)])
}

fn version(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "version")?;
    one(Value::str(env!("CARGO_PKG_VERSION")))
}

// ---- exit and quit -------------------------------------------------------

fn exit(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    leave(it, args, "exit")
}

fn quit(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    leave(it, args, "quit")
}

/// `exit`, `exit(n)` and `exit force` (and the same of `quit`): an
/// [`MError::exit`] that leaves every frame and reaches `main.rs`, which
/// exits with the code. Where the session belongs to a client, under
/// `--protocol`, `--ui` and `--http-stdio`, whose `eval` refuses `input`
/// for the same reason, it is a clean error and the session goes on.
fn leave(it: &mut Interp, args: &[Value], name: &str) -> R<Vec<Value>> {
    if it.input_refused() {
        bail!(error::exit_refused(name));
    }
    at_most(args, 2, name)?;
    let mut code = 0;
    // At most one code: `exit(1, 2)` used to exit 2 (cycle 13's review).
    if args.iter().filter(|a| a.text().is_none()).count() > 1 {
        bail!(error::exit_code_invalid(name));
    }
    for (k, a) in args.iter().enumerate() {
        match a.text() {
            Some(t) if t.eq_ignore_ascii_case("force") => {}
            Some(_) => bail!(error::exit_code_invalid(name)),
            None => {
                let n = scalar(args, k, name)?;
                if n.fract() != 0.0 || !(0.0..=255.0).contains(&n) {
                    bail!(error::exit_code_invalid(name));
                }
                code = n as i32;
            }
        }
    }
    Err(MError::exit(code))
}

#[cfg(test)]
mod tests {
    /// Extreme dates are a clean error, never an overflow panic (cycle 13's
    /// review found `datestr([1e17 1 1 0 0 0])` exiting 101).
    #[test]
    fn datestr_refuses_dates_past_its_range() {
        for v in [
            [1e17, 1.0, 1.0, 0.0, 0.0, 0.0],
            [-1e17, 1.0, 1.0, 0.0, 0.0, 0.0],
            [2000.0, 1e300, 1.0, 0.0, 0.0, 0.0],
        ] {
            let m = Value::Mat(crate::value::Matrix::row(v.to_vec()));
            let mut it = Interp::new();
            assert!(datestr(&mut it, &[m], 1).is_err(), "{v:?}");
        }
        for d in [1e300, -1e300, 2e10] {
            let mut it = Interp::new();
            let m = Value::Mat(crate::value::Matrix::scalar(d));
            assert!(datestr(&mut it, &[m], 1).is_err(), "{d}");
        }
        let mut it = Interp::new();
        let ok = Value::Mat(crate::value::Matrix::row(vec![
            2000.0, 1.0, 1.0, 0.0, 0.0, 0.0,
        ]));
        assert!(datestr(&mut it, &[ok], 1).is_ok());
    }

    use super::*;

    #[test]
    fn dates_round_trip_through_date_numbers() {
        // The epoch MATLAB counts from: datenum(1970, 1, 1) is 719529.
        assert_eq!(datenum([1970.0, 1.0, 1.0, 0.0, 0.0, 0.0]), 719_529.0);
        assert_eq!(
            datenum([2000.0, 3.0, 1.0, 0.0, 0.0, 0.0])
                - datenum([2000.0, 2.0, 28.0, 0.0, 0.0, 0.0]),
            2.0
        );
        assert_eq!(date_text(719_529.0), "01-Jan-1970 00:00:00");
        let d = datenum([2026.0, 9.0, 29.0, 13.0, 5.0, 7.0]);
        assert_eq!(date_text(d), "29-Sep-2026 13:05:07");
        // Month 13 carries into the next year.
        assert_eq!(
            date_text(datenum([2025.0, 13.0, 1.0, 0.0, 0.0, 0.0])),
            "01-Jan-2026 00:00:00"
        );
        // Rounded to the second, carrying into the next day.
        assert_eq!(
            date_text(datenum([1999.0, 12.0, 31.0, 23.0, 59.0, 59.6])),
            "01-Jan-2000 00:00:00"
        );
        for days in [-800_000_i64, -1, 0, 59, 11_016, 2_932_896] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn the_clock_is_a_plausible_date() {
        let v = local_now();
        assert!(v[0] >= 2024.0 && (1.0..=12.0).contains(&v[1]) && (1.0..=31.0).contains(&v[2]));
        assert!(
            (0.0..24.0).contains(&v[3])
                && (0.0..60.0).contains(&v[4])
                && (0.0..61.0).contains(&v[5])
        );
    }

    #[test]
    fn wildcards_match_whole_names() {
        assert!(wildcard("*.m", "a.m"));
        assert!(!wildcard("*.m", "a.mat"));
        assert!(wildcard("a?c*", "abcdef"));
        assert!(wildcard("*", ""));
        assert!(!wildcard("?", ""));
        assert!(wildcard("*a*b", "xxaxxb"));
        assert!(!wildcard("*a*b", "xxaxxbc"));
    }

    #[test]
    fn the_help_block_is_the_leading_comments() {
        assert_eq!(
            file_help("% MYF does things\n% second line\nfunction y = myf(x)\ny = x;\n").as_deref(),
            Some(" MYF does things\n second line\n")
        );
        assert_eq!(
            file_help("\nfunction y = g(x)\n  %G  doubles\n  y = 2 * x;\n").as_deref(),
            Some("G  doubles\n")
        );
        assert_eq!(file_help("x = 1;\n% late\n"), None);
        assert_eq!(file_help("%{\nblock\n%}\n"), None);
        assert_eq!(file_help("functionality = 1;\n"), None);
    }
}
