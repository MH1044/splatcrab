//! The regular-expression engine behind `regexp` and `regexprep` (cycle
//! 11): a Pike VM, which runs in time linear in the text it searches.
//!
//! A pattern is parsed into a [`Node`] tree, compiled into a small program
//! of [`Inst`]s, and run by simulating every thread of the program at once,
//! one text position at a time, never backtracking. Each program counter is
//! held by at most one thread per position, the highest-priority one, so a
//! position costs at most one step per instruction and a search is
//! `O(n * m)` for a text of `n` code units and a program of `m`
//! instructions, whatever the pattern: `(a*)*b` against 100,000 `a`s is one
//! pass. Threads are kept in priority order, which is what gives the
//! leftmost-first answers of MATLAB's (and Perl's) backtracking engines,
//! greedy and lazy quantifiers included.
//!
//! Finding every match is one pass too. A plain Pike VM finds them one
//! search at a time, and a pattern whose preferred branch runs far past
//! each match (`x*y|x` over a run of `x`s) makes every search rescan the
//! text: quadratic. Here the next search starts in the same pass, as soon
//! as the current one has a match, and a thread of a later search is
//! dropped where a thread of an earlier one already holds its program
//! counter: the two have the same future, and if the earlier one ever
//! matches, the later search is started again anyway. See [`Regex::run`].
//!
//! Backreferences (`\1`, `\k<name>`) cannot be matched in linear time and
//! are refused, as are lookaround, atomic groups, possessive quantifiers,
//! conditionals and inline flags. The Design notes of
//! `docs/modules/11-strings-and-io.md` list the syntax that is supported.
//!
//! The text is a sequence of UTF-16 code units, as a char array stores it,
//! so every position is a MATLAB index less one.
//!
//! A character class is built once, when it is parsed, into sorted,
//! disjoint ranges of code units with its negation already applied, and is
//! tested by binary search. Every copy a counted repetition makes of it,
//! and every class written again with the same text, shares that one
//! [`ClassSet`] by index, and each class's ranges count against
//! [`MAX_PROGRAM`] like instructions, so a class costs its ranges once
//! whatever repeats it.

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

use super::args::{MAX_ELEMS, check_shape};
use crate::error::{self, R, RegexFault, RegexUnsupported};

/// The largest program a pattern may compile to, counting each range of
/// each distinct class as one instruction. A counted repetition copies its
/// operand, so `(a{1000}){1000}` would otherwise be a program of a million
/// instructions from a pattern of fourteen characters.
/// A search costs up to one step per instruction per position, so this
/// also bounds the constant of the linear time.
pub const MAX_PROGRAM: usize = 20_000;

/// The deepest a pattern may nest groups and stacked quantifiers. The
/// parser and the compiler recurse once per level.
pub const MAX_NESTING: usize = 250;

/// The largest count a `{n,m}` quantifier may name.
pub const MAX_REPEAT: u32 = 1000;

/// The most capture groups a pattern may have. Every thread carries two
/// positions per group, and a search holds up to one thread per
/// instruction, so this and [`MAX_PROGRAM`] together bound a search's
/// memory, at about 32 MB for each of its two thread lists.
pub const MAX_GROUPS: usize = 100;

/// The most group positions, over every match, that a search hands back.
/// Each costs about forty bytes on the way out, so this is an eighth of the
/// element count an array may have, and about the same memory; past it the
/// search is refused with `check_shape`'s message for the table of matches
/// by groups.
pub const MAX_MATCH_ENTRIES: usize = MAX_ELEMS / 8;

/// One item of a character class, as written.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Range(u32, u32),
    Digit(bool),
    Word(bool),
    Space(bool),
}

/// A compiled character class.
#[derive(Clone, Debug, PartialEq)]
struct ClassSet {
    /// The code units the class matches, as sorted, disjoint, non-adjacent
    /// inclusive ranges: the complement of what was written when the class
    /// is negated.
    ranges: Vec<(u32, u32)>,
    /// Whether it was written negated. Only `'ignorecase'` reads it: a code
    /// unit then matches a negated class only when neither it nor its other
    /// case is one the class excludes, so `[^a]` rejects `A` as well.
    negated: bool,
}

/// What a zero-width assertion tests.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    /// `^`: the start of the text.
    Start,
    /// `$`: the end of the text.
    End,
    /// `\<`: the start of a word.
    WordStart,
    /// `\>`: the end of a word.
    WordEnd,
}

/// A parsed pattern.
#[derive(Clone, Debug, PartialEq)]
enum Node {
    Empty,
    Unit(u32),
    Any,
    /// The class `classes[i]` of the parser, shared by every copy.
    Class(usize),
    Look(Look),
    /// A group, capturing when it has an index.
    Group(Box<Node>, Option<usize>),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
        greedy: bool,
    },
}

/// One instruction of a compiled program.
#[derive(Clone, Debug)]
enum Inst {
    /// One code unit, exactly.
    Unit(u32),
    /// One code unit, compared case-folded; the operand is folded.
    Fold(u32),
    /// Any code unit.
    Any,
    /// A code unit in class `classes[i]`.
    Class(usize),
    /// Try `.0` first, then `.1`.
    Split(usize, usize),
    Jmp(usize),
    /// Record the position in capture slot `.0`.
    Save(usize),
    Look(Look),
    Match,
}

/// A compiled regular expression.
#[derive(Debug)]
pub struct Regex {
    prog: Vec<Inst>,
    classes: Vec<ClassSet>,
    icase: bool,
    /// Capture groups, the whole match included as group 0.
    groups: usize,
    /// Each named group and its index, in the order written.
    pub names: Vec<(String, usize)>,
}

/// One match: `(start, end)` of each group, zero-based and end-exclusive,
/// group 0 being the whole match and `None` a group that took no part.
pub type Groups = Vec<Option<(usize, usize)>>;

// ---- parsing ---------------------------------------------------------

struct Parser<'a> {
    p: &'a [u32],
    i: usize,
    depth: usize,
    groups: usize,
    names: Vec<(String, usize)>,
    /// Every distinct class, each built once.
    classes: Vec<ClassSet>,
    /// Each class's index by the text that wrote it, `[...]` or a `\w`
    /// outside one, so a class written again is looked up, not rebuilt.
    written: HashMap<Vec<u32>, usize>,
    /// The ranges of every class in `classes`, which count against
    /// [`MAX_PROGRAM`].
    ranges: usize,
}

