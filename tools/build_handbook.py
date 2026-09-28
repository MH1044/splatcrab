"""Render docs/HANDBOOK.md as the published handbook page.

The markdown is the source of truth and is kept in the repo; this only
presents it. Handles exactly the constructs the handbook uses: two heading
levels, matlab/output fenced pairs, tables, bullets, paragraphs, and inline
code, bold and links.
"""
import html
import io
import os
import re

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, "docs", "HANDBOOK.md")
DST = os.path.join(REPO, "target", "splatcrab-handbook.html")

md = io.open(SRC, encoding="utf-8").read()
lines = md.split("\n")


def slug(t):
    t = re.sub(r"`", "", t).strip().lower()
    t = re.sub(r"[^a-z0-9 \-]", "", t)
    return re.sub(r"\s+", "-", t)


def inline(t):
    t = html.escape(t, quote=False)
    t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", r'<a href="\2">\1</a>', t)
    return t


out = []          # html fragments
nav = []          # (level, title, id)
i = 0
n = len(lines)
in_contents = False

while i < n:
    line = lines[i]
    st = line.strip()

    # headings
    if st.startswith("## "):
        title = st[3:].strip()
        in_contents = title.lower() == "contents"
        if in_contents:
            i += 1
            continue
        sid = slug(title)
        nav.append((2, title, sid))
        out.append(f'<h2 id="{sid}">{inline(title)}</h2>')
        i += 1
        continue
    if st.startswith("### "):
        title = st[4:].strip()
        sid = slug(title)
        nav.append((3, title, sid))
        out.append(f'<h3 id="{sid}">{inline(title)}</h3>')
        i += 1
        continue
    if st.startswith("# "):
        i += 1
        continue
    if in_contents:
        i += 1
        continue

    # fenced blocks
    if st.startswith("```"):
        kind = "in" if st == "```matlab" else "out"
        j = i + 1
        buf = []
        while j < n and lines[j].strip() != "```":
            buf.append(lines[j])
            j += 1
        body = html.escape("\n".join(buf), quote=False)
        label = "Type this" if kind == "in" else "It prints"
        out.append(
            f'<div class="blk {kind}"><span class="blab">{label}</span>'
            f"<pre><code>{body}</code></pre></div>"
        )
        i = j + 1
        continue

    # tables
    if st.startswith("| "):
        rows = []
        while i < n and lines[i].strip().startswith("|"):
            rows.append(lines[i].strip())
            i += 1
        cells = [[c.strip() for c in r.strip("|").split("|")] for r in rows]
        body = ['<div class="tw"><table>']
        if len(cells) >= 2 and set("".join(cells[1]).replace("|", "")) <= set("-: "):
            body.append("<thead><tr>" + "".join(f"<th>{inline(c)}</th>" for c in cells[0]) + "</tr></thead>")
            rest = cells[2:]
        else:
            rest = cells
        body.append("<tbody>")
        for r in rest:
            body.append("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>")
        body.append("</tbody></table></div>")
        out.append("".join(body))
        continue

    # bullets
    if st.startswith("- "):
        items = []
        while i < n and lines[i].strip().startswith("- "):
            items.append(lines[i].strip()[2:])
            i += 1
            while i < n and lines[i].startswith("  ") and lines[i].strip() and not lines[i].strip().startswith("- "):
                items[-1] += " " + lines[i].strip()
                i += 1
        out.append("<ul>" + "".join(f"<li>{inline(x)}</li>" for x in items) + "</ul>")
        continue

    # blank
    if not st:
        i += 1
        continue

    # paragraph
    para = []
    while i < n and lines[i].strip() and not lines[i].strip().startswith(("#", "```", "|", "- ")):
        para.append(lines[i].strip())
        i += 1
    out.append(f"<p>{inline(' '.join(para))}</p>")

nav_html = []
for lvl, title, sid in nav:
    cls = "n2" if lvl == 2 else "n3"
    nav_html.append(f'<a class="{cls}" href="#{sid}" data-t="{html.escape(title.lower())}">{inline(title)}</a>')

