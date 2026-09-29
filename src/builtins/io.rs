//! Input and files (cycle 11): `input`, the file-identifier functions
//! `fopen`, `fclose`, `fgetl`, `fgets`, `fprintf`, `fread`, `fwrite` and
//! `feof`, the whole-file functions `fileread`, `readmatrix`,
//! `writematrix`, `csvread` and `csvwrite`, `delete`, and `save` and
//! `load` over the MAT reader and writer of `mat.rs`.
//!
//! Every path a function here names resolves against `Interp::cwd`,
//! through `Interp::resolve_path`, and a function that writes or deletes a
//! file marks the interpreter's file lookups stale, so a `.m` file a
//! script writes is found by the next call.
//!
//! Identifiers `0`, `1` and `2` are standard input, output and error.
//! `fprintf` and `fwrite` to `1` go through `Interp.out`, and to `2`
//! through `Interp.err`, so under `--protocol` and `--ui` they land in an
//! `eval`'s `out`. A file `fopen` opens takes the lowest free identifier
//! from 3 up. Any other identifier, and a standard stream given to a
//! function that reads or closes files, is "Invalid file identifier."
//!
//! Text is written as UTF-8 and read as UTF-8, an undecodable byte
//! becoming U+FFFD, as a script file is read.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};

use super::args::{MAX_ELEMS, at_most, check_shape, mat, need, string};
use super::printf::format_printf;
use super::strings::parse_double;
use super::{Registry, add, none, one, one_as};
use crate::error;
use crate::interp::{Interp, R, fmt_e, fmt_g, is_identifier};
use crate::value::{Class, Matrix, StructArray, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "input", input, "input(prompt), input(prompt,'s') - read a line from the terminal, evaluated or as text.");
    add(r, "fopen", fopen, "[fid,msg] = fopen(name,permission) - open a file: 'r', 'w', 'a', 'r+', 'w+', 'a+', with 't' or 'b'.");
    add(r, "fclose", fclose, "fclose(fid), fclose('all') - close a file.");
    add(r, "fgetl", fgetl, "fgetl(fid) - the next line of a file without its newline, or -1 at the end.");
    add(r, "fgets", fgets, "fgets(fid) - the next line of a file with its newline, or -1 at the end.");
    add(r, "fprintf", fprintf, "fprintf(fmt,...), fprintf(fid,fmt,...) - write formatted text; n = fprintf(...) is the byte count.");
    add(r, "fread", fread, "[A,count] = fread(fid,size,precision) - read binary data as a column of doubles.");
    add(r, "fwrite", fwrite, "count = fwrite(fid,A,precision) - write the elements of A as binary data.");
    add(r, "feof", feof, "feof(fid) - 1 once a read has reached the end of the file.");
    add(r, "fileread", fileread, "fileread(name) - the whole text of a file.");
    add(r, "readmatrix", readmatrix, "readmatrix(name) - a numeric matrix from a delimited text file.");
    add(r, "writematrix", writematrix, "writematrix(A,name) - write a matrix as comma-separated text.");
    add(r, "csvread", csvread, "csvread(name,r,c) - a numeric matrix from a comma-separated file, from row r and column c.");
    add(r, "csvwrite", csvwrite, "csvwrite(name,A) - write a matrix as comma-separated text, five significant digits.");
    add(r, "delete", delete, "delete(name) - delete a file.");
    add(r, "save", save, "save(name,vars...), save(name,vars...,'-ascii') - write variables to a MAT-file or a text file.");
    add(r, "load", load, "load(name,vars...), S = load(name) - read variables from a MAT-file or a text file.");
}

// ---- the file-identifier table -------------------------------------------

/// How much a read asks the operating system for at a time.
const CHUNK: usize = 1 << 16;

/// The first identifier a file can have: `0`, `1` and `2` are the
/// standard streams.
const FIRST_FID: usize = 3;

/// The most bytes of text `fileread`, `fgetl` and `fgets` read before
/// judging the result: UTF-8 takes at most three bytes per UTF-16 code
/// unit, so text longer than this is past `MAX_ELEMS` code units whatever
/// it holds, and is refused before the rest of it is read (invariant 6).
pub(crate) const MAX_TEXT_BYTES: usize = 3 * MAX_ELEMS;

/// The error for `n` bytes of text past [`MAX_TEXT_BYTES`]: `check_shape`'s
/// own, for a row at least as long as the text could decode to.
fn text_too_long(n: usize) -> error::MError {
    let units = n.div_ceil(3) as f64;
    check_shape(1.0, units)
        .err()
        .unwrap_or_else(|| error::size_overflow("1", &units.to_string()))
}

/// One open file: the handle, what it was opened for, and the bytes read
/// ahead of the position the script has reached.
pub struct OpenFile {
    /// The name `fopen` was given, for the messages.
    name: String,
    file: File,
    read: bool,
    write: bool,
    /// Opened with `t`: `fgetl` drops a carriage return before the newline.
    text: bool,
    /// Bytes read ahead, and how many of them the script has consumed.
    buf: Vec<u8>,
    pos: usize,
    /// Set once a read has reached the end of the file.
    eof: bool,
    /// Whether anything was written, so closing marks file lookups stale.
    wrote: bool,
}

impl OpenFile {
    fn readable(&self) -> R<()> {
        if self.read {
            Ok(())
        } else {
            Err(error::file_not_readable())
        }
    }

    /// True when there is a byte to read, reading ahead if need be.
    fn fill(&mut self) -> R<bool> {
        if self.pos < self.buf.len() {
            return Ok(true);
        }
        self.buf.clear();
        self.buf.resize(CHUNK, 0);
        self.pos = 0;
        let n = loop {
            match self.file.read(&mut self.buf) {
                Ok(n) => break n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    self.buf.clear();
                    return Err(error::cannot_read_file(&self.name, &e));
                }
            }
        };
        self.buf.truncate(n);
        Ok(n > 0)
    }

    /// Sets the end-of-file flag if nothing is left to read. A read that
    /// ends exactly at the end of the file sets it, so `while ~feof(fid)`
    /// stops after the last line whether or not a newline ends it.
    fn note_end(&mut self) -> R<()> {
        if !self.fill()? {
            self.eof = true;
        }
        Ok(())
    }

    /// The next line, its newline kept, or `None` at the end of the file.
    fn line(&mut self) -> R<Option<Vec<u8>>> {
        self.readable()?;
        let mut out = Vec::new();
        let mut any = false;
        loop {
            if !self.fill()? {
                self.eof = true;
                break;
            }
            any = true;
            let rest = &self.buf[self.pos..];
            match rest.iter().position(|&b| b == b'\n') {
                Some(k) => {
                    out.extend_from_slice(&rest[..=k]);
                    self.pos += k + 1;
                    self.note_end()?;
                    break;
                }
                None => {
                    out.extend_from_slice(rest);
                    self.pos = self.buf.len();
                }
            }
            if out.len() > MAX_TEXT_BYTES {
                return Err(text_too_long(out.len()));
            }
        }
        Ok(any.then_some(out))
    }

    /// Up to `max` bytes, fewer only at the end of the file.
    fn bytes(&mut self, max: usize) -> R<Vec<u8>> {
        self.readable()?;
        let mut out = Vec::new();
        while out.len() < max {
            if !self.fill()? {
                self.eof = true;
                break;
            }
            let take = (self.buf.len() - self.pos).min(max - out.len());
            out.extend_from_slice(&self.buf[self.pos..self.pos + take]);
            self.pos += take;
        }
        self.note_end()?;
        Ok(out)
    }

    fn write_bytes(&mut self, b: &[u8]) -> R<()> {
        if !self.write {
            return Err(error::file_not_writable());
        }
        // Bytes read ahead and not consumed are given back first, so the
        // write lands where the script's position is.
        let ahead = self.buf.len() - self.pos;
        if ahead > 0 {
            self.file
                .seek(SeekFrom::Current(-(ahead as i64)))
                .map_err(|e| error::cannot_write_file(&self.name, &e))?;
        }
        self.buf.clear();
        self.pos = 0;
        self.eof = false;
        self.wrote = true;
        self.file
            .write_all(b)
            .map_err(|e| error::cannot_write_file(&self.name, &e))
    }
}

