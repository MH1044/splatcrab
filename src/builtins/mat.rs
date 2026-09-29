//! MAT-file version 5, uncompressed: the format `save` writes and `load`
//! reads (cycle 11). Pure functions over bytes, with no `Interp`, so a unit
//! test can hand the reader anything.
//!
//! **The reader treats the file as untrusted input.** Every length it reads
//! is checked against the bytes that are actually there before anything is
//! sliced, every array's dimensions go through `args::check_shape` before
//! anything is allocated, a container never reserves room for more
//! elements than the bytes left could hold, and nesting is bounded by
//! [`MAX_DEPTH`], so a truncated, corrupt or hostile file is a clean error:
//! never a panic, an unbounded allocation or a stack overflow.
//!
//! What is read: double, single and the eight integer classes (all as
//! double, since SplatCrab has no other numeric class), complex values,
//! logicals, chars, cells and structs, in either byte order, with numeric
//! data stored under any of the numeric data types MATLAB uses to save
//! space. What is refused: compressed data (MATLAB's default since v7;
//! save with `-v6` there), sparse arrays, objects, function handles and
//! arrays of more than two dimensions. What is written: doubles (complex
//! included), logicals, chars, cells and structs, little-endian, each
//! numeric array as `miDOUBLE`.

use std::collections::{HashMap, HashSet};

use super::args::check_shape;
use crate::error::{self, MError, MatFault, R};
use crate::value::{CellArray, Class, Matrix, StructArray, Value};

/// How deeply cells and structs may nest in a file that is read or
/// written. Both directions recurse once per level.
pub const MAX_DEPTH: usize = 200;

/// The most elements a struct array with no fields may have, read or
/// written. Every other array's elements are paid for by the file's bytes,
/// at least one per element or eight per field value, so the reader bounds
/// them by the bytes that are there; a struct array with no fields holds no
/// data at all, and MATLAB writes a 1-by-N one in a few bytes whatever N
/// is. Each element still costs SplatCrab an empty field list, 24 bytes,
/// so without a bound of its own a 200-byte file claiming 16384-by-16384
/// took 6 GB and most of a minute to load. At 2^20 (1,048,576, a 256th of
/// the element limit) such an array costs 24 MB and milliseconds at most,
/// and a struct array with no fields is a placeholder that no honest file
/// holds a million of. `save` refuses one past it too, so SplatCrab never
/// writes a file its own `load` refuses.
pub const MAX_FIELDLESS: usize = 1 << 20;

/// The length of the header's descriptive text.
const TEXT: usize = 116;
/// The header's length.
const HEADER: usize = 128;

// Data types.
const MI_INT8: u32 = 1;
const MI_UINT8: u32 = 2;
const MI_INT16: u32 = 3;
const MI_UINT16: u32 = 4;
const MI_INT32: u32 = 5;
const MI_UINT32: u32 = 6;
const MI_SINGLE: u32 = 7;
const MI_DOUBLE: u32 = 9;
const MI_INT64: u32 = 12;
const MI_UINT64: u32 = 13;
const MI_MATRIX: u32 = 14;
const MI_COMPRESSED: u32 = 15;
const MI_UTF8: u32 = 16;
const MI_UTF16: u32 = 17;

// Array classes.
const MX_CELL: u32 = 1;
const MX_STRUCT: u32 = 2;
const MX_OBJECT: u32 = 3;
const MX_CHAR: u32 = 4;
const MX_SPARSE: u32 = 5;
const MX_DOUBLE: u32 = 6;
const MX_UINT8: u32 = 9;

// Array flags, in the second byte of the flags word.
const FLAG_COMPLEX: u32 = 0x08;
const FLAG_LOGICAL: u32 = 0x02;

// ---- writing -----------------------------------------------------------

/// A whole file holding `vars`, in order. A function handle or an
/// `MException` among them is refused, naming its variable.
pub fn write(vars: &[(String, Value)]) -> R<Vec<u8>> {
    let mut out = header();
    for (name, v) in vars {
        matrix(&mut out, name, v, name, 0)?;
    }
    Ok(out)
}

fn header() -> Vec<u8> {
    let mut h = b"MATLAB 5.0 MAT-file, Platform: SplatCrab, Created by: SplatCrab".to_vec();
    h.resize(TEXT, b' ');
    // No subsystem data.
    h.extend_from_slice(&[0; 8]);
    // Version 0x0100, then the endian indicator, both little-endian.
    h.extend_from_slice(&0x0100u16.to_le_bytes());
    h.extend_from_slice(b"IM");
    h
}