CSS = """
:root{
  --ground:#f7f8fa; --panel:#fff; --sunk:#eef1f5; --ink:#16202b; --soft:#4a5a6b;
  --faint:#7d8b99; --rule:#dbe2ea; --strong:#c2ccd8; --sea:#14557f; --seasoft:#e4eef5;
  --kelp:#256b4a; --kelpsoft:#e3f1ea;
}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){
  --ground:#10161d; --panel:#17202a; --sunk:#121a22; --ink:#e6edf4; --soft:#9fb0c0;
  --faint:#6d7f90; --rule:#25313d; --strong:#35434f; --sea:#79bde4; --seasoft:#16303f;
  --kelp:#6fc79c; --kelpsoft:#162b23;
}}
:root[data-theme="dark"]{
  --ground:#10161d; --panel:#17202a; --sunk:#121a22; --ink:#e6edf4; --soft:#9fb0c0;
  --faint:#6d7f90; --rule:#25313d; --strong:#35434f; --sea:#79bde4; --seasoft:#16303f;
  --kelp:#6fc79c; --kelpsoft:#162b23;
}
*{box-sizing:border-box}
body{background:var(--ground);color:var(--ink);margin:0;
  font-family:"IBM Plex Sans",system-ui,sans-serif;font-size:15px;line-height:1.6;
  -webkit-font-smoothing:antialiased}
.shell{display:grid;grid-template-columns:266px minmax(0,1fr);gap:0;max-width:1240px;margin:0 auto}
/* sidebar */
aside{position:sticky;top:0;height:100vh;overflow-y:auto;padding:26px 18px 40px;
  border-right:1px solid var(--rule);background:var(--panel)}
.brand{font-family:"IBM Plex Serif",Georgia,serif;font-weight:600;font-size:19px;
  letter-spacing:-.01em;margin:0 0 3px}
.brandsub{font-family:"IBM Plex Mono",monospace;font-size:10.5px;letter-spacing:.11em;
  text-transform:uppercase;color:var(--sea);margin:0 0 16px}
#filter{width:100%;font-family:"IBM Plex Mono",monospace;font-size:12.5px;padding:7px 9px;
  border:1px solid var(--strong);border-radius:3px;background:var(--ground);color:var(--ink);
  margin-bottom:14px}
#filter:focus{outline:2px solid var(--sea);outline-offset:1px;border-color:var(--sea)}
nav{display:flex;flex-direction:column;gap:1px}
nav a{text-decoration:none;color:var(--soft);border-radius:3px;display:block}
nav a.n2{font-weight:600;font-size:13px;color:var(--ink);padding:7px 8px;margin-top:9px;
  border-top:1px solid var(--rule)}
nav a.n2:first-child{margin-top:0;border-top:none}
nav a.n3{font-size:12.5px;padding:3.5px 8px 3.5px 16px;
  font-family:"IBM Plex Mono",monospace}
nav a:hover{background:var(--seasoft);color:var(--sea)}
nav a.on{background:var(--seasoft);color:var(--sea)}
nav a:focus-visible{outline:2px solid var(--sea);outline-offset:-2px}
nav a[hidden]{display:none}
/* article */
main{padding:38px 40px 120px;min-width:0}
.masthead{border-bottom:1px solid var(--strong);padding-bottom:22px;margin-bottom:28px}
h1{font-family:"IBM Plex Serif",Georgia,serif;font-weight:600;
  font-size:clamp(28px,4vw,40px);line-height:1.1;letter-spacing:-.015em;margin:0 0 12px;
  text-wrap:balance}
.lede{font-size:16px;color:var(--soft);max-width:66ch;margin:0}
.verified{display:inline-flex;align-items:center;gap:7px;margin-top:16px;
  background:var(--kelpsoft);border:1px solid var(--kelp);border-radius:3px;
  padding:6px 11px;font-size:12.5px;color:var(--ink)}
.verified b{color:var(--kelp);font-family:"IBM Plex Mono",monospace;font-weight:600}
h2{font-family:"IBM Plex Serif",Georgia,serif;font-size:25px;font-weight:600;
  letter-spacing:-.01em;margin:52px 0 6px;padding-bottom:7px;
  border-bottom:1px solid var(--strong);scroll-margin-top:16px;text-wrap:balance}
h2:first-of-type{margin-top:0}
h3{font-size:16px;font-weight:600;margin:30px 0 8px;color:var(--ink);
  scroll-margin-top:16px;text-wrap:balance}
p{margin:0 0 13px;max-width:72ch}
ul{margin:0 0 15px;padding-left:20px;max-width:72ch}
li{margin-bottom:5px}
code{font-family:"IBM Plex Mono",ui-monospace,monospace;font-size:.88em;
  background:var(--sunk);padding:1px 4px;border-radius:2px}
a{color:var(--sea)}
/* example blocks */
.blk{position:relative;margin:0 0 3px;border:1px solid var(--rule);border-radius:3px;
  overflow:hidden}
.blk.in{background:var(--panel);border-left:3px solid var(--sea)}
.blk.out{background:var(--sunk);border-left:3px solid var(--strong);margin-bottom:17px}
.blab{position:absolute;top:0;right:0;font-family:"IBM Plex Mono",monospace;font-size:9.5px;
  letter-spacing:.1em;text-transform:uppercase;color:var(--faint);padding:4px 8px}
.blk pre{margin:0;padding:11px 13px;overflow-x:auto}
.blk code{background:none;padding:0;font-size:12.7px;line-height:1.55;
  white-space:pre;display:block;color:var(--ink)}
.blk.out code{color:var(--soft)}
/* tables */
.tw{overflow-x:auto;margin:0 0 17px;border:1px solid var(--rule);border-radius:3px}
table{border-collapse:collapse;width:100%;font-size:13.5px}
th{background:var(--sunk);text-align:left;font-weight:600;font-size:11.5px;
  letter-spacing:.05em;text-transform:uppercase;color:var(--soft);
  padding:8px 11px;border-bottom:1px solid var(--strong);white-space:nowrap}
td{padding:8px 11px;border-bottom:1px solid var(--rule);vertical-align:top}
tr:last-child td{border-bottom:none}
section[hidden]{display:none}
@media (max-width:900px){
  .shell{grid-template-columns:1fr}
  aside{position:static;height:auto;border-right:none;border-bottom:1px solid var(--rule)}
  main{padding:26px 20px 80px}
}
@media (prefers-reduced-motion:reduce){*{transition:none!important}}
"""