/// The open files, by identifier.
#[derive(Default)]
pub struct FileTable {
    /// Slot `k` holds identifier `k + 3`.
    files: Vec<Option<OpenFile>>,
}

impl FileTable {
    /// Adds `f` under the lowest free identifier, which it returns.
    fn insert(&mut self, f: OpenFile) -> usize {
        match self.files.iter().position(Option::is_none) {
            Some(k) => {
                self.files[k] = Some(f);
                k + FIRST_FID
            }
            None => {
                self.files.push(Some(f));
                self.files.len() - 1 + FIRST_FID
            }
        }
    }

    fn slot(fid: f64) -> Option<usize> {
        (fid.is_finite() && fid.fract() == 0.0 && fid >= FIRST_FID as f64)
            .then(|| fid as usize - FIRST_FID)
    }

    /// The file under `fid`, or "Invalid file identifier."
    fn get(&mut self, fid: f64) -> R<&mut OpenFile> {
        Self::slot(fid)
            .and_then(|k| self.files.get_mut(k))
            .and_then(Option::as_mut)
            .ok_or_else(error::invalid_fid)
    }

    fn remove(&mut self, fid: f64) -> R<OpenFile> {
        let f = Self::slot(fid)
            .and_then(|k| self.files.get_mut(k))
            .and_then(Option::take)
            .ok_or_else(error::invalid_fid)?;
        while matches!(self.files.last(), Some(None)) {
            self.files.pop();
        }
        Ok(f)
    }

    fn remove_all(&mut self) -> Vec<OpenFile> {
        self.files.drain(..).flatten().collect()
    }
}

/// A file identifier argument: a real scalar.
fn fid_arg(args: &[Value], i: usize, name: &str) -> R<f64> {
    match args.get(i) {
        Some(Value::Mat(m)) if m.class != Class::Char => {
            m.scalar_value().ok_or_else(error::invalid_fid)
        }
        Some(_) => Err(error::invalid_fid()),
        None => Err(error::not_enough_args(name)),
    }
}

fn num(v: f64) -> Value {
    Value::Mat(Matrix::scalar(v))
}

/// Bytes as a char row, decoded as UTF-8, its length judged first.
fn text_value(b: &[u8]) -> R<Value> {
    let text = String::from_utf8_lossy(b);
    check_shape(1.0, text.encode_utf16().count() as f64)?;
    Ok(Value::str(&text))
}

// ---- input -----------------------------------------------------------------

/// `input(prompt)` evaluates the line typed as an expression in the
/// workspace, an empty line being `[]`; `input(prompt, 's')` returns it as
/// text. Where there is no terminal, `--protocol`, `--ui` and
/// `--http-stdio`, it is a clean error before the prompt is written.
fn input(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "input")?;
    at_most(args, 2, "input")?;
    let prompt = string(args, 0, "input")?;
    let as_text = match args.get(1) {
        None => false,
        Some(v) if v.text().as_deref() == Some("s") => true,
        Some(_) => return Err(error::input_option()),
    };
    if it.input_refused() {
        return Err(error::input_unavailable());
    }
    it.emit(&prompt)?;
    let line = it.read_input_line()?.ok_or_else(error::input_ended)?;
    if as_text {
        return one(Value::str(&line));
    }
    if line.trim().is_empty() {
        return one(Value::Mat(Matrix::empty()));
    }
    one(it.eval_text(&line)?)
}

// ---- opening and closing -----------------------------------------------------

/// A permission: `r`, `w`, `a`, `r+`, `w+` or `a+`, with at most one `t`
/// or `b` anywhere after the letter.
fn permission(mode: &str) -> Option<(OpenOptions, bool, bool, bool)> {
    let mut chars = mode.chars();
    let base = chars.next()?;
    let (mut plus, mut text, mut binary) = (false, false, false);
    for c in chars {
        let seen = match c {
            '+' => &mut plus,
            't' => &mut text,
            'b' => &mut binary,
            _ => return None,
        };
        if *seen {
            return None;
        }
        *seen = true;
    }
    if text && binary {
        return None;
    }
    let mut o = OpenOptions::new();
    let (read, write) = match base {
        'r' => {
            o.read(true).write(plus);
            (true, plus)
        }
        'w' => {
            o.write(true).read(plus).create(true).truncate(true);
            (plus, true)
        }
        'a' => {
            o.append(true).read(plus).create(true);
            (plus, true)
        }
        _ => return None,
    };
    Some((o, read, write, text))
}

/// `[fid, msg] = fopen(name, permission)`: `-1` and the reason when the
/// file cannot be opened, as MATLAB does, rather than an error.
fn fopen(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "fopen")?;
    at_most(args, 2, "fopen")?;
    let name = string(args, 0, "fopen")?;
    let mode = match args.get(1) {
        Some(_) => string(args, 1, "fopen")?,
        None => "r".to_string(),
    };
    let (opts, read, write, text) =
        permission(&mode).ok_or_else(|| error::fopen_permission(&mode))?;
    let path = it.resolve_path(&name);
    let opened = if name.is_empty() {
        Err(io::Error::from(io::ErrorKind::NotFound))
    } else if path.is_dir() {
        Err(io::Error::from(io::ErrorKind::IsADirectory))
    } else {
        opts.open(&path)
    };
    match opened {
        Ok(file) => {
            if write {
                it.files_changed();
            }
            let fid = it.open_files.insert(OpenFile {
                name,
                file,
                read,
                write,
                text,
                buf: Vec::new(),
                pos: 0,
                eof: false,
                wrote: false,
            });
            Ok(vec![num(fid as f64), Value::str("")])
        }
        Err(e) => Ok(vec![num(-1.0), Value::str(error::io_reason(&e))]),
    }
}

/// `fclose(fid)` and `fclose('all')`, each `0`.
fn fclose(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "fclose")?;
    at_most(args, 1, "fclose")?;
    let closed = if args[0].text().as_deref() == Some("all") {
        it.open_files.remove_all()
    } else {
        vec![it.open_files.remove(fid_arg(args, 0, "fclose")?)?]
    };
    if closed.iter().any(|f| f.wrote) {
        it.files_changed();
    }
    one(num(0.0))
}

// ---- reading and writing through an identifier -------------------------------------