fn pad(out: &mut Vec<u8>) {
    while out.len() % 8 != 0 {
        out.push(0);
    }
}

/// A length as the `u32` a tag holds it in: a length past 4 GiB would be
/// written wrapped, and the file silently corrupt, so `var` is refused.
fn len32(n: usize, var: &str) -> R<u32> {
    u32::try_from(n).map_err(|_| error::save_too_large(var))
}

/// One data element of `var`: its tag, its bytes and the padding to eight.
fn element(out: &mut Vec<u8>, ty: u32, data: &[u8], var: &str) -> R<()> {
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(&len32(data.len(), var)?.to_le_bytes());
    out.extend_from_slice(data);
    pad(out);
    Ok(())
}

/// An `miMATRIX` element for `v`, named `name` (empty inside a container).
/// `var` is the variable it belongs to, for the refusal.
fn matrix(out: &mut Vec<u8>, name: &str, v: &Value, var: &str, depth: usize) -> R<()> {
    if depth > MAX_DEPTH {
        return Err(error::save_too_deep(var));
    }
    let start = out.len();
    out.extend_from_slice(&MI_MATRIX.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    let (rows, cols) = v.dims();
    let flags = |out: &mut Vec<u8>, class: u32, bits: u32| -> R<()> {
        let mut d = (class | (bits << 8)).to_le_bytes().to_vec();
        d.extend_from_slice(&0u32.to_le_bytes());
        element(out, MI_UINT32, &d, var)?;
        let mut d = (rows as i32).to_le_bytes().to_vec();
        d.extend_from_slice(&(cols as i32).to_le_bytes());
        element(out, MI_INT32, &d, var)?;
        element(out, MI_INT8, name.as_bytes(), var)
    };
    match v {
        Value::Mat(m) => match m.class {
            Class::Double => {
                let bits = if m.is_complex() { FLAG_COMPLEX } else { 0 };
                flags(out, MX_DOUBLE, bits)?;
                let re: Vec<u8> = m.data.iter().flat_map(|x| x.to_le_bytes()).collect();
                element(out, MI_DOUBLE, &re, var)?;
                if let Some(im) = &m.im {
                    let im: Vec<u8> = im.iter().flat_map(|x| x.to_le_bytes()).collect();
                    element(out, MI_DOUBLE, &im, var)?;
                }
            }
            Class::Logical => {
                flags(out, MX_UINT8, FLAG_LOGICAL)?;
                let d: Vec<u8> = m.data.iter().map(|x| u8::from(*x != 0.0)).collect();
                element(out, MI_UINT8, &d, var)?;
            }
            Class::Char => {
                flags(out, MX_CHAR, 0)?;
                let d: Vec<u8> = m
                    .data
                    .iter()
                    .flat_map(|x| (*x as u16).to_le_bytes())
                    .collect();
                element(out, MI_UINT16, &d, var)?;
            }
        },
        Value::Cell(c) => {
            flags(out, MX_CELL, 0)?;
            for item in &c.data {
                matrix(out, "", item, var, depth + 1)?;
            }
        }
        Value::Struct(s) => {
            if s.fields.is_empty() && rows.saturating_mul(cols) > MAX_FIELDLESS {
                return Err(error::save_fieldless_too_large(var, MAX_FIELDLESS));
            }
            flags(out, MX_STRUCT, 0)?;
            let len = s
                .fields
                .iter()
                .map(|f| f.len() + 1)
                .max()
                .unwrap_or(1)
                .max(32);
            // The field-name length, as a small element.
            out.extend_from_slice(&(MI_INT32 | (4 << 16)).to_le_bytes());
            out.extend_from_slice(&(len as i32).to_le_bytes());
            let mut names = Vec::with_capacity(len * s.fields.len());
            for f in &s.fields {
                let mut b = f.as_bytes().to_vec();
                b.resize(len, 0);
                names.extend_from_slice(&b);
            }
            element(out, MI_INT8, &names, var)?;
            for e in &s.elems {
                for item in e {
                    matrix(out, "", item, var, depth + 1)?;
                }
            }
        }
        other => return Err(error::save_unsupported(var, other.class_name())),
    }
    // A cell or a struct can hold more than a tag can count: several arrays
    // of 2 GB each, the most an array of doubles can be.
    let n = len32(out.len() - start - 8, var)?;
    out[start + 4..start + 8].copy_from_slice(&n.to_le_bytes());
    Ok(())
}

// ---- reading -----------------------------------------------------------

/// Every variable in the MAT file `bytes`, in the order it holds them.
/// `file` names the file in the errors.
pub fn read(bytes: &[u8], file: &str) -> R<Vec<(String, Value)>> {
    let bad = |fault: MatFault| error::mat_file(file, fault);
    if bytes.len() < HEADER {
        return Err(bad(MatFault::Truncated));
    }
    let big = match &bytes[126..128] {
        b"IM" => false,
        b"MI" => true,
        _ => return Err(bad(MatFault::NotVersion5)),
    };
    let version = if big {
        u16::from_be_bytes([bytes[124], bytes[125]])
    } else {
        u16::from_le_bytes([bytes[124], bytes[125]])
    };
    if version != 0x0100 {
        return Err(bad(MatFault::NotVersion5));
    }
    let mut r = Reader {
        b: &bytes[HEADER..],
        pos: 0,
        big,
        file,
        inner: false,
    };
    let mut vars: Vec<(String, Value)> = Vec::new();
    // Where each name is in `vars`: a name the file holds twice keeps its
    // first place and its last value, as loading them in turn would leave
    // it, and `S = load(...)` never makes a struct with a field twice.
    let mut seen: HashMap<String, usize> = HashMap::new();
    while r.pos < r.b.len() {
        let (ty, data) = r.element()?;
        match ty {
            MI_MATRIX => {
                let (name, v) = r.sub(data).matrix(0)?;
                if !is_name(&name) {
                    return Err(bad(MatFault::Corrupt));
                }
                match seen.get(&name) {
                    Some(&k) => vars[k].1 = v,
                    None => {
                        seen.insert(name.clone(), vars.len());
                        vars.push((name, v));
                    }
                }
            }
            MI_COMPRESSED => {
                return Err(bad(MatFault::Compressed));
            }
            _ => return Err(bad(MatFault::Corrupt)),
        }
    }
    if vars.is_empty() {
        return Err(bad(MatFault::HeaderOnly));
    }
    Ok(vars)
}

fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic())
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
}

