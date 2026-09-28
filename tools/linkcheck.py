"""Report markdown links that point at a file the repository does not track.

A link to a file that exists only on the author's machine is invisible locally
and broken for everyone else, so this is worth running before a docs push.
"""
import io
import os
import re
import subprocess

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def git(*args):
    return subprocess.run(
        ["git"] + list(args), capture_output=True, text=True, cwd=REPO
    ).stdout.split("\n")


tracked = {p.strip().replace("\\", "/") for p in git("ls-files") if p.strip()}
mds = [p.strip() for p in git("ls-files", "*.md") if p.strip()]

bad = 0
for f in mds:
    text = io.open(os.path.join(REPO, f), encoding="utf-8").read()
    for m in re.finditer(r"\]\(([^)]+)\)", text):
        target = m.group(1).strip()
        if target.startswith(("http://", "https://", "mailto:", "#")):
            continue
        path = target.split("#")[0]
        if not path:
            continue
        rel = os.path.normpath(os.path.join(os.path.dirname(f), path))
        rel = rel.replace("\\", "/")
        if rel not in tracked:
            print("  %s -> %s" % (f, target))
            bad += 1

print("dead local links:", bad)
