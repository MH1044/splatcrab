# tools

Two optional development scripts. Neither is part of the build, neither is a
crate, and nothing in CI depends on them. They need Python 3; the interpreter
itself needs only Rust.

## The handbook verifier moved to `tests/handbook.rs`

`verify_handbook.py` used to live here. It is now the integration test
`tests/handbook.rs`, with the same extraction rules, so `cargo test` runs it
and CI enforces it on both platforms: the handbook can no longer rot between
cycles because nobody remembered to run a script.

```
cargo test --test handbook
```

A failure prints the handbook line, the script, the handbook's output and the
binary's. The output blocks are the real bytes the binary produced, never
output written from MATLAB knowledge: on this project, output written from
recall rather than run has been wrong roughly one time in fifteen.

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