fn next_line(it: &mut Interp, args: &[Value], name: &str, keep: bool) -> R<Vec<Value>> {
    need(args, 1, name)?;
    at_most(args, 1, name)?;
    let f = it.open_files.get(fid_arg(args, 0, name)?)?;
    let Some(mut line) = f.line()? else {
        return one(num(-1.0));
    };
    if !keep && line.last() == Some(&b'\n') {
        line.pop();
        if f.text && line.last() == Some(&b'\r') {
            line.pop();
        }
    }
    one(text_value(&line)?)
}

fn fgetl(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    next_line(it, args, "fgetl", false)
}

fn fgets(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    next_line(it, args, "fgets", true)
}

fn feof(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "feof")?;
    at_most(args, 1, "feof")?;
    let f = it.open_files.get(fid_arg(args, 0, "feof")?)?;
    one(num(f64::from(u8::from(f.eof))))
}

/// Text or bytes to identifier `fid`: `1` is `Interp.out`, `2` is
/// `Interp.err`, and from 3 up an open file.
fn write_to(it: &mut Interp, fid: f64, b: &[u8]) -> R<()> {
    if fid == 1.0 {
        it.emit_bytes(b)
    } else if fid == 2.0 {
        it.emit_err_bytes(b)
    } else {
        it.open_files.get(fid)?.write_bytes(b)
    }
}

/// `fprintf(fmt, ...)` and `fprintf(fid, fmt, ...)`: a numeric first
/// argument followed by more is a file identifier. `n = fprintf(...)` is
/// the number of bytes written.
fn fprintf(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(args, 1, "fprintf")?;
    let fid = match &args[0] {
        Value::Mat(m) if m.class != Class::Char && args.len() >= 2 => {
            Some(m.scalar_value().ok_or_else(error::invalid_fid)?)
        }
        _ => None,
    };
    let rest = if fid.is_some() { &args[1..] } else { args };
    // An identifier that names nothing is refused before anything is
    // formatted.
    if let Some(fid) = fid.filter(|f| *f != 1.0 && *f != 2.0) {
        it.open_files.get(fid)?;
    }
    let text = format_printf(rest)?;
    write_to(it, fid.unwrap_or(1.0), text.as_bytes())?;
    if nargout > 0 {
        return one(num(text.len() as f64));
    }
    none()
}

/// A binary precision: the type of each element in the file.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Prec {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl Prec {
    fn size(self) -> usize {
        match self {
            Prec::U8 | Prec::I8 => 1,
            Prec::U16 | Prec::I16 => 2,
            Prec::U32 | Prec::I32 | Prec::F32 => 4,
            Prec::U64 | Prec::I64 | Prec::F64 => 8,
        }
    }

    /// The bytes of `v`, little-endian: an integer type rounds and
    /// saturates, and a `NaN` is `0`, as MATLAB writes them.
    fn encode(self, v: f64, out: &mut Vec<u8>) {
        let int = |lo: f64, hi: f64| {
            if v.is_nan() {
                0.0
            } else {
                v.round().clamp(lo, hi)
            }
        };
        match self {
            Prec::U8 => out.push(int(0.0, 255.0) as u8),
            Prec::I8 => out.push(int(-128.0, 127.0) as i8 as u8),
            Prec::U16 => out.extend((int(0.0, 65535.0) as u16).to_le_bytes()),
            Prec::I16 => out.extend((int(-32768.0, 32767.0) as i16).to_le_bytes()),
            Prec::U32 => out.extend((int(0.0, 4294967295.0) as u32).to_le_bytes()),
            Prec::I32 => out.extend((int(-2147483648.0, 2147483647.0) as i32).to_le_bytes()),
            Prec::U64 => out.extend((int(0.0, u64::MAX as f64) as u64).to_le_bytes()),
            Prec::I64 => out.extend((int(i64::MIN as f64, i64::MAX as f64) as i64).to_le_bytes()),
            Prec::F32 => out.extend((v as f32).to_le_bytes()),
            Prec::F64 => out.extend(v.to_le_bytes()),
        }
    }

    fn decode(self, b: &[u8]) -> f64 {
        let mut w = [0u8; 8];
        w[..b.len()].copy_from_slice(b);
        match self {
            Prec::U8 => f64::from(w[0]),
            Prec::I8 => f64::from(w[0] as i8),
            Prec::U16 => f64::from(u16::from_le_bytes([w[0], w[1]])),
            Prec::I16 => f64::from(i16::from_le_bytes([w[0], w[1]])),
            Prec::U32 => f64::from(u32::from_le_bytes([w[0], w[1], w[2], w[3]])),
            Prec::I32 => f64::from(i32::from_le_bytes([w[0], w[1], w[2], w[3]])),
            Prec::U64 => u64::from_le_bytes(w) as f64,
            Prec::I64 => i64::from_le_bytes(w) as f64,
            Prec::F32 => f64::from(f32::from_le_bytes([w[0], w[1], w[2], w[3]])),
            Prec::F64 => f64::from_le_bytes(w),
        }
    }
}

/// A precision text: the type, and whether `fread` returns a char (`'*char'`
/// or `'...=>char'`). Every other output class is a double here.
fn precision(text: &str, name: &str) -> R<(Prec, bool)> {
    let t = text.trim();
    let (src, char_out) = if let Some(s) = t.strip_prefix('*') {
        (s, s.starts_with("char") || s == "uchar")
    } else if let Some((a, b)) = t.split_once("=>") {
        (a.trim(), b.trim() == "char")
    } else {
        (t, false)
    };
    let p = match src {
        "uint8" | "uchar" | "unsigned char" | "char" | "char*1" => Prec::U8,
        "int8" | "schar" | "signed char" | "integer*1" => Prec::I8,
        "uint16" | "ushort" => Prec::U16,
        "int16" | "short" | "integer*2" => Prec::I16,
        "uint32" | "uint" | "ulong" => Prec::U32,
        "int32" | "int" | "long" | "integer*4" => Prec::I32,
        "uint64" => Prec::U64,
        "int64" | "integer*8" => Prec::I64,
        "single" | "float32" | "float" | "real*4" => Prec::F32,
        "double" | "float64" | "real*8" => Prec::F64,
        _ => return Err(error::bad_precision(text, name)),
    };
    Ok((p, char_out))
}

/// A count or a size: a non-negative integer or `Inf`.
fn count_value(v: f64) -> Option<f64> {
    (v >= 0.0 && (v.fract() == 0.0 || v == f64::INFINITY)).then_some(v)
}