fn syntax(fault: RegexFault) -> error::MError {
    error::regex_syntax(fault)
}

const fn u(c: char) -> u32 {
    c as u32
}

impl Parser<'_> {
    fn peek(&self) -> Option<u32> {
        self.p.get(self.i).copied()
    }

    fn peek_at(&self, k: usize) -> Option<u32> {
        self.p.get(self.i + k).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(u(c)) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// Counts one more capture group, against [`MAX_GROUPS`].
    fn new_group(&mut self) -> R<()> {
        self.groups += 1;
        if self.groups > MAX_GROUPS {
            return Err(error::regex_too_large());
        }
        Ok(())
    }

    fn deeper(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(error::regex_too_large());
        }
        Ok(())
    }

    fn alt(&mut self) -> R<Node> {
        let mut arms = vec![self.concat()?];
        while self.eat('|') {
            arms.push(self.concat()?);
        }
        Ok(if arms.len() == 1 {
            arms.pop().unwrap_or(Node::Empty)
        } else {
            Node::Alt(arms)
        })
    }

    fn concat(&mut self) -> R<Node> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == u('|') || c == u(')') {
                break;
            }
            items.push(self.repeat()?);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().unwrap_or(Node::Empty),
            _ => Node::Concat(items),
        })
    }

    /// An atom and the quantifiers after it. Stacked quantifiers, `a**`,
    /// each nest one level, so they count against [`MAX_NESTING`].
    fn repeat(&mut self) -> R<Node> {
        let saved = self.depth;
        let mut node = self.atom()?;
        loop {
            let (min, max) = match self.peek() {
                Some(c) if c == u('*') => {
                    self.i += 1;
                    (0, None)
                }
                Some(c) if c == u('+') => {
                    self.i += 1;
                    (1, None)
                }
                Some(c) if c == u('?') => {
                    self.i += 1;
                    (0, Some(1))
                }
                Some(c) if c == u('{') => match self.counted()? {
                    Some(q) => q,
                    None => break,
                },
                _ => break,
            };
            if matches!(node, Node::Look(_) | Node::Empty) {
                return Err(syntax(RegexFault::NothingToRepeat));
            }
            let greedy = if self.eat('?') {
                false
            } else if self.peek() == Some(u('+')) {
                return Err(error::regex_unsupported(RegexUnsupported::Possessive));
            } else {
                true
            };
            self.deeper()?;
            node = Node::Repeat {
                node: Box::new(node),
                min,
                max,
                greedy,
            };
        }
        self.depth = saved;
        Ok(node)
    }

    /// `{n}`, `{n,}` or `{n,m}` at the cursor, consumed; `None`, with the
    /// cursor left on the brace, when what follows is not one, in which case
    /// the brace is an ordinary character, as in Perl.
    fn counted(&mut self) -> R<Option<(u32, Option<u32>)>> {
        let start = self.i;
        self.i += 1;
        let number = |s: &mut Self| -> Option<u64> {
            let from = s.i;
            let mut v: u64 = 0;
            while let Some(d) = s.peek().filter(|c| (u('0')..=u('9')).contains(c)) {
                v = v.saturating_mul(10).saturating_add(u64::from(d - u('0')));
                s.i += 1;
            }
            (s.i > from).then_some(v)
        };
        let Some(min) = number(self) else {
            self.i = start;
            return Ok(None);
        };
        let max = if self.eat(',') {
            number(self)
        } else {
            Some(min)
        };
        if !self.eat('}') {
            self.i = start;
            return Ok(None);
        }
        let limit = u64::from(MAX_REPEAT);
        if min > limit || max.is_some_and(|m| m > limit) {
            return Err(error::regex_too_large());
        }
        if max.is_some_and(|m| m < min) {
            return Err(syntax(RegexFault::RepeatOrder));
        }
        Ok(Some((min as u32, max.map(|m| m as u32))))
    }

    fn atom(&mut self) -> R<Node> {
        let Some(c) = self.peek() else {
            return Ok(Node::Empty);
        };
        self.i += 1;
        Ok(match char::from_u32(c).unwrap_or('\u{FFFD}') {
            '(' => return self.group(),
            '[' => Node::Class(self.class()?),
            '.' => Node::Any,
            '^' => Node::Look(Look::Start),
            '$' => Node::Look(Look::End),
            '\\' => self.escape()?,
            '*' | '+' | '?' => return Err(syntax(RegexFault::NothingToRepeat)),
            _ => Node::Unit(c),
        })
    }

    fn group(&mut self) -> R<Node> {
        self.deeper()?;
        let mut index = None;
        if self.eat('?') {
            match self.peek().and_then(char::from_u32) {
                Some(':') => self.i += 1,
                Some('#') => {
                    // A comment runs to the next `)`.
                    while let Some(c) = self.peek() {
                        self.i += 1;
                        if c == u(')') {
                            self.depth -= 1;
                            return Ok(Node::Empty);
                        }
                    }
                    return Err(syntax(RegexFault::MissingParen));
                }
                Some('=') | Some('!') => return Err(error::regex_lookaround()),
                Some('<') if matches!(self.peek_at(1), Some(c) if c == u('=') || c == u('!')) => {
                    return Err(error::regex_lookaround());
                }
                Some('<') => {
                    self.i += 1;
                    let name = self.group_name()?;
                    self.new_group()?;
                    index = Some(self.groups);
                    self.names.push((name, self.groups));
                }
                Some('>') => return Err(error::regex_unsupported(RegexUnsupported::Atomic)),
                Some('(') => return Err(error::regex_unsupported(RegexUnsupported::Conditional)),
                _ => return Err(error::regex_unsupported(RegexUnsupported::InlineFlag)),
            }
        } else {
            self.new_group()?;
            index = Some(self.groups);
        }
        let inner = self.alt()?;
        if !self.eat(')') {
            return Err(syntax(RegexFault::MissingParen));
        }
        self.depth -= 1;
        Ok(Node::Group(Box::new(inner), index))
    }

    /// The name of `(?<name>...)`, up to its `>`: a letter, then letters,
    /// digits and underscores, as a struct field needs.
    fn group_name(&mut self) -> R<String> {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == u('>') {
                let ok = name.chars().next().is_some_and(|f| f.is_ascii_alphabetic())
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if !ok {
                    return Err(syntax(RegexFault::GroupName));
                }
                return Ok(name);
            }
            name.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
        }
        Err(syntax(RegexFault::GroupNameEnd))
    }

    /// The character after a backslash, outside a class.
    fn escape(&mut self) -> R<Node> {
        let Some(c) = self.peek() else {
            return Err(syntax(RegexFault::TrailingBackslash));
        };
        self.i += 1;
        let item = match char::from_u32(c) {
            Some('d') => Some(Item::Digit(true)),
            Some('D') => Some(Item::Digit(false)),
            Some('w') => Some(Item::Word(true)),
            Some('W') => Some(Item::Word(false)),
            Some('s') => Some(Item::Space(true)),
            Some('S') => Some(Item::Space(false)),
            _ => None,
        };
        if let Some(item) = item {
            let key = vec![u('\\'), c];
            return Ok(Node::Class(self.intern(key, &[item], false)?));
        }
        Ok(match char::from_u32(c).unwrap_or('\u{FFFD}') {
            '<' => Node::Look(Look::WordStart),
            '>' => Node::Look(Look::WordEnd),
            '1'..='9' => return Err(error::regex_backreference()),
            'k' if self.peek() == Some(u('<')) => return Err(error::regex_backreference()),
            _ => {
                self.i -= 1;
                Node::Unit(self.unit_escape()?)
            }
        })
    }

    /// An escape that stands for one code unit, the cursor on the character
    /// after the backslash: `\n`, `\t`, `\xN`, `\oN`, or any other character
    /// taken literally, as MATLAB's `\char` is.
    fn unit_escape(&mut self) -> R<u32> {
        let Some(c) = self.peek() else {
            return Err(syntax(RegexFault::TrailingBackslash));
        };
        self.i += 1;
        Ok(match char::from_u32(c).unwrap_or('\u{FFFD}') {
            'n' => 10,
            't' => 9,
            'r' => 13,
            'f' => 12,
            'v' => 11,
            'a' => 7,
            'b' => 8,
            'e' => 27,
            '0' => 0,
            'x' => self.coded(16)?,
            'o' => self.coded(8)?,
            _ => c,
        })
    }

    /// The digits of `\xN`, `\x{N}`, `\oN` or `\o{N}` in `radix`.
    fn coded(&mut self, radix: u32) -> R<u32> {
        let braced = self.eat('{');
        let mut v: u32 = 0;
        let mut n = 0;
        while let Some(d) = self
            .peek()
            .and_then(char::from_u32)
            .and_then(|c| c.to_digit(radix))
        {
            v = v.saturating_mul(radix).saturating_add(d);
            self.i += 1;
            n += 1;
        }
        if n == 0 || (braced && !self.eat('}')) {
            return Err(syntax(RegexFault::BadCode));
        }
        if v > 0xFFFF {
            return Err(syntax(RegexFault::CodeTooLarge));
        }
        Ok(v)
    }

    /// A class, the cursor after its `[`: the index of its [`ClassSet`].
    fn class(&mut self) -> R<usize> {
        let start = self.i - 1;
        let negated = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return Err(syntax(RegexFault::MissingBracket));
            };
            if c == u(']') && !first {
                self.i += 1;
                break;
            }
            first = false;
            self.i += 1;
            let lo = if c == u('\\') {
                match self.peek().and_then(char::from_u32) {
                    Some(k @ ('d' | 'D' | 'w' | 'W' | 's' | 'S')) => {
                        self.i += 1;
                        items.push(match k {
                            'd' => Item::Digit(true),
                            'D' => Item::Digit(false),
                            'w' => Item::Word(true),
                            'W' => Item::Word(false),
                            's' => Item::Space(true),
                            _ => Item::Space(false),
                        });
                        continue;
                    }
                    _ => self.unit_escape()?,
                }
            } else {
                c
            };
            // A range, unless the `-` is last in the class.
            if self.peek() == Some(u('-')) && self.peek_at(1).is_some_and(|n| n != u(']')) {
                self.i += 1;
                let Some(h) = self.peek() else {
                    return Err(syntax(RegexFault::MissingBracket));
                };
                self.i += 1;
                let hi = if h == u('\\') { self.unit_escape()? } else { h };
                if hi < lo {
                    return Err(syntax(RegexFault::RangeOrder));
                }
                items.push(Item::Range(lo, hi));
            } else {
                items.push(Item::Range(lo, lo));
            }
        }
        let key = self.p[start..self.i].to_vec();
        self.intern(key, &items, negated)
    }

    /// The index of the class written as `key`, built from `items` the
    /// first time that text is seen. Its ranges count against
    /// [`MAX_PROGRAM`], at least one for a class that matches nothing.
    fn intern(&mut self, key: Vec<u32>, items: &[Item], negated: bool) -> R<usize> {
        if let Some(&k) = self.written.get(&key) {
            return Ok(k);
        }
        let set = ClassSet::new(items, negated);
        self.ranges = self.ranges.saturating_add(set.ranges.len().max(1));
        if self.ranges > MAX_PROGRAM {
            return Err(error::regex_too_large());
        }
        self.classes.push(set);
        self.written.insert(key, self.classes.len() - 1);
        Ok(self.classes.len() - 1)
    }
}

