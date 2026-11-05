#!/usr/bin/env python3
"""Builds project/RowAccents.dc.html: options for a neon accent per sidebar row.

Owner request (2026-11-05): every row created in the sidebar gets a different neon color.
Each option shows the same seven rows, one waiting on the person (amber corner) and one with
a manual status background, so both stay readable next to the row's own neon.
"""
from pathlib import Path

OUT = Path(__file__).parent / "project" / "RowAccents.dc.html"

# Neon hues in assignment order. No amber, orange or yellow: amber means "needs you".
NEON = [
    ("cyan", "#2ee6ff"),
    ("pink", "#ff4fd8"),
    ("lime", "#9dff3d"),
    ("violet", "#a78bff"),
    ("mint", "#3dffb0"),
    ("blue", "#4f8dff"),
    ("rose", "#ff5c8a"),
    ("ice", "#b8f3ff"),
]

# (name, icon, chip, meta, flags)
ROWS = [
    ("PROJ-482 audit export", "group", ("Idle", None), "3 tabs", ""),
    ("acme/api", "agent", ("Review", "blue"), "claude · implement", "tint"),
    ("docs", "doc", ("Idle", None), "zsh · ~/work/docs", ""),
    ("deploy", "term", ("Waiting", "need"), "claude asks a question", "need"),
    ("k9s", "term", ("Running", "run"), "k9s -n staging", "active"),
    ("notes", "doc", ("Idle", None), "notes.md", ""),
    ("PROJ-477 rate limit", "group", ("Idle", None), "2 tabs", ""),
]

OPTIONS = [
    ("edge-icon", "Edge + icon", True,
     "The DS row already paints its edge and icon from --st-accent; the neon sets that var. "
     "Status keeps the chip and the background, amber keeps the corner square."),
    ("edge", "Edge only", False,
     "Only the 4 px left edge is neon. Calmest; the hue is easy to miss on short rows."),
    ("icon", "Icon only", False,
     "Neon icon, neutral edge. Reads as a sprite colour; the edge stays free for status."),
    ("name", "Name tint", False,
     "Neon side tab name. Loud, and lime and ice names lose contrast on a tinted background."),
    ("glow", "Glow frame", False,
     "1 px neon frame with a soft glow. Arcade feel; fights the white frame of the active row."),
    ("wash", "Wash", False,
     "Neon wash behind the row plus the edge. Collides with a status background (acme/api)."),
]


def chip(label, kind):
    if kind == "need":
        return '<span class="ao-stat need">Waiting</span>'
    if kind == "run":
        return '<span class="ao-stat" style="--s-fg: var(--blue)">Running</span>'
    if kind:
        return f'<span class="ao-stat manual" style="--s-fg: var(--{kind}); --s-edge: var(--{kind})">{label}</span>'
    return f'<span class="ao-stat">{label}</span>'


def row(opt, i, r):
    name, icon, (label, kind), meta, flags = r
    hue = NEON[i % len(NEON)][1]
    cls = "ao-st" + (" need" if "need" in flags else "") + (" active" if "active" in flags else "")
    bg = "var(--tint-blue)" if "tint" in flags else "var(--panel)"
    style = [f"--st-bg: {bg}", "position: relative"]
    icon_style = ""
    name_style = ""
    if opt == "edge-icon":
        style.append(f"--st-accent: {hue}")
    elif opt == "edge":
        style.append(f"--st-accent: {hue}")
        icon_style = ' style="color: var(--dim)"'
    elif opt == "icon":
        icon_style = f' style="color: {hue}"'
    elif opt == "name":
        name_style = f' style="color: {hue}"'
    elif opt == "glow":
        ring = "inset 0 0 0 2px var(--fg)" if "active" in flags else f"inset 0 0 0 1px {hue}"
        style.append(f"box-shadow: {ring}, 0 0 10px -2px {hue}")
        icon_style = f' style="color: {hue}"'
    elif opt == "wash":
        style.append(f"--st-accent: {hue}")
        if "tint" not in flags:
            style[0] = f"--st-bg: color-mix(in srgb, {hue} 12%, var(--panel))"
        else:
            style[0] = f"--st-bg: color-mix(in srgb, {hue} 12%, var(--tint-blue))"
    return (f'<div class="{cls}" style="{"; ".join(style)}"><i class="ao-i ao-i-{icon}"{icon_style}></i>'
            f'<span class="t"{name_style}>{name}</span>{chip(label, kind)}'
            f'<span class="m ao-dim">{meta}</span></div>')


