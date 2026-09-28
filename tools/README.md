# tools

Two optional development scripts. Neither is part of the build, neither is a
crate, and nothing in CI depends on them. They need Python 3; the interpreter
itself needs only Rust.

## `verify_handbook.py`

Extracts every example from `docs/HANDBOOK.md`, runs it through
`target/debug/splatcrab`, and diffs the result against the output block printed
beside it.

```
cargo build
python tools/verify_handbook.py
```

It reports `MATCH` and `MISMATCH` counts and prints a diff for each mismatch.
**Run it after any cycle that changes behaviour the handbook documents.** That
is what the handbook item in the Definition of Done is asking for, and it is
how cycles 01d and 01e each caught the passages they had invalidated within
seconds of landing.

The output blocks it compares against are the real bytes the binary produced,
not output written from MATLAB knowledge. That distinction matters: on this
project, output written from recall rather than run has been wrong roughly one
time in fifteen.

**Worth doing when someone has the time:** rewrite this as a Rust integration
test, `tests/handbook.rs`, using the same extraction logic. It needs no crate,
it would run in CI on both platforms, and the handbook could then never rot
between cycles. As a Python script it only runs when somebody remembers.

## `build_handbook.py`

Renders `docs/HANDBOOK.md` to a standalone HTML page at
`target/splatcrab-handbook.html`: sidebar navigation, a section filter, and
input and output blocks styled apart. The markdown in `docs/` is the source of
truth; this only presents it.

It handles exactly the constructs the handbook uses, which is two heading
levels, fenced `matlab` and output pairs, tables, bullets, and inline code,
bold and links. It is not a general markdown renderer and will quietly ignore
anything else, so check the output if you add a new construct.

## `linkcheck.py`

Reports any markdown link pointing at a file the repository does not track.

```
python tools/linkcheck.py
```

A link to a file that exists only on one machine resolves fine for its author
and is broken for everyone else, which is how `docs/ROADMAP.md` came to cite a
document that was never published. Run it before a docs push.