/// A cursor over one run of data elements.
struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
    big: bool,
    file: &'a str,
    /// Inside an element whose own length was there in full: running out
    /// of bytes here is a length that lies, not a file cut short.
    inner: bool,
}

impl<'a> Reader<'a> {
    fn bad(&self, fault: MatFault) -> MError {
        error::mat_file(self.file, fault)
    }

    /// Bytes that are not there: at the top level the file is cut short;
    /// inside a complete element a length in it claims more than it holds,
    /// and the file is corrupt.
    fn truncated(&self) -> MError {
        self.bad(if self.inner {
            MatFault::Corrupt
        } else {
            MatFault::Truncated
        })
    }

    fn sub(&self, b: &'a [u8]) -> Reader<'a> {
        Reader {
            b,
            pos: 0,
            big: self.big,
            file: self.file,
            inner: true,
        }
    }

    fn u32_at(&self, k: usize) -> Option<u32> {
        let s: [u8; 4] = self.b.get(k..k + 4)?.try_into().ok()?;
        Some(if self.big {
            u32::from_be_bytes(s)
        } else {
            u32::from_le_bytes(s)
        })
    }

    /// The next data element's type and bytes; the cursor moves past it and
    /// its padding. The bytes are a slice of the input, checked to be there.
    fn element(&mut self) -> R<(u32, &'a [u8])> {
        let first = self.u32_at(self.pos).ok_or_else(|| self.truncated())?;
        // A small element packs its length into the first word's upper half
        // and its data into the second word; the word is written in the
        // file's byte order like any other, so the test is the same in both.
        if first >> 16 != 0 {
            let ty = first & 0xFFFF;
            let n = (first >> 16) as usize;
            if n > 4 {
                return Err(self.bad(MatFault::Corrupt));
            }
            let start = self.pos + 4;
            let data = self
                .b
                .get(start..start + n)
                .ok_or_else(|| self.truncated())?;
            if self.b.len() < self.pos + 8 {
                return Err(self.truncated());
            }
            self.pos += 8;
            return Ok((ty, data));
        }
        let n = self.u32_at(self.pos + 4).ok_or_else(|| self.truncated())? as usize;
        let start = self.pos + 8;
        let end = start.checked_add(n).ok_or_else(|| self.truncated())?;
        let data = self.b.get(start..end).ok_or_else(|| self.truncated())?;
        // Padding to eight; a file that ends without the last element's
        // padding is still read.
        let padded = end.div_ceil(8).saturating_mul(8);
        self.pos = padded.min(self.b.len());
        Ok((first, data))
    }

    /// The next element, which must be of one of `types`.
    fn expect(&mut self, types: &[u32]) -> R<(u32, &'a [u8])> {
        if self.pos >= self.b.len() {
            return Err(self.truncated());
        }
        let (ty, d) = self.element()?;
        if !types.contains(&ty) {
            return Err(self.bad(MatFault::Corrupt));
        }
        Ok((ty, d))
    }

    fn words(&self, d: &[u8]) -> Vec<i64> {
        d.chunks_exact(4)
            .map(|c| {
                let s = [c[0], c[1], c[2], c[3]];
                i64::from(if self.big {
                    i32::from_be_bytes(s)
                } else {
                    i32::from_le_bytes(s)
                })
            })
            .collect()
    }

    /// The contents of one `miMATRIX` element: its name and its value.
    fn matrix(&mut self, depth: usize) -> R<(String, Value)> {
        if depth > MAX_DEPTH {
            return Err(self.bad(MatFault::TooDeep));
        }
        // MATLAB writes an empty array inside a container as an element
        // with no bytes at all.
        if self.b.is_empty() {
            return Ok((String::new(), Value::Mat(Matrix::empty())));
        }
        let (_, f) = self.expect(&[MI_UINT32])?;
        if f.len() != 8 {
            return Err(self.bad(MatFault::Corrupt));
        }
        let word = self.u32_at_slice(f);
        let class = word & 0xFF;
        let bits = (word >> 8) & 0xFF;
        let (_, d) = self.expect(&[MI_INT32])?;
        if d.len() < 8 || d.len() % 4 != 0 {
            return Err(self.bad(MatFault::Corrupt));
        }
        let dims = self.words(d);
        if dims.iter().any(|&k| k < 0) {
            return Err(self.bad(MatFault::Corrupt));
        }
        if dims[2..].iter().any(|&k| k != 1) {
            return Err(self.bad(MatFault::NDims));
        }
        let (rows, cols) = check_shape(dims[0] as f64, dims[1] as f64)?;
        let numel = rows * cols;
        let (_, n) = self.expect(&[MI_INT8, MI_UINT8, MI_UTF8])?;
        let name = String::from_utf8_lossy(n)
            .trim_end_matches('\0')
            .to_string();
        let value = match class {
            MX_CELL => {
                let mut items = Vec::with_capacity(numel.min(self.left() / 8));
                for _ in 0..numel {
                    items.push(self.child(depth)?);
                }
                Value::cell(CellArray::new(rows, cols, items))
            }
            MX_STRUCT => self.strukt(rows, cols, depth)?,
            MX_OBJECT => return Err(self.bad(MatFault::Object)),
            MX_SPARSE => {
                return Err(self.bad(MatFault::Sparse));
            }
            MX_CHAR => {
                let (ty, data) = self.expect(&[MI_UINT16, MI_UINT8, MI_INT8, MI_UTF8, MI_UTF16])?;
                // Each count is judged against the array's before anything
                // the data's own length would size is allocated.
                let units: Vec<f64> = match ty {
                    MI_UTF8 => {
                        let text = String::from_utf8_lossy(data);
                        if text.encode_utf16().count() != numel {
                            return Err(self.bad(MatFault::Corrupt));
                        }
                        text.encode_utf16().map(f64::from).collect()
                    }
                    MI_UINT8 | MI_INT8 if data.len() == numel => {
                        data.iter().map(|&b| f64::from(b)).collect()
                    }
                    MI_UINT8 | MI_INT8 => return Err(self.bad(MatFault::Corrupt)),
                    _ => self.numbers(MI_UINT16, data, numel)?,
                };
                Value::Mat(Matrix::new(rows, cols, units).with_class(Class::Char))
            }
            6..=15 => {
                let re = self.numeric(numel)?;
                let im = if bits & FLAG_COMPLEX != 0 {
                    Some(self.numeric(numel)?)
                } else {
                    None
                };
                let m = Matrix::new(rows, cols, re);
                if bits & FLAG_LOGICAL != 0 {
                    let data = m
                        .data
                        .iter()
                        .map(|x| f64::from(u8::from(*x != 0.0)))
                        .collect();
                    Value::Mat(Matrix::new(rows, cols, data).with_class(Class::Logical))
                } else {
                    Value::Mat(m.with_im(im))
                }
            }
            16 | 17 => {
                return Err(self.bad(MatFault::Object));
            }
            _ => return Err(self.bad(MatFault::Corrupt)),
        };
        Ok((name, value))
    }

    fn u32_at_slice(&self, f: &[u8]) -> u32 {
        let s = [f[0], f[1], f[2], f[3]];
        if self.big {
            u32::from_be_bytes(s)
        } else {
            u32::from_le_bytes(s)
        }
    }

    fn left(&self) -> usize {
        self.b.len().saturating_sub(self.pos)
    }

    /// One element of a cell or a struct: an `miMATRIX` of its own.
    fn child(&mut self, depth: usize) -> R<Value> {
        let (_, data) = self.expect(&[MI_MATRIX])?;
        let (_, v) = self.sub(data).matrix(depth + 1)?;
        Ok(v)
    }

    fn strukt(&mut self, rows: usize, cols: usize, depth: usize) -> R<Value> {
        let (_, l) = self.expect(&[MI_INT32])?;
        let len = self.words(l).first().copied().unwrap_or(0);
        if len < 1 || l.len() != 4 {
            return Err(self.bad(MatFault::Corrupt));
        }
        let len = len as usize;
        let (_, names) = self.expect(&[MI_INT8, MI_UINT8])?;
        if names.len() % len != 0 {
            return Err(self.bad(MatFault::Corrupt));
        }
        let mut fields: Vec<String> = Vec::new();
        // A set, not a scan of `fields`: a struct of 100,000 fields is
        // judged in linear time, not quadratic.
        let mut seen: HashSet<String> = HashSet::new();
        for chunk in names.chunks(len) {
            let end = chunk.iter().position(|&b| b == 0).unwrap_or(chunk.len());
            let f = String::from_utf8_lossy(&chunk[..end]).into_owned();
            if !is_name(&f) || !seen.insert(f.clone()) {
                return Err(self.bad(MatFault::Corrupt));
            }
            fields.push(f);
        }
        let numel = rows * cols;
        // With no fields the bytes bound nothing; see `MAX_FIELDLESS`.
        if fields.is_empty() && numel > MAX_FIELDLESS {
            return Err(self.bad(MatFault::FieldlessTooLarge(MAX_FIELDLESS)));
        }
        let total = numel
            .checked_mul(fields.len())
            .ok_or_else(|| self.bad(MatFault::Corrupt))?;
        // Each value is at least one eight-byte tag.
        if total > self.left() / 8 {
            return Err(self.truncated());
        }
        let mut elems = Vec::with_capacity(numel);
        for _ in 0..numel {
            let mut e = Vec::with_capacity(fields.len());
            for _ in 0..fields.len() {
                e.push(self.child(depth)?);
            }
            elems.push(e);
        }
        Ok(Value::strukt(StructArray::new(rows, cols, fields, elems)))
    }

    /// One numeric data element of `numel` values, of any numeric type.
    fn numeric(&mut self, numel: usize) -> R<Vec<f64>> {
        let (ty, data) = self.expect(&[
            MI_INT8, MI_UINT8, MI_INT16, MI_UINT16, MI_INT32, MI_UINT32, MI_SINGLE, MI_DOUBLE,
            MI_INT64, MI_UINT64,
        ])?;
        self.numbers(ty, data, numel)
    }

    /// The `numel` values of a numeric data element, as doubles. A data
    /// element holding any other count is corrupt, judged before a value is
    /// converted, so its length never sizes an allocation.
    fn numbers(&self, ty: u32, d: &[u8], numel: usize) -> R<Vec<f64>> {
        let size = match ty {
            MI_INT8 | MI_UINT8 => 1,
            MI_INT16 | MI_UINT16 => 2,
            MI_INT32 | MI_UINT32 | MI_SINGLE => 4,
            _ => 8,
        };
        if numel.checked_mul(size) != Some(d.len()) {
            return Err(self.bad(MatFault::Corrupt));
        }
        let big = self.big;
        Ok(d.chunks_exact(size)
            .map(|c| {
                let mut b = [0u8; 8];
                b[..size].copy_from_slice(c);
                if big {
                    b[..size].reverse();
                }
                match ty {
                    MI_INT8 => f64::from(b[0] as i8),
                    MI_UINT8 => f64::from(b[0]),
                    MI_INT16 => f64::from(i16::from_le_bytes([b[0], b[1]])),
                    MI_UINT16 => f64::from(u16::from_le_bytes([b[0], b[1]])),
                    MI_INT32 => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    MI_UINT32 => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    MI_SINGLE => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    MI_INT64 => i64::from_le_bytes(b) as f64,
                    MI_UINT64 => u64::from_le_bytes(b) as f64,
                    _ => f64::from_le_bytes(b),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    fn round_trip(vars: Vec<(String, Value)>) -> Vec<(String, Value)> {
        read(&write(&vars).unwrap(), "t.mat").unwrap()
    }

    fn err(bytes: &[u8]) -> String {
        read(bytes, "t.mat").unwrap_err().msg
    }

    /// One data element, built by hand.
    fn put(out: &mut Vec<u8>, ty: u32, data: &[u8]) {
        element(out, ty, data, "t").unwrap();
    }

    #[test]
    fn every_kind_of_value_survives_a_round_trip() {
        let x = Value::Mat(Matrix::new(2, 2, vec![1.0, 3.0, 2.0, 4.0]));
        let z = Value::Mat(Matrix::complex_parts(1, 2, vec![1.0, 2.0], vec![0.5, -1.0]));
        let t = Value::Mat(Matrix::row(vec![1.0, 0.0]).with_class(Class::Logical));
        let s = Value::str("héllo😀");
        let c = Value::cell(CellArray::new(
            1,
            3,
            vec![num(1.0), Value::str("a"), Value::Mat(Matrix::empty())],
        ));
        let st = Value::strukt(StructArray::new(
            1,
            2,
            vec!["a".into(), "long_field_name_past_thirty_one_chars".into()],
            vec![vec![num(1.0), s.clone()], vec![c.clone(), num(2.0)]],
        ));
        let vars = vec![
            ("x".to_string(), x),
            ("z".to_string(), z),
            ("t".to_string(), t),
            ("s".to_string(), s),
            ("c".to_string(), c),
            ("st".to_string(), st),
        ];
        let back = round_trip(vars.clone());
        assert_eq!(back.len(), vars.len());
        for ((n1, v1), (n2, v2)) in vars.iter().zip(&back) {
            assert_eq!(n1, n2);
            assert_eq!(format!("{v1:?}"), format!("{v2:?}"), "{n1}");
        }
    }

    #[test]
    fn a_handle_cannot_be_saved() {
        let f = Value::Exception(crate::error::MError::new("x"));
        let e = write(&[("e".into(), f)]).unwrap_err().msg;
        assert_eq!(
            e,
            "Unable to save variable 'e': a value of class 'MException' cannot be saved."
        );
    }

    /// Acceptance test 16's two files, and every cut of a good file: each is
    /// a clean error, never a panic.
    #[test]
    fn truncated_and_hostile_files_are_clean_errors() {
        let good = write(&[
            ("x".into(), Value::Mat(Matrix::row(vec![1.0, 2.0, 3.0]))),
            (
                "c".into(),
                Value::cell(CellArray::new(1, 2, vec![num(1.0), Value::str("ab")])),
            ),
        ])
        .unwrap();
        assert_eq!(read(&good, "t.mat").unwrap().len(), 2);
        for cut in 0..good.len() {
            // Only the cut after the first variable's element is a whole file.
            if let Ok(v) = read(&good[..cut], "t.mat") {
                assert_eq!(v.len(), 1, "cut at {cut}");
            }
        }
        // A header and nothing after it.
        assert_eq!(
            err(&good[..HEADER]),
            "Unable to read MAT-file 't.mat': the file is truncated after its header."
        );
        assert!(err(&good[..HEADER + 4]).contains("truncated"));
        assert!(err(&good[..50]).contains("truncated"));
        // An array header that claims 1e6 x 1e6 elements.
        let mut huge = header();
        let mut body = Vec::new();
        put(
            &mut body,
            MI_UINT32,
            &[MX_DOUBLE as u8, 0, 0, 0, 0, 0, 0, 0],
        );
        let mut d = 1_000_000i32.to_le_bytes().to_vec();
        d.extend_from_slice(&1_000_000i32.to_le_bytes());
        put(&mut body, MI_INT32, &d);
        put(&mut body, MI_INT8, b"x");
        put(&mut body, MI_DOUBLE, &[0; 8]);
        huge.extend_from_slice(&MI_MATRIX.to_le_bytes());
        huge.extend_from_slice(&(body.len() as u32).to_le_bytes());
        huge.extend_from_slice(&body);
        assert!(
            err(&huge).contains("exceeds the maximum array size"),
            "{}",
            err(&huge)
        );
        // A cell that claims 2^28 elements holds none: no giant reservation.
        let mut cell = header();
        let mut body = Vec::new();
        put(&mut body, MI_UINT32, &[MX_CELL as u8, 0, 0, 0, 0, 0, 0, 0]);
        let mut d = 16384i32.to_le_bytes().to_vec();
        d.extend_from_slice(&16384i32.to_le_bytes());
        put(&mut body, MI_INT32, &d);
        put(&mut body, MI_INT8, b"c");
        cell.extend_from_slice(&MI_MATRIX.to_le_bytes());
        cell.extend_from_slice(&(body.len() as u32).to_le_bytes());
        cell.extend_from_slice(&body);
        assert!(err(&cell).contains("corrupt"), "{}", err(&cell));
        // A tag that claims four gigabytes.
        let mut lie = header();
        lie.extend_from_slice(&MI_MATRIX.to_le_bytes());
        lie.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(err(&lie).contains("truncated"));
        // Not a MAT file at all, and a compressed one.
        assert!(err(&[b'x'; 200]).contains("not a MAT-file"));
        let mut z = header();
        put(&mut z, MI_COMPRESSED, &[1, 2, 3]);
        assert!(err(&z).contains("compressed"));
    }

    /// Cycle 11's review: the bytes of a file bound nothing in a struct
    /// array with no fields, so 16384-by-16384 of one in 200 bytes took
    /// 6 GB and most of a minute to load. It is bounded by
    /// `MAX_FIELDLESS`, reading and writing.
    #[test]
    fn a_struct_array_with_no_fields_is_bounded_both_ways() {
        let fieldless = |rows: i32, cols: i32| {
            let mut body = Vec::new();
            put(
                &mut body,
                MI_UINT32,
                &[MX_STRUCT as u8, 0, 0, 0, 0, 0, 0, 0],
            );
            let mut d = rows.to_le_bytes().to_vec();
            d.extend_from_slice(&cols.to_le_bytes());
            put(&mut body, MI_INT32, &d);
            put(&mut body, MI_INT8, b"s");
            // The field-name length, a small element, and no names.
            body.extend_from_slice(&(MI_INT32 | (4 << 16)).to_le_bytes());
            body.extend_from_slice(&32i32.to_le_bytes());
            put(&mut body, MI_INT8, &[]);
            let mut f = header();
            f.extend_from_slice(&MI_MATRIX.to_le_bytes());
            f.extend_from_slice(&(body.len() as u32).to_le_bytes());
            f.extend_from_slice(&body);
            f
        };
        let t = std::time::Instant::now();
        let hostile = fieldless(16384, 16384);
        assert_eq!(hostile.len(), 200);
        assert_eq!(
            err(&hostile),
            "Unable to read MAT-file 't.mat': it holds a struct array with no fields and more than 1048576 elements."
        );
        assert!(t.elapsed().as_secs() < 1, "{:?}", t.elapsed());
        let past = MAX_FIELDLESS as i32 + 1;
        assert!(err(&fieldless(1, past)).contains("no fields"));
        let v = read(&fieldless(1, MAX_FIELDLESS as i32), "t.mat").unwrap();
        assert_eq!(v[0].1.dims(), (1, MAX_FIELDLESS));
        // `save` writes none that `load` would refuse, and the rest survive.
        let n = MAX_FIELDLESS + 1;
        let s = Value::strukt(StructArray::new(1, n, Vec::new(), vec![Vec::new(); n]));
        assert_eq!(
            write(&[("s".into(), s)]).unwrap_err().msg,
            "Unable to save variable 's': a struct array with no fields and more than 1048576 elements cannot be saved."
        );
        let s = Value::strukt(StructArray::new(2, 3, Vec::new(), vec![Vec::new(); 6]));
        assert_eq!(round_trip(vec![("s".into(), s)])[0].1.dims(), (2, 3));
    }

    /// A tag holds a length in 32 bits, so a variable past 4 GiB would be
    /// written with its length wrapped and the file silently corrupt. No
    /// test can build one; the check it goes through is this.
    #[test]
    fn a_length_past_four_gibibytes_is_refused_not_wrapped() {
        assert_eq!(len32(u32::MAX as usize, "x").unwrap(), u32::MAX);
        assert_eq!(
            len32(u32::MAX as usize + 1, "x").unwrap_err().msg,
            "Unable to save variable 'x': it is larger than the 4 GiB a MAT-file of version 5 can hold in one element."
        );
    }

    #[test]
    fn a_name_held_twice_is_one_variable_with_the_last_value() {
        let one = write(&[("v".into(), num(1.0))]).unwrap();
        let two = write(&[("v".into(), num(2.0)), ("w".into(), num(3.0))]).unwrap();
        let mut both = one.clone();
        both.extend_from_slice(&two[HEADER..]);
        let vars = read(&both, "t.mat").unwrap();
        let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["v", "w"]);
        assert_eq!(vars[0].1.mat().unwrap().data, vec![2.0]);
    }

    #[test]
    fn a_struct_of_many_fields_is_read_in_linear_time() {
        let n = 100_000;
        let fields: Vec<String> = (0..n).map(|k| format!("f{k}")).collect();
        let values: Vec<Value> = (0..n).map(|k| num(k as f64)).collect();
        let s = Value::strukt(StructArray::scalar(fields, values));
        let bytes = write(&[("s".into(), s)]).unwrap();
        let t = std::time::Instant::now();
        let vars = read(&bytes, "t.mat").unwrap();
        assert_eq!(vars.len(), 1);
        // Quadratic, the duplicate check took minutes in a debug build.
        assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
        // A field named twice is corrupt.
        let dup = Value::strukt(StructArray::scalar(
            vec!["a".into(), "a".into()],
            vec![num(1.0), num(2.0)],
        ));
        let bytes = write(&[("s".into(), dup)]).unwrap();
        assert!(err(&bytes).contains("corrupt"), "{}", err(&bytes));
    }

    #[test]
    fn nesting_past_the_bound_is_refused_both_ways() {
        let mut v = num(1.0);
        for _ in 0..MAX_DEPTH + 5 {
            v = Value::cell(CellArray::new(1, 1, vec![v]));
        }
        let e = write(&[("deep".into(), v)]).unwrap_err().msg;
        assert!(e.contains("nested too deeply"), "{e}");
        // A file nested past the bound, built by hand, is refused on read.
        let mut inner = Vec::new();
        matrix(&mut inner, "", &num(1.0), "x", 0).unwrap();
        for _ in 0..MAX_DEPTH + 5 {
            let mut body = Vec::new();
            put(&mut body, MI_UINT32, &[MX_CELL as u8, 0, 0, 0, 0, 0, 0, 0]);
            let mut d = 1i32.to_le_bytes().to_vec();
            d.extend_from_slice(&1i32.to_le_bytes());
            put(&mut body, MI_INT32, &d);
            put(&mut body, MI_INT8, b"");
            body.extend_from_slice(&inner);
            inner = MI_MATRIX.to_le_bytes().to_vec();
            inner.extend_from_slice(&(body.len() as u32).to_le_bytes());
            inner.extend_from_slice(&body);
        }
        // Name the outermost one.
        let mut file = header();
        file.extend_from_slice(&inner);
        let e = err(&file);
        assert!(
            e.contains("nested too deeply") || e.contains("valid name"),
            "{e}"
        );
    }

    #[test]
    fn numeric_data_is_read_under_any_numeric_type_and_either_byte_order() {
        // A 1x3 double stored as miUINT8, as MATLAB stores small integers.
        let mut f = header();
        let mut body = Vec::new();
        put(
            &mut body,
            MI_UINT32,
            &[MX_DOUBLE as u8, 0, 0, 0, 0, 0, 0, 0],
        );
        let mut d = 1i32.to_le_bytes().to_vec();
        d.extend_from_slice(&3i32.to_le_bytes());
        put(&mut body, MI_INT32, &d);
        // A small element for the name.
        body.extend_from_slice(&(MI_INT8 | (1 << 16)).to_le_bytes());
        body.extend_from_slice(b"v\0\0\0");
        put(&mut body, MI_UINT8, &[7, 8, 9]);
        f.extend_from_slice(&MI_MATRIX.to_le_bytes());
        f.extend_from_slice(&(body.len() as u32).to_le_bytes());
        f.extend_from_slice(&body);
        let v = read(&f, "t.mat").unwrap();
        assert_eq!(v[0].0, "v");
        assert_eq!(v[0].1.mat().unwrap().data, [7.0, 8.0, 9.0]);
        // The same file big-endian.
        let be = |w: u32| w.to_be_bytes();
        let mut g = header();
        g[124..126].copy_from_slice(&0x0100u16.to_be_bytes());
        g[126..128].copy_from_slice(b"MI");
        let mut body = Vec::new();
        let el = |body: &mut Vec<u8>, ty: u32, data: &[u8]| {
            body.extend_from_slice(&be(ty));
            body.extend_from_slice(&be(data.len() as u32));
            body.extend_from_slice(data);
            pad(body);
        };
        el(
            &mut body,
            MI_UINT32,
            &[0, 0, 0, MX_DOUBLE as u8, 0, 0, 0, 0],
        );
        let mut d = be(1).to_vec();
        d.extend_from_slice(&be(2));
        el(&mut body, MI_INT32, &d);
        el(&mut body, MI_INT8, b"w");
        let mut vals = 1.5f64.to_be_bytes().to_vec();
        vals.extend_from_slice(&(-2.0f64).to_be_bytes());
        el(&mut body, MI_DOUBLE, &vals);
        g.extend_from_slice(&be(MI_MATRIX));
        g.extend_from_slice(&be(body.len() as u32));
        g.extend_from_slice(&body);
        let v = read(&g, "t.mat").unwrap();
        assert_eq!(v[0].1.mat().unwrap().data, [1.5, -2.0]);
    }
}