def option(key, title, rec, note):
    rows = "\n        ".join(row(key, i, r) for i, r in enumerate(ROWS))
    tag = ('<span class="ao-pixel" style="font-size: 8px; padding: 2px 6px; color: var(--on-amber); '
           'background: var(--amber)">Recommended</span>') if rec else ""
    # The active row's tabs take its neon as the top line of the active tab.
    hue = NEON[4][1]
    tab_line = hue if key in ("edge-icon", "edge", "glow", "wash") else "transparent"
    tab_icon = f' style="color: {hue}"' if key in ("edge-icon", "icon", "glow") else ""
    return f'''
    <section aria-labelledby="h-{key}" style="box-sizing: border-box; display: flex; flex-direction: column; gap: 14px; width: 440px; padding: 20px; background: var(--panel); box-shadow: {"var(--frame-amber)" if rec else "var(--frame-neutral)"}">
      <div style="display: flex; align-items: center; gap: 10px"><h2 id="h-{key}" class="pixel-heading" style="margin: 0; color: var(--fg)">{title}</h2>{tag}</div>
      <p class="body" style="margin: 0; min-height: 60px; color: var(--dim)">{note}</p>
      <nav aria-label="Side tabs, {title}" class="ao-side" style="display: flex; flex-direction: column; gap: 4px; padding: 8px; background: var(--bg); border: 2px solid var(--line)">
        {rows}
      </nav>
      <div style="display: grid; gap: 6px">
        <span class="ao-pixel" style="font-size: 8px; color: var(--dim)">k9s tab bar</span>
        <div class="ao-tabs" style="background: var(--bg)">
          <span class="ao-tab active" style="--tab-accent: {tab_line}"><i class="ao-i x1 ao-i-term"{tab_icon}></i><span class="n">k9s -n staging</span></span>
          <span class="ao-tab"><i class="ao-i x1 ao-i-term"></i><span class="n">zsh · ~/work/ops</span></span>
        </div>
      </div>
    </section>'''


def palette():
    cells = "".join(
        f'<div style="display: grid; justify-items: center; gap: 6px"><span style="width: 40px; height: 40px; background: {hx}; box-shadow: 0 0 12px -2px {hx}"></span>'
        f'<span class="ao-pixel" style="font-size: 8px; color: var(--dim)">{i + 1} {n}</span><span class="small ao-dim">{hx}</span></div>'
        for i, (n, hx) in enumerate(NEON))
    return cells


html = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Sidebar row accents</title>
<script src="./support.js"></script>
<link rel="stylesheet" href="ds/agentorch/tokens.css">
<link rel="stylesheet" href="ds/agentorch/components/bundle.css">
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Geist+Mono:wght@400;600&amp;family=Silkscreen&amp;display=swap">
<style>
body{{margin:0;background:#111318}}
.ao-side .ao-st{{grid-template-columns: 16px minmax(0, 1fr) auto}}
</style>
</helmet>
<div style="width: 2960px; height: 1200px; box-sizing: border-box; padding: 48px; display: flex; flex-direction: column; gap: 32px; background: var(--bg); color: var(--fg); font-family: var(--font-mono)">
  <header style="display: flex; flex-direction: column; gap: 10px">
    <h1 class="pixel-display" style="margin: 0; color: var(--fg)">Sidebar · a neon per row</h1>
    <p class="body" style="margin: 0; max-width: 1500px; color: var(--dim)">Each new side tab takes the next unused hue below, in creation order, and the list cycles after eight. The hue stays with the row through renames, moves and restarts. A manual status still owns the chip and the background; amber still means "needs you". In every option, acme/api has the Review status with the blue background and deploy waits on the person.</p>
  </header>
  <div style="display: flex; align-items: flex-end; gap: 28px; padding: 16px 20px; background: var(--panel); box-shadow: var(--frame-neutral); align-self: flex-start">
    <span class="ao-pixel" style="font-size: 8px; color: var(--dim); align-self: center">Assignment order</span>
    {palette()}
    <span class="small ao-dim" style="align-self: center; max-width: 360px">No amber, orange or yellow. Red stays for errors in chips, so the warm slot is rose.</span>
  </div>
  <div style="display: flex; gap: 28px; align-items: stretch">
    {"".join(option(*o) for o in OPTIONS)}
  </div>
</div>
</x-dc>
</body>
</html>
'''
OUT.write_text(html)
print("wrote", OUT)