// ---- classes and case --------------------------------------------------

fn is_word(c: u32) -> bool {
    c == u('_') || char::from_u32(c).is_some_and(char::is_alphanumeric)
}

pub(crate) fn fold(c: u32) -> u32 {
    if c < 128 {
        return u32::from((c as u8).to_ascii_lowercase());
    }
    single_case(c, char::to_lowercase)
}

pub(crate) fn upper(c: u32) -> u32 {
    if c < 128 {
        return u32::from((c as u8).to_ascii_uppercase());
    }
    single_case(c, char::to_uppercase)
}

/// `c` mapped by `f` when the mapping is one character in the BMP, and `c`
/// itself otherwise.
fn single_case<I: Iterator<Item = char>>(c: u32, f: impl Fn(char) -> I) -> u32 {
    let Some(ch) = char::from_u32(c) else {
        return c;
    };
    let mut it = f(ch);
    match (it.next(), it.next()) {
        (Some(m), None) if (m as u32) <= 0xFFFF => m as u32,
        _ => c,
    }
}

/// The code units of `\w`, as ranges: a letter or digit of any script, or
/// `_`. Built once, over the BMP, which is every code unit a char holds.
fn word_ranges() -> &'static [(u32, u32)] {
    static WORD: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    WORD.get_or_init(|| {
        let mut out: Vec<(u32, u32)> = Vec::new();
        for c in 0..=0xFFFF {
            if !is_word(c) {
                continue;
            }
            match out.last_mut() {
                Some(last) if last.1 + 1 == c => last.1 = c,
                _ => out.push((c, c)),
            }
        }
        out
    })
}