/// `[A, count] = fread(fid, size, precision)`: `size` is `Inf` (every
/// element, as a column, the default), `n` (at most `n`, as a column) or
/// `[m n]` (`m` rows, `n` of them at most or `Inf`, the last column padded
/// with zeros).
fn fread(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "fread")?;
    at_most(args, 3, "fread")?;
    let fid = fid_arg(args, 0, "fread")?;
    let (prec, char_out) = match args.get(2) {
        Some(_) => precision(&string(args, 2, "fread")?, "fread")?,
        None => (Prec::U8, false),
    };
    let size = match args.get(1) {
        Some(v) => v.mat()?.data.clone(),
        None => vec![f64::INFINITY],
    };
    let (rows, max) = match size[..] {
        [n] => (
            None,
            count_value(n).ok_or_else(|| error::bad_size_arg("fread"))?,
        ),
        [m, n] => {
            let m = count_value(m)
                .filter(|m| m.is_finite())
                .ok_or_else(|| error::bad_size_arg("fread"))?;
            let n = count_value(n).ok_or_else(|| error::bad_size_arg("fread"))?;
            (Some(m), m * n)
        }
        _ => return Err(error::bad_size_arg("fread")),
    };
    let limit = if max >= (usize::MAX / 8) as f64 {
        usize::MAX
    } else {
        max as usize * prec.size()
    };
    // One element past the most an array may hold is enough to know the
    // read is too large, and no more of the file is read than that.
    let limit = limit.min((MAX_ELEMS + 1) * prec.size());
    let bytes = it.open_files.get(fid)?.bytes(limit)?;
    check_shape((bytes.len() / prec.size()) as f64, 1.0)?;
    let mut data: Vec<f64> = bytes
        .chunks_exact(prec.size())
        .map(|c| prec.decode(c))
        .collect();
    let count = data.len();
    let (r, c) = match rows {
        None => (count, 1),
        Some(0.0) => (0, 0),
        Some(m) => {
            let cols = (count as f64 / m).ceil();
            let (r, c) = check_shape(m, cols)?;
            data.resize(r * c, 0.0);
            (r, c)
        }
    };
    let mut a = Matrix::new(r, c, data);
    if char_out {
        a = a.to_class(Class::Char)?;
    }
    Ok(vec![Value::Mat(a), num(count as f64)])
}

/// `count = fwrite(fid, A, precision)`: the elements of `A` in
/// column-major order, `uint8` by default.
fn fwrite(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "fwrite")?;
    at_most(args, 3, "fwrite")?;
    let fid = fid_arg(args, 0, "fwrite")?;
    let a = args[1].mat()?;
    let prec = match args.get(2) {
        Some(_) => precision(&string(args, 2, "fwrite")?, "fwrite")?.0,
        None => Prec::U8,
    };
    let mut bytes = Vec::with_capacity(a.numel().saturating_mul(prec.size()));
    for v in &a.data {
        prec.encode(*v, &mut bytes);
    }
    write_to(it, fid, &bytes)?;
    one(num(a.numel() as f64))
}

// ---- whole files ------------------------------------------------------------

/// A file name argument, refused when it is empty.
fn file_name(args: &[Value], i: usize, name: &str) -> R<String> {
    let f = string(args, i, name)?;
    if f.is_empty() {
        return Err(error::empty_file_name(name));
    }
    Ok(f)
}

/// A folder where a file was named is "It is a directory" on every
/// platform: Windows would otherwise answer "Permission denied" where
/// Linux answers the truth, and the reason must read the same on both.
fn not_a_folder(path: &std::path::Path) -> io::Result<()> {
    if path.is_dir() {
        return Err(io::Error::from(io::ErrorKind::IsADirectory));
    }
    Ok(())
}

fn read_file(it: &Interp, name: &str) -> R<Vec<u8>> {
    let path = it.resolve_path(name);
    not_a_folder(&path)
        .and_then(|_| fs::read(&path))
        .map_err(|e| error::cannot_read_file(name, &e))
}

pub(crate) fn write_file(it: &mut Interp, name: &str, bytes: &[u8]) -> R<()> {
    let path = it.resolve_path(name);
    not_a_folder(&path)
        .and_then(|_| fs::write(&path, bytes))
        .map_err(|e| error::cannot_write_file(name, &e))?;
    it.files_changed();
    Ok(())
}

fn fileread(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "fileread")?;
    at_most(args, 1, "fileread")?;
    let name = file_name(args, 0, "fileread")?;
    let path = it.resolve_path(&name);
    let mut bytes = Vec::new();
    not_a_folder(&path)
        .and_then(|_| File::open(&path))
        .and_then(|f| f.take(MAX_TEXT_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| error::cannot_read_file(&name, &e))?;
    if bytes.len() > MAX_TEXT_BYTES {
        return Err(text_too_long(bytes.len()));
    }
    one(text_value(&bytes)?)
}

/// How a text file's fields are separated.
#[derive(Clone, Copy)]
enum Delim {
    Byte(char),
    /// Runs of whitespace.
    White,
}

fn fields(line: &str, d: Delim) -> Vec<&str> {
    match d {
        Delim::Byte(c) => line.split(c).map(str::trim).collect(),
        Delim::White => line.split_whitespace().collect(),
    }
}

/// Judges the matrix `rows` would make so far each time a row is added,
/// `widest` being the longest row yet, so a huge text file is refused by
/// `check_shape` before its numbers take more memory than the largest
/// array could (invariant 6). Only the last row can widen it, so this is
/// constant time.
fn judge_rows(rows: &[Vec<f64>], widest: &mut usize) -> R<()> {
    *widest = (*widest).max(rows.last().map_or(0, Vec::len));
    check_shape(rows.len() as f64, *widest as f64).map(|_| ())
}

/// Rows of numbers as a matrix, short rows padded with `pad`, the shape
/// judged by `check_shape` before anything is allocated.
fn table(rows: Vec<Vec<f64>>, pad: f64) -> R<Matrix> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let (r, c) = check_shape(rows.len() as f64, cols as f64)?;
    let mut data = vec![pad; r * c];
    for (i, row) in rows.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            data[j * r + i] = *v;
        }
    }
    Ok(Matrix::new(r, c, data))
}

/// `readmatrix(name)`: the numbers of a delimited text file. The delimiter
/// is the first of a comma, a tab and a semicolon that the file holds,
/// and runs of whitespace otherwise. Leading lines with no number
/// in them are a header and skipped, blank lines are skipped, and a field
/// that is empty or not a number is `NaN`, as are the missing fields of a
/// short row.
fn readmatrix(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "readmatrix")?;
    at_most(args, 1, "readmatrix")?;
    let name = file_name(args, 0, "readmatrix")?;
    let text = String::from_utf8_lossy(&read_file(it, &name)?).into_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let delim = if text.contains(',') {
        Delim::Byte(',')
    } else if text.contains('\t') {
        Delim::Byte('\t')
    } else if text.contains(';') {
        Delim::Byte(';')
    } else {
        Delim::White
    };
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut widest = 0;
    for line in lines {
        let row: Vec<Option<f64>> = fields(line, delim).into_iter().map(parse_double).collect();
        if rows.is_empty() && row.iter().all(Option::is_none) {
            continue;
        }
        rows.push(row.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect());
        judge_rows(&rows, &mut widest)?;
    }
    one_as(table(rows, f64::NAN)?)
}

