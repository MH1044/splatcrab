"""Independently verify every example in docs/HANDBOOK.md.

Extracts each ```matlab block and the plain ``` block that follows it, runs the
script through the built binary, and compares. Normalisation matches the golden
harness: CRLF to LF, trailing whitespace stripped per line, trailing blank
lines dropped.
"""
import io
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HB = os.path.join(REPO, "docs", "HANDBOOK.md")
BIN = os.path.join(REPO, "target", "debug", "splatcrab.exe")
WORK = os.path.join(REPO, "target", "hbcheck")
os.makedirs(WORK, exist_ok=True)

text = io.open(HB, encoding="utf-8").read()
lines = text.split("\n")


def line_no(idx):
    return idx + 1


# Walk the file collecting (matlab_block, next_plain_block, line).
pairs = []
i = 0
while i < len(lines):
    if lines[i].strip() == "```matlab":
        start = i + 1
        j = start
        while j < len(lines) and lines[j].strip() != "```":
            j += 1
        script = "\n".join(lines[start:j])
        script_line = line_no(start)
        # find the next fenced block, allowing prose in between
        k = j + 1
        out = None
        while k < len(lines):
            st = lines[k].strip()
            if st == "```":
                m = k + 1
                while m < len(lines) and lines[m].strip() != "```":
                    m += 1
                out = "\n".join(lines[k + 1 : m])
                k = m
                break
            if st.startswith("```"):
                break  # another language block: this example has no output block
            if st.startswith("#"):
                break  # new heading before any output block
            k += 1
        if out is not None:
            pairs.append((script, out, script_line))
        i = j + 1
    else:
        i += 1


def norm(s):
    ls = [l.rstrip() for l in s.replace("\r\n", "\n").split("\n")]
    while ls and ls[-1] == "":
        ls.pop()
    return "\n".join(ls)


ok = 0
bad = []
skipped = 0
for n, (script, expected, ln) in enumerate(pairs):
    if "..." in expected and "…" not in expected and expected.strip().endswith("..."):
        skipped += 1
        continue
    p = os.path.join(WORK, "case%03d.m" % n)
    io.open(p, "w", encoding="utf-8", newline="\n").write(script + "\n")
    try:
        r = subprocess.run([BIN, p], capture_output=True, text=True, timeout=20)
    except subprocess.TimeoutExpired:
        bad.append((ln, script, expected, "<<TIMED OUT>>"))
        continue
    actual = r.stdout + r.stderr
    if norm(actual) == norm(expected):
        ok += 1
    else:
        bad.append((ln, script, expected, actual))

print("examples found: %d" % len(pairs))
print("skipped (elided output): %d" % skipped)
print("MATCH: %d" % ok)
print("MISMATCH: %d" % len(bad))
print()
for ln, script, expected, actual in bad[:40]:
    print("=" * 70)
    print("HANDBOOK.md line %d" % ln)
    print("--- script ---")
    print(script[:400])
    print("--- expected ---")
    print(norm(expected)[:600])
    print("--- actual ---")
    print(norm(actual)[:600])
    print()
if len(bad) > 40:
    print("... and %d more" % (len(bad) - 40))