/// `ranges` sorted, with overlapping and adjacent ranges merged.
fn normalise(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    ranges.sort_unstable();
    let mut out: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
    for (lo, hi) in ranges {
        match out.last_mut() {
            Some(last) if lo <= last.1.saturating_add(1) => last.1 = last.1.max(hi),
            _ => out.push((lo, hi)),
        }
    }
    out
}

/// Every code unit that normalised `ranges` leave out, as ranges.
fn complement(ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(ranges.len() + 1);
    let mut next = 0;
    for &(lo, hi) in ranges {
        if lo > next {
            out.push((next, lo - 1));
        }
        if hi == u32::MAX {
            return out;
        }
        next = hi + 1;
    }
    out.push((next, u32::MAX));
    out
}

impl ClassSet {
    /// The class `items` write, negated when `negated` is set. Each of
    /// `\d \w \s` and their negations adds its ranges once however often
    /// it is written, so the work is the items' count plus a constant.
    fn new(items: &[Item], negated: bool) -> ClassSet {
        let mut ranges = Vec::new();
        // Digit, word and space, each positive and negative.
        let mut shorthand = [false; 6];
        for item in items {
            let (k, yes) = match *item {
                Item::Range(lo, hi) => {
                    ranges.push((lo, hi));
                    continue;
                }
                Item::Digit(yes) => (0, yes),
                Item::Word(yes) => (2, yes),
                Item::Space(yes) => (4, yes),
            };
            shorthand[k + usize::from(!yes)] = true;
        }
        let digit = [(u('0'), u('9'))];
        // MATLAB's `\s`: `[ \f\n\r\t\v]`.
        let space = [(9, 13), (32, 32)];
        for k in 0..3 {
            if !shorthand[2 * k] && !shorthand[2 * k + 1] {
                continue;
            }
            let set: &[(u32, u32)] = match k {
                0 => &digit,
                1 => word_ranges(),
                _ => &space,
            };
            if shorthand[2 * k] {
                ranges.extend_from_slice(set);
            }
            if shorthand[2 * k + 1] {
                ranges.extend(complement(set));
            }
        }
        let ranges = normalise(ranges);
        ClassSet {
            ranges: if negated { complement(&ranges) } else { ranges },
            negated,
        }
    }

    /// Whether `c` is in [`ClassSet::ranges`], by binary search.
    fn holds(&self, c: u32) -> bool {
        let k = self.ranges.partition_point(|&(_, hi)| hi < c);
        self.ranges.get(k).is_some_and(|&(lo, _)| lo <= c)
    }

    fn matches(&self, c: u32, icase: bool) -> bool {
        if !icase {
            self.holds(c)
        } else if self.negated {
            // Not one of the written units, in either case.
            self.holds(c) && self.holds(fold(c)) && self.holds(upper(c))
        } else {
            self.holds(c) || self.holds(fold(c)) || self.holds(upper(c))
        }
    }
}

// ---- compiling -----------------------------------------------------------

struct Compiler {
    prog: Vec<Inst>,
    icase: bool,
    /// The classes' ranges, which count against [`MAX_PROGRAM`] with the
    /// instructions.
    ranges: usize,
}

impl Compiler {
    fn push(&mut self, i: Inst) -> R<usize> {
        if self.prog.len() + self.ranges >= MAX_PROGRAM {
            return Err(error::regex_too_large());
        }
        self.prog.push(i);
        Ok(self.prog.len() - 1)
    }

    fn patch(&mut self, at: usize, to: usize) {
        match &mut self.prog[at] {
            Inst::Jmp(x) => *x = to,
            Inst::Split(_, y) => *y = to,
            _ => {}
        }
    }