/// `csvread(name, r, c)`: a comma-separated file of numbers, from
/// zero-based row `r` and column `c` on. An empty field and the missing
/// fields of a short row are `0`; any other field that is not a number is
/// an error naming its line.
fn csvread(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "csvread")?;
    at_most(args, 3, "csvread")?;
    let name = file_name(args, 0, "csvread")?;
    let offset = |i: usize| -> R<usize> {
        match args.get(i) {
            None => Ok(0),
            Some(_) => match mat(args, i, "csvread")?.scalar_value() {
                Some(v) if v >= 0.0 && v.fract() == 0.0 => Ok(v.min(usize::MAX as f64) as usize),
                _ => Err(error::arg_nonneg_int(i + 1, "csvread")),
            },
        }
    };
    let (r0, c0) = (offset(1)?, offset(2)?);
    let text = String::from_utf8_lossy(&read_file(it, &name)?).into_owned();
    let mut rows = Vec::new();
    let mut widest = 0;
    for (k, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row = fields(line, Delim::Byte(','))
            .into_iter()
            .map(|f| match f {
                "" => Some(0.0),
                f => parse_double(f),
            })
            .collect::<Option<Vec<f64>>>()
            .ok_or_else(|| error::text_not_number(&name, k + 1))?;
        rows.push(row);
        judge_rows(&rows, &mut widest)?;
    }
    let rows: Vec<Vec<f64>> = rows
        .into_iter()
        .skip(r0)
        .map(|row| row.into_iter().skip(c0).collect())
        .collect();
    one_as(table(rows, 0.0)?)
}

/// A real matrix to write as text: a double or a logical.
fn numeric_to_write(v: &Value, name: &str) -> R<Matrix> {
    match v {
        Value::Mat(m) if m.class != Class::Char => Ok(m.clone()),
        other => Err(error::write_unsupported(other.class_name(), name)),
    }
}

/// The rows of `m`, each value through `f`, joined by `sep`, one line each.
fn delimited(m: &Matrix, sep: &str, lead: &str, f: impl Fn(f64) -> String) -> String {
    let mut out = String::new();
    for r in 0..m.rows {
        out.push_str(lead);
        let row: Vec<String> = (0..m.cols).map(|c| f(m.get(r, c))).collect();
        out.push_str(&row.join(sep));
        out.push('\n');
    }
    out
}

/// `writematrix(A, name, 'Delimiter', d)`: comma-separated by default,
/// each number with up to 15 significant digits. The file is
/// `matrix.txt` when no name is given.
fn writematrix(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "writematrix")?;
    let m = numeric_to_write(&args[0], "writematrix")?;
    let (name, rest) = match args.get(1) {
        Some(_) => (file_name(args, 1, "writematrix")?, &args[2..]),
        None => ("matrix.txt".to_string(), &args[1..]),
    };
    if rest.len() % 2 != 0 {
        return Err(error::option_pairs("writematrix"));
    }
    let mut sep = ",".to_string();
    for pair in rest.chunks(2) {
        let opt = pair[0].text().unwrap_or_default();
        if !opt.eq_ignore_ascii_case("Delimiter") {
            return Err(error::unrecognized_option(&opt, "writematrix"));
        }
        let v = pair[1].text().unwrap_or_default();
        sep = match v.as_str() {
            "," | "comma" => ",",
            " " | "space" => " ",
            "\t" | "\\t" | "tab" => "\t",
            ";" | "semi" => ";",
            "|" | "bar" => "|",
            _ => return Err(error::option_value("Delimiter", "writematrix")),
        }
        .to_string();
    }
    let text = delimited(&m, &sep, "", |v| fmt_g(v, 15));
    write_file(it, &name, text.as_bytes())?;
    none()
}

/// `csvwrite(name, A, r, c)`: comma-separated with five significant
/// digits, as MATLAB's `csvwrite` writes them, after `r` empty lines and
/// with `c` empty fields at the start of each line.
fn csvwrite(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "csvwrite")?;
    at_most(args, 4, "csvwrite")?;
    let name = file_name(args, 0, "csvwrite")?;
    let m = numeric_to_write(&args[1], "csvwrite")?;
    let offset = |i: usize| -> R<usize> {
        match args.get(i) {
            None => Ok(0),
            Some(_) => match mat(args, i, "csvwrite")?.scalar_value() {
                Some(v) if v >= 0.0 && v.fract() == 0.0 && v <= 1e6 => Ok(v as usize),
                _ => Err(error::arg_nonneg_int(i + 1, "csvwrite")),
            },
        }
    };
    let (r0, c0) = (offset(2)?, offset(3)?);
    let mut text = "\n".repeat(r0);
    text.push_str(&delimited(&m, ",", &",".repeat(c0), |v| fmt_g(v, 5)));
    write_file(it, &name, text.as_bytes())?;
    none()
}

/// `delete(name, ...)`: each file by its full name. A wildcard is refused,
/// which is safer than expanding one (a recorded deviation), and a file
/// that is not there is a warning, as in MATLAB.
fn delete(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "delete")?;
    let names = (0..args.len())
        .map(|i| file_name(args, i, "delete"))
        .collect::<R<Vec<_>>>()?;
    if names.iter().any(|n| n.contains('*')) {
        return Err(error::delete_wildcard());
    }
    for name in names {
        let path = it.resolve_path(&name);
        if !path.exists() {
            it.warn(Some(error::delete_not_found(&name)))?;
            continue;
        }
        not_a_folder(&path)
            .and_then(|_| fs::remove_file(&path))
            .map_err(|e| error::cannot_delete(&name, &e))?;
        it.files_changed();
    }
    none()
}

// ---- save and load -------------------------------------------------------------

/// `name` with `.mat` added when it has no extension of its own.
fn with_mat(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    if file.contains('.') {
        name.to_string()
    } else {
        format!("{name}.mat")
    }
}

/// The text `save -ascii` writes for one matrix: each value `%.7e`
/// (`%.16e` with `-double`) after three spaces, less one for a minus sign,
/// or tab-separated with `-tabs`.
fn ascii_text(m: &Matrix, double: bool, tabs: bool) -> String {
    let digits = if double { 16 } else { 7 };
    let width = digits + 9;
    let mut out = String::new();
    for r in 0..m.rows {
        let row: Vec<String> = (0..m.cols)
            .map(|c| {
                let t = fmt_e(m.get(r, c), digits);
                if tabs { t } else { format!("{:>width$}", t) }
            })
            .collect();
        out.push_str(&row.join(if tabs { "\t" } else { "" }));
        out.push('\n');
    }
    out
}