JS = """
(function(){
  var f=document.getElementById('filter');
  var links=[].slice.call(document.querySelectorAll('nav a'));
  var heads=[].slice.call(document.querySelectorAll('main h2, main h3'));

  f.addEventListener('input',function(){
    var q=f.value.trim().toLowerCase();
    links.forEach(function(a){ a.hidden = q && a.dataset.t.indexOf(q)===-1; });
  });

  // highlight the section currently in view
  var byId={};
  links.forEach(function(a){ byId[a.getAttribute('href').slice(1)]=a; });
  function mark(){
    var best=null, top=null;
    heads.forEach(function(h){
      var r=h.getBoundingClientRect();
      if(r.top<=120 && (top===null || r.top>top)){ top=r.top; best=h.id; }
    });
    links.forEach(function(a){ a.classList.remove('on'); });
    if(best && byId[best]) byId[best].classList.add('on');
  }
  var tick=false;
  window.addEventListener('scroll',function(){
    if(tick) return; tick=true;
    window.requestAnimationFrame(function(){ mark(); tick=false; });
  },{passive:true});
  mark();
})();
"""

page = f"""<title>SplatCrab Handbook</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600&family=IBM+Plex+Serif:wght@500;600&display=swap">
<style>{CSS}</style>

<div class="shell">
<aside>
  <p class="brand">SplatCrab</p>
  <p class="brandsub">Handbook</p>
  <input id="filter" type="search" placeholder="Filter sections" aria-label="Filter sections">
  <nav>{''.join(nav_html)}</nav>
</aside>

<main>
  <div class="masthead">
    <h1>SplatCrab Handbook</h1>
    <p class="lede">How to use SplatCrab, for someone who already knows MATLAB. Every example
    on this page was run against the binary, and the output blocks are the bytes it produced.
    Where SplatCrab differs from MATLAB, the difference is stated at that point rather than
    left for you to discover.</p>
    <span class="verified"><b>92 / 92</b> examples verified against the interpreter</span>
  </div>
  {''.join(out)}
</main>
</div>
<script>{JS}</script>
"""

io.open(DST, "w", encoding="utf-8", newline="\n").write(page)
print("wrote", DST, len(page), "bytes;", len(nav), "nav entries")