    fn node(&mut self, n: &Node) -> R<()> {
        match n {
            Node::Empty => {}
            Node::Unit(c) if self.icase && fold(*c) != upper(*c) => {
                self.push(Inst::Fold(fold(*c)))?;
            }
            Node::Unit(c) => {
                self.push(Inst::Unit(*c))?;
            }
            Node::Any => {
                self.push(Inst::Any)?;
            }
            Node::Class(k) => {
                self.push(Inst::Class(*k))?;
            }
            Node::Look(l) => {
                self.push(Inst::Look(*l))?;
            }
            Node::Group(inner, index) => match index {
                Some(k) => {
                    self.push(Inst::Save(2 * k))?;
                    self.node(inner)?;
                    self.push(Inst::Save(2 * k + 1))?;
                }
                None => self.node(inner)?,
            },
            Node::Concat(items) => {
                for item in items {
                    self.node(item)?;
                }
            }
            Node::Alt(arms) => {
                let mut exits = Vec::new();
                for (k, arm) in arms.iter().enumerate() {
                    if k + 1 < arms.len() {
                        let split = self.push(Inst::Split(0, 0))?;
                        let body = self.prog.len();
                        if let Inst::Split(x, _) = &mut self.prog[split] {
                            *x = body;
                        }
                        self.node(arm)?;
                        exits.push(self.push(Inst::Jmp(0))?);
                        let next = self.prog.len();
                        self.patch(split, next);
                    } else {
                        self.node(arm)?;
                    }
                }
                let end = self.prog.len();
                for e in exits {
                    self.patch(e, end);
                }
            }
            Node::Repeat {
                node,
                min,
                max,
                greedy,
            } => {
                for _ in 0..*min {
                    let before = self.prog.len();
                    self.node(node)?;
                    // A repetition of nothing is nothing, however many
                    // times: `(?:){1000}{1000}{1000}` must not spin.
                    if self.prog.len() == before {
                        break;
                    }
                }
                match max {
                    None => self.star(node, *greedy)?,
                    Some(max) => {
                        for _ in *min..*max {
                            self.optional(node, *greedy)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// A split that prefers its body when greedy and skipping it otherwise;
    /// returns the split's address. Its exit is patched by the caller.
    fn split(&mut self, greedy: bool) -> R<usize> {
        let at = self.push(Inst::Split(0, 0))?;
        let body = at + 1;
        self.prog[at] = if greedy {
            Inst::Split(body, 0)
        } else {
            Inst::Split(0, body)
        };
        Ok(at)
    }

    fn set_exit(&mut self, split: usize, greedy: bool, exit: usize) {
        if let Inst::Split(x, y) = &mut self.prog[split] {
            if greedy {
                *y = exit;
            } else {
                *x = exit;
            }
        }
    }

    fn star(&mut self, n: &Node, greedy: bool) -> R<()> {
        let split = self.split(greedy)?;
        self.node(n)?;
        self.push(Inst::Jmp(split))?;
        let exit = self.prog.len();
        self.set_exit(split, greedy, exit);
        Ok(())
    }

    fn optional(&mut self, n: &Node, greedy: bool) -> R<()> {
        let split = self.split(greedy)?;
        self.node(n)?;
        let exit = self.prog.len();
        self.set_exit(split, greedy, exit);
        Ok(())
    }
}

// ---- running ---------------------------------------------------------------

/// A slot that has recorded no position.
const NONE: usize = usize::MAX;

struct Thread {
    pc: usize,
    /// The search this thread belongs to; see [`Regex::run`].
    search: u64,
    /// Capture positions, two per group; empty for an instruction that
    /// consumes nothing, which the step loop never runs.
    slots: Box<[usize]>,
}

/// The threads at one position, one per program counter at most, in
/// priority order: a sparse set, so that clearing it and cutting its tail
/// are both cheap.
struct List {
    dense: Vec<Thread>,
    sparse: Vec<usize>,
}

impl List {
    fn new(n: usize) -> List {
        List {
            dense: Vec::new(),
            sparse: vec![0; n],
        }
    }

    fn holds(&self, pc: usize) -> bool {
        let i = self.sparse[pc];
        i < self.dense.len() && self.dense[i].pc == pc
    }

    fn push(&mut self, t: Thread) {
        self.sparse[t.pc] = self.dense.len();
        self.dense.push(t);
    }
}

/// One search of [`Regex::run`]: the match it has found so far, and the
/// position at which an empty match is not allowed.
struct Search {
    id: u64,
    found: Option<Box<[usize]>>,
    no_empty_at: Option<usize>,
}

impl Regex {
    /// Compiles `pattern`, a sequence of UTF-16 code units, matching
    /// without regard to case when `icase` is set.
    pub fn new(pattern: &[u32], icase: bool) -> R<Regex> {
        let mut p = Parser {
            p: pattern,
            i: 0,
            depth: 0,
            groups: 0,
            names: Vec::new(),
            classes: Vec::new(),
            written: HashMap::new(),
            ranges: 0,
        };
        let tree = p.alt()?;
        if p.i < pattern.len() {
            // Only an unmatched `)` stops the top-level alternation early.
            return Err(syntax(RegexFault::UnmatchedParen));
        }
        let mut c = Compiler {
            prog: Vec::new(),
            icase,
            ranges: p.ranges,
        };
        c.push(Inst::Save(0))?;
        c.node(&tree)?;
        c.push(Inst::Save(1))?;
        c.push(Inst::Match)?;
        Ok(Regex {
            prog: c.prog,
            classes: p.classes,
            icase,
            groups: p.groups + 1,
            names: p.names,
        })
    }

    /// How many capture groups the pattern has, not counting the whole
    /// match.
    pub fn captures(&self) -> usize {
        self.groups - 1
    }

    /// Every match in `text`, leftmost-first and not overlapping, at most
    /// `limit` of them. An empty match is skipped unless `empty` is set; if
    /// it is, an empty match right where an empty match ended is not taken
    /// again, which is Perl's rule.
    ///
    /// The matches are refused, as an array too large, when they would hold
    /// more positions than an array may: one match per code unit of a long
    /// subject, each with a hundred groups, is billions.
    pub fn find(&self, text: &[u32], empty: bool, limit: usize) -> R<Vec<Groups>> {
        Ok(self
            .run(text, empty, limit)?
            .into_iter()
            .map(|slots| {
                (0..self.groups)
                    .map(|g| {
                        let (s, e) = (slots[2 * g], slots[2 * g + 1]);
                        (s != NONE && e != NONE).then_some((s, e))
                    })
                    .collect()
            })
            .collect())
    }

    /// Refuses a list of `found` matches too large to hand back: more than
    /// [`MAX_MATCH_ENTRIES`] groups in all, the whole match counted as one.
    fn check_count(&self, found: usize) -> R<()> {
        if found.saturating_mul(self.groups) > MAX_MATCH_ENTRIES {
            check_shape(found as f64, self.groups as f64)?;
        }
        Ok(())
    }

    /// True for an instruction that consumes nothing and is not `Match`:
    /// the steps of a closure.
    fn epsilon(&self, pc: usize) -> bool {
        matches!(
            self.prog[pc],
            Inst::Jmp(_) | Inst::Split(..) | Inst::Save(_) | Inst::Look(_)
        )
    }

    /// Walks everything reachable from `pc` at position `at` without
    /// consuming input, in priority order. `f` is told of each instruction
    /// reached, `Step::Visit`, and answers whether it is new; of each new
    /// instruction that consumes or matches, `Step::Leaf`, with its capture
    /// slots, and answers whether to go on. Iterative, since a program can
    /// be a long run of jumps.
    fn closure(
        &self,
        stack: &mut Vec<(usize, Box<[usize]>)>,
        pc: usize,
        slots: Box<[usize]>,
        at: usize,
        text: &[u32],
        f: &mut dyn FnMut(Step) -> bool,
    ) {
        stack.clear();
        stack.push((pc, slots));
        while let Some((pc, mut slots)) = stack.pop() {
            if !f(Step::Visit(pc)) {
                continue;
            }
            match self.prog[pc] {
                Inst::Jmp(x) => stack.push((x, slots)),
                Inst::Split(x, y) => {
                    stack.push((y, slots.clone()));
                    stack.push((x, slots));
                }
                Inst::Save(k) => {
                    slots[k] = at;
                    stack.push((pc + 1, slots));
                }
                Inst::Look(l) => {
                    let before = at.checked_sub(1).map(|k| text[k]);
                    let after = text.get(at).copied();
                    let ok = match l {
                        Look::Start => at == 0,
                        Look::End => at == text.len(),
                        Look::WordStart => {
                            !before.is_some_and(is_word) && after.is_some_and(is_word)
                        }
                        Look::WordEnd => before.is_some_and(is_word) && !after.is_some_and(is_word),
                    };
                    if ok {
                        stack.push((pc + 1, slots));
                    }
                }
                _ => {
                    if !f(Step::Leaf(pc, slots)) {
                        stack.clear();
                        return;
                    }
                }
            }
        }
    }

    /// Adds the closure of `pc` at `at` to `list` for search `search`,
    /// skipping every instruction some thread already holds there.
    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        list: &mut List,
        stack: &mut Vec<(usize, Box<[usize]>)>,
        pc: usize,
        slots: Box<[usize]>,
        at: usize,
        text: &[u32],
        search: u64,
    ) {
        self.closure(stack, pc, slots, at, text, &mut |step| match step {
            Step::Visit(pc) => {
                if list.holds(pc) {
                    return false;
                }
                if self.epsilon(pc) {
                    list.push(Thread {
                        pc,
                        search,
                        slots: Box::new([]),
                    });
                }
                true
            }
            Step::Leaf(pc, slots) => {
                list.push(Thread { pc, search, slots });
                true
            }
        });
    }

    fn consumes(&self, pc: usize, c: u32) -> bool {
        match self.prog[pc] {
            Inst::Unit(x) => c == x,
            Inst::Fold(x) => fold(c) == x,
            Inst::Any => true,
            Inst::Class(k) => self.classes[k].matches(c, self.icase),
            _ => false,
        }
    }

    /// The matches as raw slots, in one pass over the text.
    ///
    /// The pass runs a chain of searches. The first starts at position 0;
    /// each later one starts where the one before it last found a match, as
    /// a fresh search after that match would. Only the last search in the
    /// chain is still looking for its first match, so only it is seeded
    /// with a new thread at each position, and its threads come after all
    /// the others in priority. A search that finds a match cuts its own
    /// lower-priority threads, as a Pike VM does, and every search after
    /// it, whose start that match has just moved; the next search then
    /// starts afresh there. A search whose threads have all died is final
    /// once every search before it is, and its match is the next answer.
    ///
    /// A thread that is about to consume is dropped where a thread of an
    /// earlier search already holds its program counter at the same
    /// position. That is safe, because from there the two have the same
    /// future: every match either can reach lies past this position, so
    /// neither is empty, and if the earlier thread dies the later one would
    /// have died too, while if it matches, the later search is cut and
    /// started again. It is also what makes the pass linear, because it
    /// keeps the whole chain to one thread per program counter per
    /// position. The closure of a search started in the middle of a step
    /// is walked on its own, since the instructions an earlier search
    /// passed through on its way to the match just taken lead the new
    /// search to a match of its own.
    fn run(&self, text: &[u32], empty: bool, limit: usize) -> R<Vec<Box<[usize]>>> {
        let n = self.prog.len();
        let width = 2 * self.groups;
        let fresh = || vec![NONE; width].into_boxed_slice();
        let mut clist = List::new(n);
        let mut nlist = List::new(n);
        let mut stack = Vec::new();
        // The closure of a search started mid-step marks what it has
        // walked with a stamp of its own.
        let mut seen = vec![0u64; n];
        let mut stamp = 0u64;
        let mut chain: VecDeque<Search> = VecDeque::new();
        chain.push_back(Search {
            id: 0,
            found: None,
            no_empty_at: None,
        });
        let mut next_id = 1;
        let mut out: Vec<Box<[usize]>> = Vec::new();
        if limit == 0 {
            return Ok(out);
        }
        for at in 0..=text.len() {
            let last = chain.back().map_or(0, |s| s.id);
            self.add(&mut clist, &mut stack, 0, fresh(), at, text, last);
            let mut i = 0;
            while i < clist.dense.len() {
                let pc = clist.dense[i].pc;
                let id = clist.dense[i].search;
                match self.prog[pc] {
                    Inst::Match => {
                        // Ids rise along the chain.
                        let Ok(k) = chain.binary_search_by_key(&id, |s| s.id) else {
                            i += 1;
                            continue;
                        };
                        let slots = std::mem::take(&mut clist.dense[i].slots);
                        if accepts(&chain[k], &slots, at, empty) {
                            // Lower-priority threads of this search, and
                            // every later search, are cut.
                            clist.dense.truncate(i + 1);
                            chain.truncate(k + 1);
                            let was_empty = slots[0] == at;
                            chain[k].found = Some(slots);
                            // The next search starts here, and so does the
                            // one after it if that one matches empty here.
                            let mut no_empty_at = was_empty.then_some(at);
                            loop {
                                let id = next_id;
                                next_id += 1;
                                let mut search = Search {
                                    id,
                                    found: None,
                                    no_empty_at,
                                };
                                stamp += 1;
                                let mut taken = None;
                                self.closure(&mut stack, 0, fresh(), at, text, &mut |step| {
                                    match step {
                                        Step::Visit(pc) if self.epsilon(pc) => {
                                            let new = seen[pc] != stamp;
                                            seen[pc] = stamp;
                                            new
                                        }
                                        Step::Visit(pc) if matches!(self.prog[pc], Inst::Match) => {
                                            seen[pc] != stamp
                                        }
                                        Step::Visit(pc) => !clist.holds(pc),
                                        Step::Leaf(pc, slots) => {
                                            seen[pc] = stamp;
                                            if !matches!(self.prog[pc], Inst::Match) {
                                                clist.push(Thread {
                                                    pc,
                                                    search: id,
                                                    slots,
                                                });
                                                true
                                            } else if accepts(&search, &slots, at, empty) {
                                                taken = Some(slots);
                                                false
                                            } else {
                                                true
                                            }
                                        }
                                    }
                                });
                                match taken {
                                    Some(slots) => {
                                        // It matched empty here, and is
                                        // final as soon as nothing before
                                        // it can still move.
                                        search.found = Some(slots);
                                        chain.push_back(search);
                                        no_empty_at = Some(at);
                                    }
                                    None => {
                                        chain.push_back(search);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ if at < text.len() && self.consumes(pc, text[at]) => {
                        let slots = std::mem::take(&mut clist.dense[i].slots);
                        self.add(&mut nlist, &mut stack, pc + 1, slots, at + 1, text, id);
                    }
                    _ => {}
                }
                i += 1;
            }
            // A search with no threads left, and every search before it
            // final, is final.
            while chain.len() > 1 {
                let front = chain.front().map_or(0, |s| s.id);
                if nlist.dense.first().is_some_and(|t| t.search == front) {
                    break;
                }
                if let Some(Search { found: Some(s), .. }) = chain.pop_front() {
                    out.push(s);
                    self.check_count(out.len())?;
                    if out.len() >= limit {
                        return Ok(out);
                    }
                }
            }
            std::mem::swap(&mut clist, &mut nlist);
            nlist.dense.clear();
        }
        while let Some(s) = chain.pop_front() {
            if let Some(found) = s.found {
                out.push(found);
                self.check_count(out.len())?;
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }
}

/// What [`Regex::closure`] reports.
enum Step {
    Visit(usize),
    Leaf(usize, Box<[usize]>),
}

/// Whether `search` takes the match `slots` ending at `at`: a non-empty one
/// always; an empty one only when empty matches are wanted, and not where
/// the search is barred from one.
fn accepts(search: &Search, slots: &[usize], at: usize, empty: bool) -> bool {
    slots[0] != at || (empty && search.no_empty_at != Some(at))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> Vec<u32> {
        s.encode_utf16().map(u32::from).collect()
    }

    fn re(p: &str) -> Regex {
        Regex::new(&units(p), false).unwrap()
    }

    /// Every match of `p` in `t` as text.
    fn all(p: &str, t: &str) -> Vec<String> {
        let text = units(t);
        re(p)
            .find(&text, false, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|g| {
                let (s, e) = g[0].unwrap();
                t.chars().skip(s).take(e - s).collect()
            })
            .collect()
    }

    fn spans(p: &str, t: &str, empty: bool) -> Vec<(usize, usize)> {
        re(p)
            .find(&units(t), empty, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|g| g[0].unwrap())
            .collect()
    }

    fn err(p: &str) -> String {
        Regex::new(&units(p), false).unwrap_err().msg
    }

    #[test]
    fn literals_classes_and_escapes() {
        assert_eq!(all("bc", "abcabc"), ["bc", "bc"]);
        assert_eq!(all("\\d+", "ab12c345"), ["12", "345"]);
        assert_eq!(all("[a-c]+", "xxabcaxbx"), ["abca", "b"]);
        assert_eq!(all("[^a-c ]+", "ab xyz c"), ["xyz"]);
        assert_eq!(all("\\w+", "hi, you_2!"), ["hi", "you_2"]);
        assert_eq!(all("\\s", "a b\tc"), [" ", "\t"]);
        assert_eq!(all("\\S+", " a bc "), ["a", "bc"]);
        assert_eq!(all("\\D+", "a1b"), ["a", "b"]);
        assert_eq!(all("[\\d.]+", "x3.14y"), ["3.14"]);
        assert_eq!(all("a.c", "abc a\nc"), ["abc", "a\nc"]);
        assert_eq!(all("\\.", "a.b"), ["."]);
        assert_eq!(all("\\x41\\o102", "zAB"), ["AB"]);
        assert_eq!(all("[]a]+", "x]a]"), ["]a]"]);
        assert_eq!(all("[a-]+", "b-a-"), ["-a-"]);
        assert_eq!(all("a{", "a{"), ["a{"]);
    }

    #[test]
    fn anchors_alternation_and_word_edges() {
        assert_eq!(all("^a", "aaa"), ["a"]);
        assert_eq!(all("a$", "aaa"), ["a"]);
        assert_eq!(all("cat|dog", "dog cat"), ["dog", "cat"]);
        assert_eq!(all("\\<\\w", "one two"), ["o", "t"]);
        assert_eq!(all("\\w\\>", "one two"), ["e", "o"]);
        assert_eq!(all("x(ab|cd)y", "xcdy xaby"), ["xcdy", "xaby"]);
        assert_eq!(all("a|ab", "ab"), ["a"]);
        assert_eq!(all("ab|a", "ab"), ["ab"]);
    }

    #[test]
    fn greedy_and_lazy_quantifiers_give_leftmost_first_answers() {
        assert_eq!(all("<.*>", "<a><b>"), ["<a><b>"]);
        assert_eq!(all("<.*?>", "<a><b>"), ["<a>", "<b>"]);
        assert_eq!(all("a+?", "aaa"), ["a", "a", "a"]);
        assert_eq!(all("a??b", "ab b"), ["ab", "b"]);
        assert_eq!(all("a{2}", "aaaaa"), ["aa", "aa"]);
        assert_eq!(all("a{2,3}", "aaaaaaa"), ["aaa", "aaa"]);
        assert_eq!(all("a{2,3}?", "aaaaa"), ["aa", "aa"]);
        assert_eq!(all("a{2,}", "a aa aaaa"), ["aa", "aaaa"]);
        assert_eq!(all("(ab)+", "ababxab"), ["abab", "ab"]);
    }

    #[test]
    fn captures_and_names() {
        let r = re("(\\w+)=(?<val>\\d+)(x)?");
        assert_eq!(r.captures(), 3);
        assert_eq!(r.names, [("val".to_string(), 2)]);
        let g = r.find(&units("k=12"), false, usize::MAX).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0], [Some((0, 4)), Some((0, 1)), Some((2, 4)), None]);
        // A group inside a repetition keeps its last iteration.
        let g = re("(a|b)+").find(&units("abba"), false, 1).unwrap();
        assert_eq!(g[0][1], Some((3, 4)));
        // (?:...) does not capture.
        assert_eq!(re("(?:a)(b)").captures(), 1);
    }

    #[test]
    fn empty_matches_are_skipped_unless_asked_for() {
        assert_eq!(spans("x*", "abc", false), []);
        assert_eq!(spans("x*", "abc", true), [(0, 0), (1, 1), (2, 2), (3, 3)]);
        // An empty match is allowed right after a non-empty one, and not
        // again right after an empty one: Perl's rule.
        assert_eq!(spans("b*", "abc", true), [(0, 0), (1, 2), (2, 2), (3, 3)]);
        assert_eq!(spans("a*?", "aa", false), [(0, 1), (1, 2)]);
        assert_eq!(all("a*", "baaca"), ["aa", "a"]);
    }

    #[test]
    fn case_can_be_ignored() {
        let r = Regex::new(&units("ab[c-d]"), true).unwrap();
        assert_eq!(r.find(&units("xABDabc"), false, 9).unwrap().len(), 2);
        let r = Regex::new(&units("é"), true).unwrap();
        assert_eq!(r.find(&units("É"), false, 9).unwrap().len(), 1);
    }

    #[test]
    fn unsupported_syntax_is_refused_cleanly() {
        let back = "Backreferences are not supported in regular expressions.";
        assert_eq!(err("(a)\\1"), back);
        assert_eq!(err("(?<x>a)\\k<x>"), back);
        let look = "Lookahead and lookbehind are not supported in regular expressions.";
        for p in ["a(?=b)", "a(?!b)", "(?<=a)b", "(?<!a)b"] {
            assert_eq!(err(p), look, "{p}");
        }
        assert!(err("a*+").contains("possessive"));
        assert!(err("(?>a)").contains("atomic"));
        assert!(err("(?i)a").contains("inline flag"));
        for p in [
            "(a", "a)", "[ab", "*a", "a|*", "\\", "[b-a]", "a{3,2}", "(?<1x>a)", "\\x",
        ] {
            assert!(
                err(p).starts_with("Invalid regular expression: "),
                "{p}: {}",
                err(p)
            );
        }
    }

    #[test]
    fn a_pattern_too_large_or_too_deep_is_refused() {
        let big = "The regular expression is too large.";
        assert_eq!(err("(a{1000}){1000}"), big);
        assert_eq!(err("a{1001}"), big);
        assert_eq!(err(&"(".repeat(100_000)), big);
        assert_eq!(err(&format!("a{}", "*".repeat(100_000))), big);
        // The limits leave room for honest patterns.
        assert!(Regex::new(&units(&"(a)".repeat(MAX_GROUPS)), false).is_ok());
        assert_eq!(err(&"(a)".repeat(MAX_GROUPS + 1)), big);
        assert!(Regex::new(&units(&"(?:a)".repeat(1000)), false).is_ok());
        assert!(Regex::new(&units("[a-z]{1000}"), false).is_ok());
    }

    /// Acceptance test 15: the pathological pattern of a backtracking
    /// engine, on a long subject, finishes at once.
    #[test]
    fn a_pathological_pattern_on_a_long_subject_is_fast() {
        let text = vec![u('a'); 100_000];
        let start = std::time::Instant::now();
        assert!(
            re("(a*)*b")
                .find(&text, false, usize::MAX)
                .unwrap()
                .is_empty()
        );
        assert!(
            re("(a|aa)*c")
                .find(&text, false, usize::MAX)
                .unwrap()
                .is_empty()
        );
        let m = re("(a*)*").find(&text, false, usize::MAX).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0][0], Some((0, 100_000)));
        // The pattern that makes repeated searches quadratic: its preferred
        // branch runs to the end of the text before each one-unit match.
        let text = vec![u('x'); 100_000];
        let m = re("x*y|x").find(&text, false, usize::MAX).unwrap();
        assert_eq!(m.len(), 100_000);
        assert!(start.elapsed().as_secs() < 20, "{:?}", start.elapsed());
    }

    #[test]
    fn classes_are_normalised_ranges_with_negation_applied() {
        let set = |p: &str| re(p).classes[0].clone();
        assert_eq!(
            set("[c-ea-bx]").ranges,
            [(u('a'), u('e')), (u('x'), u('x'))]
        );
        assert_eq!(set("[bbbb]").ranges, [(u('b'), u('b'))]);
        assert_eq!(set("[^b]").ranges, [(0, u('a')), (u('c'), u32::MAX)]);
        assert_eq!(set("[\\d\\D]").ranges, [(0, u32::MAX)]);
        assert!(set("[^\\s\\S]").ranges.is_empty());
        assert_eq!(all("[\\Wa]+", "ab a!b"), ["a", " a!"]);
        assert_eq!(all("[^\\w ]+", "ab, c!!"), [",", "!!"]);
        assert!(all("[^\\s\\S]", "abc").is_empty());
        // A class written twice, and every copy a repetition makes, is one.
        assert_eq!(re("\\w+\\s\\w+[ab][ab]{5}").classes.len(), 3);
        // A negated class under 'ignorecase' excludes both cases.
        let r = Regex::new(&units("[^a]"), true).unwrap();
        assert!(r.find(&units("aA"), false, 9).unwrap().is_empty());
        assert_eq!(re("[^a]").find(&units("aA"), false, 9).unwrap().len(), 1);
        let r = Regex::new(&units("[^a-z]+"), true).unwrap();
        assert_eq!(
            r.find(&units("aB1Cd"), false, 9).unwrap()[0][0],
            Some((2, 3))
        );
    }

    /// A class counts its ranges against the program's size, is built once
    /// and is bisected. The class of ten thousand `b`s copied 19,000 times
    /// was built afresh for every copy, 2 GB and 12 seconds before it matched
    /// anything; a class of ten thousand ranges was scanned item by item for
    /// every code unit, half a billion comparisons over 100,000 units.
    #[test]
    fn a_pathological_class_is_compiled_once_and_matched_fast() {
        let start = std::time::Instant::now();
        let p = format!("(?:[{}]{{1000}}){{19}}", "b".repeat(10_000));
        let r = re(&p);
        assert_eq!(r.classes.len(), 1);
        let text = vec![u('a'); 100_000];
        assert!(r.find(&text, false, usize::MAX).unwrap().is_empty());
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 1000, "{elapsed:?}");
        // Ten thousand disjoint ranges, bisected per code unit. A debug
        // build spends most of this in the VM itself, which a one-range
        // class costs as much of.
        let start = std::time::Instant::now();
        let class: String = (0..10_000u32)
            .map(|k| char::from_u32(0x4E00 + 2 * k).unwrap())
            .collect();
        let r = re(&format!("[{class}]+"));
        assert_eq!(r.classes[0].ranges.len(), 10_000);
        let text: Vec<u32> = (0..100_000u32).map(|k| 0x4E00 + 2 * (k % 10_000)).collect();
        let m = r.find(&text, false, usize::MAX).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0][0], Some((0, 100_000)));
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 2000, "{elapsed:?}");
        // A class past the bound is too large.
        let big: String = (0..=MAX_PROGRAM as u32)
            .map(|k| char::from_u32(0x100 + 2 * k).unwrap())
            .collect();
        assert_eq!(
            err(&format!("[{big}]")),
            "The regular expression is too large."
        );
        // The ranges count with the instructions.
        let half: String = (0..MAX_PROGRAM as u32 / 2)
            .map(|k| char::from_u32(0x100 + 2 * k).unwrap())
            .collect();
        assert!(Regex::new(&units(&format!("[{half}]")), false).is_ok());
        assert_eq!(
            err(&format!("[{half}](?:a{{1000}}){{10}}")),
            "The regular expression is too large."
        );
        // `\w` costs its ranges once however often it is written.
        assert!(Regex::new(&units(&"\\w".repeat(5000)), false).is_ok());
    }

    #[test]
    fn the_limit_stops_early() {
        assert_eq!(re("a").find(&units("aaaa"), false, 2).unwrap().len(), 2);
        assert!(re("a").find(&units("aaaa"), false, 0).unwrap().is_empty());
    }
}