/// `save(name, vars..., options...)`: every variable when none is named,
/// to `matlab.mat` when no file is named. The options are `-mat`, `-v6`
/// and `-v7` (each an uncompressed MAT-file here), `-append`, and `-ascii`
/// with `-double` and `-tabs`.
fn save(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let words = (0..args.len())
        .map(|i| string(args, i, "save"))
        .collect::<R<Vec<_>>>()?;
    let (mut ascii, mut double, mut tabs, mut append) = (false, false, false, false);
    let mut plain = Vec::new();
    for w in &words {
        match w.as_str() {
            "-ascii" => ascii = true,
            "-double" => double = true,
            "-tabs" => tabs = true,
            "-append" => append = true,
            "-mat" | "-v6" | "-v7" => {}
            opt if opt.starts_with('-') => return Err(error::unrecognized_option(opt, "save")),
            _ => plain.push(w.clone()),
        }
    }
    let file = match plain.first() {
        Some(f) if f.is_empty() => return Err(error::empty_file_name("save")),
        Some(f) if ascii => f.clone(),
        Some(f) => with_mat(f),
        None => "matlab.mat".to_string(),
    };
    let mut names: Vec<String> = plain.iter().skip(1).cloned().collect();
    if names.is_empty() {
        names = it.vars().keys().cloned().collect();
        names.sort();
    }
    let mut vars = Vec::with_capacity(names.len());
    for n in names {
        let v = it
            .vars()
            .get(&n)
            .cloned()
            .ok_or_else(|| error::save_no_variable(&n))?;
        vars.push((n, v));
    }
    if ascii {
        let mut text = String::new();
        for (n, v) in &vars {
            match v {
                Value::Mat(m) if m.is_complex() => return Err(error::complex_argument("save")),
                Value::Mat(m) => text.push_str(&ascii_text(m, double, tabs)),
                other => return Err(error::save_ascii_unsupported(n, other.class_name())),
            }
        }
        if append {
            let mut old = match fs::read(it.resolve_path(&file)) {
                Ok(b) => b,
                Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
                Err(e) => return Err(error::cannot_read_file(&file, &e)),
            };
            old.extend_from_slice(text.as_bytes());
            return write_file(it, &file, &old).and_then(|_| none());
        }
        write_file(it, &file, text.as_bytes())?;
        return none();
    }
    if append && it.resolve_path(&file).exists() {
        let mut old = super::mat::read(&read_file(it, &file)?, &file)?;
        let mut at: std::collections::HashMap<String, usize> = old
            .iter()
            .enumerate()
            .map(|(k, (n, _))| (n.clone(), k))
            .collect();
        for (n, v) in vars {
            match at.get(&n) {
                Some(&k) => old[k].1 = v,
                None => {
                    at.insert(n.clone(), old.len());
                    old.push((n, v));
                }
            }
        }
        vars = old;
    }
    if vars.is_empty() {
        return Err(error::save_nothing());
    }
    let bytes = super::mat::write(&vars)?;
    write_file(it, &file, &bytes)?;
    none()
}

/// The variable `load -ascii` names after its file: the name less its
/// folder and extension, every character that cannot be in a name an
/// underscore, and an `X` first when it does not start with a letter.
fn ascii_variable(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
    let mut v: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !v.starts_with(|c: char| c.is_ascii_alphabetic()) {
        v.insert(0, 'X');
    }
    v
}

/// The numbers of a text file `load -ascii` reads: whitespace or commas
/// between them, `%` starting a comment, every line the same length.
fn ascii_matrix(bytes: &[u8], file: &str) -> R<Matrix> {
    let text = String::from_utf8_lossy(bytes);
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut widest = 0;
    for (k, line) in text.lines().enumerate() {
        let line = line.split('%').next().unwrap_or("");
        let row = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|f| !f.is_empty())
            .map(parse_double)
            .collect::<Option<Vec<f64>>>()
            .ok_or_else(|| error::text_not_number(file, k + 1))?;
        if row.is_empty() {
            continue;
        }
        if rows.first().is_some_and(|r| r.len() != row.len()) {
            return Err(error::text_ragged(file, k + 1));
        }
        rows.push(row);
        judge_rows(&rows, &mut widest)?;
    }
    table(rows, 0.0)
}

/// `load(name, vars..., options...)`: a MAT-file, or with `-ascii` or a
/// name that is not a `.mat` and a file that is not a MAT-file, a text
/// file of numbers. With no output the variables are assigned in the
/// workspace; `S = load(...)` gives a MAT-file's variables as the fields
/// of a struct and a text file's numbers as a matrix.
fn load(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    let words = (0..args.len())
        .map(|i| string(args, i, "load"))
        .collect::<R<Vec<_>>>()?;
    let (mut ascii, mut mat_format) = (false, false);
    let mut plain = Vec::new();
    for w in &words {
        match w.as_str() {
            "-ascii" => ascii = true,
            "-mat" => mat_format = true,
            opt if opt.starts_with('-') => return Err(error::unrecognized_option(opt, "load")),
            _ => plain.push(w.clone()),
        }
    }
    let mut file = match plain.first() {
        Some(f) if f.is_empty() => return Err(error::empty_file_name("load")),
        Some(f) => f.clone(),
        None => "matlab.mat".to_string(),
    };
    if !it.resolve_path(&file).exists() && with_mat(&file) != file {
        file = with_mat(&file);
    }
    let bytes = read_file(it, &file)?;
    let is_mat = mat_format
        || (!ascii
            && (file.to_ascii_lowercase().ends_with(".mat")
                || bytes.starts_with(b"MATLAB 5.0 MAT-file")));
    if !is_mat {
        let m = ascii_matrix(&bytes, &file)?;
        if nargout > 0 {
            return one_as(m);
        }
        let name = ascii_variable(&file);
        it.vars_mut().insert(name, Value::Mat(m));
        return none();
    }
    let mut vars = super::mat::read(&bytes, &file)?;
    let wanted: Vec<&String> = plain.iter().skip(1).collect();
    if !wanted.is_empty() {
        for w in &wanted {
            if !vars.iter().any(|(n, _)| n == *w) {
                it.warn(Some(error::load_no_variable(w)))?;
            }
        }
        vars.retain(|(n, _)| wanted.contains(&n));
    }
    if nargout > 0 {
        let (fields, values): (Vec<String>, Vec<Value>) = vars.into_iter().unzip();
        return one(Value::strukt(StructArray::scalar(fields, values)));
    }
    for (n, v) in vars {
        debug_assert!(is_identifier(&n));
        it.vars_mut().insert(n, v);
    }
    none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(e: crate::error::MError) -> String {
        e.msg
    }

    /// A directory of the test's own under the system's temporary folder,
    /// removed when the guard drops.
    struct Dir(std::path::PathBuf);

    impl Dir {
        fn new(tag: &str) -> Dir {
            let d = std::env::temp_dir().join(format!(
                "splatcrab-io-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos())
            ));
            fs::create_dir_all(&d).unwrap();
            Dir(d)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn interp_in(d: &Dir) -> Interp {
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        it.cwd = d.0.clone();
        it
    }

    fn s(t: &str) -> Value {
        Value::str(t)
    }

    fn call(
        it: &mut Interp,
        f: super::super::BuiltinFn,
        args: &[Value],
        n: usize,
    ) -> R<Vec<Value>> {
        f(it, args, n)
    }

    fn scalar(v: &Value) -> f64 {
        v.mat().unwrap().data[0]
    }

    #[test]
    fn the_file_id_table_takes_the_lowest_free_identifier() {
        let d = Dir::new("table");
        let mut it = interp_in(&d);
        let open =
            |it: &mut Interp, n: &str| scalar(&call(it, fopen, &[s(n), s("w")], 1).unwrap()[0]);
        assert_eq!(open(&mut it, "a.txt"), 3.0);
        assert_eq!(open(&mut it, "b.txt"), 4.0);
        assert_eq!(open(&mut it, "c.txt"), 5.0);
        call(&mut it, fclose, &[num(4.0)], 1).unwrap();
        assert_eq!(open(&mut it, "d.txt"), 4.0);
        // Closing twice, a standard stream, a fraction and a stranger are
        // each the one refusal.
        call(&mut it, fclose, &[num(4.0)], 1).unwrap();
        for bad in [4.0, 0.0, 1.0, 2.0, 3.5, -1.0, 99.0, f64::NAN] {
            let e = call(&mut it, fclose, &[num(bad)], 1).unwrap_err();
            assert_eq!(msg(e), "Invalid file identifier.", "{bad}");
        }
        call(&mut it, fclose, &[s("all")], 1).unwrap();
        assert!(it.open_files.files.is_empty());
        assert_eq!(open(&mut it, "e.txt"), 3.0);
        // A file that is not there is -1 and a reason, not an error.
        let v = call(&mut it, fopen, &[s("missing.txt")], 2).unwrap();
        assert_eq!(scalar(&v[0]), -1.0);
        assert_eq!(v[1].text().unwrap(), "No such file or directory");
        assert!(call(&mut it, fopen, &[s("x"), s("q")], 1).is_err());
        assert!(call(&mut it, fopen, &[s("x"), s("rr")], 1).is_err());
        assert!(call(&mut it, fopen, &[s("x"), s("rtb")], 1).is_err());
    }

    #[test]
    fn lines_bytes_and_the_end_of_file_flag() {
        let d = Dir::new("lines");
        let mut it = interp_in(&d);
        fs::write(d.0.join("t.txt"), b"ab\r\ncd\n\nlast").unwrap();
        let fid = call(&mut it, fopen, &[s("t.txt"), s("rt")], 1).unwrap()[0].clone();
        let get =
            |it: &mut Interp, f| call(it, f, std::slice::from_ref(&fid), 1).unwrap()[0].clone();
        assert_eq!(get(&mut it, fgetl).text().unwrap(), "ab");
        assert_eq!(get(&mut it, fgets).text().unwrap(), "cd\n");
        assert_eq!(scalar(&get(&mut it, feof)), 0.0);
        assert_eq!(get(&mut it, fgetl).text().unwrap(), "");
        assert_eq!(get(&mut it, fgetl).text().unwrap(), "last");
        assert_eq!(scalar(&get(&mut it, feof)), 1.0);
        assert_eq!(scalar(&get(&mut it, fgetl)), -1.0);
        // Writing to a file opened for reading is refused.
        let e = call(&mut it, fprintf, &[fid.clone(), s("x")], 0).unwrap_err();
        assert_eq!(msg(e), "The file is not open for writing.");
        // Binary round trip, every precision.
        let fid = call(&mut it, fopen, &[s("b.bin"), s("w+")], 1).unwrap()[0].clone();
        let vals = Value::Mat(Matrix::row(vec![1.5, -2.0, 300.0]));
        for p in ["uint8", "int16", "uint32", "int64", "single", "double"] {
            call(&mut it, fwrite, &[fid.clone(), vals.clone(), s(p)], 1).unwrap();
        }
        call(&mut it, fclose, &[fid], 1).unwrap();
        let fid = call(&mut it, fopen, &[s("b.bin")], 1).unwrap()[0].clone();
        let read = |it: &mut Interp, p: &str| {
            let v = call(it, fread, &[fid.clone(), num(3.0), s(p)], 2).unwrap();
            v[0].mat().unwrap().data.clone()
        };
        // uint8 saturates and rounds: 1.5 is 2, -2 is 0, 300 is 255.
        assert_eq!(read(&mut it, "uint8"), [2.0, 0.0, 255.0]);
        assert_eq!(read(&mut it, "int16"), [2.0, -2.0, 300.0]);
        assert_eq!(read(&mut it, "uint32"), [2.0, 0.0, 300.0]);
        assert_eq!(read(&mut it, "int64"), [2.0, -2.0, 300.0]);
        assert_eq!(read(&mut it, "single"), [1.5, -2.0, 300.0]);
        assert_eq!(read(&mut it, "double"), [1.5, -2.0, 300.0]);
        assert_eq!(read(&mut it, "double"), Vec::<f64>::new());
        assert!(call(&mut it, fread, &[fid.clone(), num(1.0), s("int9")], 1).is_err());
        // A shape far past what the file holds is judged by its data, and
        // a rows count past the size limit is refused, never allocated.
        let v = call(&mut it, fread, &[fid.clone(), num(1e15)], 1).unwrap();
        assert_eq!(v[0].mat().unwrap().rows, 0);
        call(&mut it, fclose, &[fid], 1).unwrap();
        fs::write(d.0.join("c.bin"), b"abcde").unwrap();
        let fid = call(&mut it, fopen, &[s("c.bin")], 1).unwrap()[0].clone();
        let shape = Value::Mat(Matrix::row(vec![2.0, f64::INFINITY]));
        let v = call(&mut it, fread, &[fid.clone(), shape, s("*char")], 2).unwrap();
        let m = v[0].mat().unwrap();
        assert_eq!((m.rows, m.cols, m.class), (2, 3, Class::Char));
        assert_eq!(m.data, [97.0, 98.0, 99.0, 100.0, 101.0, 0.0]);
        assert_eq!(scalar(&v[1]), 5.0);
        call(&mut it, fclose, &[fid], 1).unwrap();
        let fid = call(&mut it, fopen, &[s("c.bin")], 1).unwrap()[0].clone();
        let shape = Value::Mat(Matrix::row(vec![1e12, 2.0]));
        assert!(call(&mut it, fread, &[fid, shape], 1).is_err());
    }

    #[test]
    fn fprintf_counts_its_bytes_and_refuses_a_stranger() {
        let d = Dir::new("fprintf");
        let mut it = interp_in(&d);
        let n = call(&mut it, fprintf, &[s("é\\n")], 1).unwrap();
        assert_eq!(scalar(&n[0]), 3.0);
        let e = call(&mut it, fprintf, &[num(7.0), s("x")], 0).unwrap_err();
        assert_eq!(msg(e), "Invalid file identifier.");
        let e = call(&mut it, fprintf, &[num(0.0), s("x")], 0).unwrap_err();
        assert_eq!(msg(e), "Invalid file identifier.");
        assert!(call(&mut it, fprintf, &[num(1.0), s("x")], 0).is_ok());
        assert!(call(&mut it, fprintf, &[num(2.0), s("x")], 0).is_ok());
    }

    #[test]
    fn whole_files_and_delete() {
        let d = Dir::new("whole");
        let mut it = interp_in(&d);
        let m = Value::Mat(Matrix::new(2, 2, vec![1.0, 3.0, 2.5, 4.0]));
        call(&mut it, writematrix, &[m.clone(), s("m.csv")], 0).unwrap();
        assert_eq!(
            fs::read_to_string(d.0.join("m.csv")).unwrap(),
            "1,2.5\n3,4\n"
        );
        let back = call(&mut it, readmatrix, &[s("m.csv")], 1).unwrap();
        assert_eq!(back[0].mat().unwrap().data, [1.0, 3.0, 2.5, 4.0]);
        fs::write(d.0.join("h.csv"), "a,b\n1,,x\n2\n").unwrap();
        let back = call(&mut it, readmatrix, &[s("h.csv")], 1).unwrap();
        let back = back[0].mat().unwrap();
        assert_eq!((back.rows, back.cols), (2, 3));
        assert_eq!(back.data[0..2], [1.0, 2.0]);
        assert!(back.data[2..].iter().all(|v| v.is_nan()));
        call(
            &mut it,
            csvwrite,
            &[s("c.csv"), Value::Mat(Matrix::row(vec![1.0 / 3.0, 2.0]))],
            0,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(d.0.join("c.csv")).unwrap(),
            "0.33333,2\n"
        );
        fs::write(d.0.join("r.csv"), "1,2,3\n4,,6\n7,8\n").unwrap();
        let back = call(&mut it, csvread, &[s("r.csv"), num(1.0), num(1.0)], 1).unwrap();
        assert_eq!(back[0].mat().unwrap().data, [0.0, 8.0, 6.0, 0.0]);
        fs::write(d.0.join("bad.csv"), "1,2\n3,x\n").unwrap();
        let e = call(&mut it, csvread, &[s("bad.csv")], 1).unwrap_err();
        assert!(msg(e).contains("line 2"));
        // A ragged file whose longest line and line count multiply past the
        // limit is refused before the matrix is allocated.
        let mut ragged = "1\n".repeat(20_000);
        ragged.push_str(&",".repeat(20_000));
        ragged.push_str("1\n");
        fs::write(d.0.join("wide.csv"), ragged).unwrap();
        let e = call(&mut it, readmatrix, &[s("wide.csv")], 1).unwrap_err();
        assert!(msg(e).contains("exceeds the maximum array size"));
        let text = call(&mut it, fileread, &[s("c.csv")], 1).unwrap();
        assert_eq!(text[0].text().unwrap(), "0.33333,2\n");
        let e = call(&mut it, delete, &[s("*.csv")], 0).unwrap_err();
        assert_eq!(msg(e), "Wildcards are not supported by 'delete'.");
        assert!(d.0.join("c.csv").exists());
        call(&mut it, delete, &[s("c.csv")], 0).unwrap();
        assert!(!d.0.join("c.csv").exists());
        // A file that is not there is a warning, not an error.
        call(&mut it, delete, &[s("c.csv")], 0).unwrap();
        let e = call(&mut it, fileread, &[s("c.csv")], 1).unwrap_err();
        assert_eq!(
            msg(e),
            "Unable to read file 'c.csv': No such file or directory."
        );
        // A folder is "It is a directory" on every platform, for reading,
        // writing and deleting alike, and delete leaves it alone.
        let e = call(&mut it, fileread, &[s(".")], 1).unwrap_err();
        assert_eq!(msg(e), "Unable to read file '.': It is a directory.");
        let e = call(&mut it, readmatrix, &[s(".")], 1).unwrap_err();
        assert_eq!(msg(e), "Unable to read file '.': It is a directory.");
        let one_value = Value::Mat(Matrix::scalar(1.0));
        let e = call(&mut it, writematrix, &[one_value, s(".")], 0).unwrap_err();
        assert_eq!(msg(e), "Unable to write file '.': It is a directory.");
        let e = call(&mut it, delete, &[s(".")], 0).unwrap_err();
        assert_eq!(msg(e), "Unable to delete file '.': It is a directory.");
        assert!(d.0.is_dir());
    }

    #[test]
    fn text_past_the_largest_row_is_refused_by_its_length() {
        // Three bytes can hold one code unit at most, so text past three
        // times the element limit is too long whatever it holds.
        assert_eq!(MAX_TEXT_BYTES, 3 * MAX_ELEMS);
        let e = text_too_long(MAX_TEXT_BYTES + 1);
        assert!(msg(e).contains("exceeds the maximum array size"));
        // The rows of a text file are judged as they are read.
        let mut widest = 0;
        let rows = vec![vec![0.0; 3]; 2];
        assert!(judge_rows(&rows, &mut widest).is_ok());
        assert_eq!(widest, 3);
        let wide = vec![Vec::new(), vec![0.0; 2]];
        assert!(judge_rows(&wide, &mut widest).is_ok());
        assert_eq!(widest, 3);
    }

    #[test]
    fn save_and_load_round_trip_both_formats() {
        let d = Dir::new("save");
        let mut it = interp_in(&d);
        let x = Value::Mat(Matrix::new(2, 2, vec![1.0, 3.0, -2.0, 4.5]));
        it.vars_mut().insert("x".into(), x.clone());
        it.vars_mut().insert("s".into(), s("hi"));
        call(&mut it, save, &[s("t"), s("x"), s("s")], 0).unwrap();
        assert!(d.0.join("t.mat").exists());
        it.vars_mut().clear();
        call(&mut it, load, &[s("t")], 0).unwrap();
        assert_eq!(it.vars()["x"].mat().unwrap().data, [1.0, 3.0, -2.0, 4.5]);
        assert_eq!(it.vars()["s"].text().unwrap(), "hi");
        let st = call(&mut it, load, &[s("t.mat"), s("s")], 1).unwrap();
        let Value::Struct(st) = &st[0] else { panic!() };
        assert_eq!(st.fields, ["s"]);
        // -ascii: MATLAB's %.7e layout, read back as a double named after
        // the file.
        call(&mut it, save, &[s("a.txt"), s("x"), s("-ascii")], 0).unwrap();
        let text = fs::read_to_string(d.0.join("a.txt")).unwrap();
        assert_eq!(
            text,
            "   1.0000000e+00  -2.0000000e+00\n   3.0000000e+00   4.5000000e+00\n"
        );
        call(&mut it, load, &[s("a.txt")], 0).unwrap();
        assert_eq!(it.vars()["a"].mat().unwrap().data, [1.0, 3.0, -2.0, 4.5]);
        assert_eq!(ascii_variable("dir/2nd-file.txt"), "X2nd_file");
        let e = call(&mut it, save, &[s("q.mat"), s("nope")], 0).unwrap_err();
        assert_eq!(msg(e), "Variable 'nope' not found.");
        fs::write(d.0.join("r.txt"), "1 2\n3\n").unwrap();
        assert!(msg(call(&mut it, load, &[s("r.txt")], 1).unwrap_err()).contains("line 2"));
        // A truncated MAT-file is a clean error.
        let full = fs::read(d.0.join("t.mat")).unwrap();
        fs::write(d.0.join("cut.mat"), &full[..full.len() - 3]).unwrap();
        let e = call(&mut it, load, &[s("cut.mat")], 0).unwrap_err();
        assert!(msg(e).starts_with("Unable to read MAT-file 'cut.mat'"));
        // An empty workspace is not saved: the file would be a header
        // alone, which load refuses.
        let mut empty = interp_in(&d);
        let e = call(&mut empty, save, &[s("none.mat")], 0).unwrap_err();
        assert_eq!(msg(e), "There are no variables to save.");
        assert!(!d.0.join("none.mat").exists());
    }

    #[test]
    fn input_reads_a_line_and_is_refused_without_a_terminal() {
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
        it.input = crate::interp::InputSource::Reader(Box::new(io::Cursor::new(
            b"6 * 7\nBob\n\n".to_vec(),
        )));
        let v = call(&mut it, input, &[s("n: ")], 1).unwrap();
        assert_eq!(scalar(&v[0]), 42.0);
        let v = call(&mut it, input, &[s("name: "), s("s")], 1).unwrap();
        assert_eq!(v[0].text().unwrap(), "Bob");
        let v = call(&mut it, input, &[s("")], 1).unwrap();
        assert!(v[0].mat().unwrap().is_empty());
        let e = call(&mut it, input, &[s("")], 1).unwrap_err();
        assert_eq!(msg(e), "'input' reached the end of standard input.");
        it.input = crate::interp::InputSource::Refused;
        let e = call(&mut it, input, &[s("n: ")], 1).unwrap_err();
        assert_eq!(
            msg(e),
            "input is not available in this session: there is no terminal to read from."
        );
    }
}
