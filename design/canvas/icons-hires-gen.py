#!/usr/bin/env python3
"""Builds project/IconsHiRes.dc.html: a 12x12 pixel icon set for tab identities.

Owner request (2026-11-05): each tab opens with a different random icon, drawn with a bit
more pixels than the 8x8 ao-i masks. ICONS holds the pixel data ('#' on, anything else off);
a drawing shorter or narrower than 12 is centred. `mask(name)` gives the SVG a later
ao-i-* class can use as its --ao-i mask.
"""
from pathlib import Path
from urllib.parse import quote

N = 12
OUT = Path(__file__).parent / "project" / "IconsHiRes.dc.html"

ICONS = {
    "terminal": """
############
#..........#
#.##.......#
#..##......#
#...##.....#
#..##......#
#.##..####.#
#..........#
############""",
    "rocket": """
.....##.....
....####....
....#..#....
...##..##...
...#.##.#...
...#.##.#...
...#....#...
..##....##..
.###.##.###.
.##..##..##.
.....##.....
....#..#....""",
    "bug": """
..#......#..
...#....#...
....####....
#..######..#
.#.##..##.#.
...######...
####.##.####
...######...
.#.##..##.#.
#..######..#
....####....""",
    "database": """
..########..
.#........#.
.##......##.
.#.######.#.
.#........#.
.##......##.
.#.######.#.
.#........#.
.##......##.
..########..""",
    "cloud": """
....####....
...#....#...
.###.....##.
#.........#.
#..........#
#..........#
.##########.""",
    "gear": """
.....##.....
..#..##..#..
...######...
..##....##..
..#..##..#..
###.####.###
###.####.###
..#..##..#..
..##....##..
...######...
..#..##..#..
.....##.....""",
    "flask": """
....####....
.....##.....
.....##.....
.....##.....
....#..#....
...#....#...
..#......#..
..#.####.#..
.#.######.#.
.#.######.#.
..########..""",
    "book": """
.####..####.
#....##....#
#.##.##.##.#
#....##....#
#.##.##.##.#
#....##....#
#.##.##.##.#
#....##....#
.####..####.""",
    "key": """
..####......
.#....#.....
#..##..#....
#..##..#....
.#....######
..####..#.##
........#.##""",
    "globe": """
....####....
..##.##.##..
.#..#..#..#.
.#..#..#..#.
############
#...#..#...#
#...#..#...#
############
.#..#..#..#.
.#..#..#..#.
..##.##.##..
....####....""",
    "branch": """
..##.....##.
.#..#...#..#
.#..#...#..#
..##.....##.
...#......#.
...#......#.
...#.....#..
...#...##...
...#.##.....
..##........
.#..#.......
..##........""",
    "package": """
..########..
.#...##...#.
############
#....##....#
#....##....#
#..........#
#..........#
#..........#
#..........#
############""",
    "bolt": """
.......###..
......###...
.....###....
....###.....
...#######..
......###...
.....###....
....###.....
...###......
..##........
.#..........""",
    "heart": """
.###....###.
#####..#####
############
############
.##########.
..########..
...######...
....####....
.....##.....""",
    "star": """
.....##.....
.....##.....
....####....
....####....
############
.##########.
..########..
...######...
...######...
..###..###..
..##....##..
.#........#.""",
    "flag": """
.#..........
.#########..
.#########..
.##########.
.##########.
.#########..
.#..........
.#..........
.#..........
.#..........
.#..........
###.........""",
    "coffee": """
...#..#.....
....#..#....
...#..#.....
............
#########...
#.......####
#.......#..#
#.......#..#
#.......####
.#.....#....
..#####.....
############""",
    "ghost": """
....####....
..########..
.##########.
.##..##..##.
.##..##..##.
############
############
############
############
############
##.###.###.#
#...#...#...""",
    "chart": """
#...........
#.........##
#.........##
#......##.##
#......##.##
#...##.##.##
#...##.##.##
#.#.##.##.##
#.#.##.##.##
#.#.##.##.##
#...........
############""",
    "lock": """
....####....
...#....#...
..#......#..
..#......#..
..#......#..
############
############
#####..#####
#####..#####
######.#####
############
############""",
    "mail": """
############
##........##
#.#......#.#
#..#....#..#
#...#..#...#
#....##....#
#..........#
#..........#
############""",
    "music": """
......#.....
......##....
......#.#...
......#..#..
......#...#.
......#...#.
......#..#..
......#.....
..#####.....
.######.....
.#####......
..###.......""",
    "leaf": """
..........##
......######
....####..##
...###...#.#
..###...#..#
..##...#..##
.##...#...#.
.#...#...##.
.#..#..###..
..##.####...
.#..........
#...........""",
    "cube": """
....####....
..##....##..
##........##
#.##....##.#
#...####...#
#....##....#
#....##....#
#....##....#
#....##....#
.##..##..##.
...######...""",
    "robot": """
.....##.....
.....##.....
..########..
.#........#.
.#.##..##.#.
.#.##..##.#.
.#........#.
.#..####..#.
..########..
#.#......#.#
#.#......#.#
..##....##..""",
    "moon": """
...####.....
.####.......
.###........
###.........
###.........
###.........
###.........
####......#.
.####....##.
.########...
...#####....""",
}


def grid(name):
    rows = [r for r in ICONS[name].strip("\n").split("\n")]
    assert len(rows) <= N and all(len(r) <= N for r in rows), name
    top = (N - len(rows)) // 2
    w = max(len(r) for r in rows)
    left = (N - w) // 2
    g = [["."] * N for _ in range(N)]
    for y, r in enumerate(rows):
        for x, c in enumerate(r):
            if c == "#":
                g[top + y][left + x] = "#"
    return g


def rects(name):
    out = []
    for y, row in enumerate(grid(name)):
        x = 0
        while x < N:
            if row[x] == "#":
                s = x
                while x < N and row[x] == "#":
                    x += 1
                out.append(f"<rect x='{s}' y='{y}' width='{x - s}' height='1'/>")
            else:
                x += 1
    return "".join(out)


def svg(name, px, color="currentColor"):
    return (f'<svg viewBox="0 0 {N} {N}" width="{px}" height="{px}" shape-rendering="crispEdges" '
            f'fill="{color}" aria-label="{name}" role="img" style="display: block; flex: none">{rects(name)}</svg>')


def mask(name):
    """The data URI an ao-i-<name> class would carry, as the 8x8 set does."""
    body = f"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {N} {N}' shape-rendering='crispEdges'>{rects(name)}</svg>"
    return "data:image/svg+xml," + quote(body, safe="='/ :")


NEON = ["#2ee6ff", "#ff4fd8", "#9dff3d", "#a78bff", "#3dffb0", "#4f8dff", "#ff5c8a", "#b8f3ff"]
OLD = ["term", "agent", "group", "tab", "window", "split", "doc", "tpl", "prompt", "var",
       "link", "pin", "play", "check", "bp", "pr", "merge", "save", "in", "out"]


def cell(name):
    return (f'<div style="display: grid; justify-items: center; gap: 8px; padding: 10px 4px; background: var(--bg); border: 2px solid var(--line)">'
            f'<div style="display: flex; align-items: flex-end; gap: 12px; height: 48px">{svg(name, 12)}{svg(name, 24)}{svg(name, 48)}</div>'
            f'<span class="small ao-dim">{name}</span></div>')


def side_row(name, icon, meta, hue, active=False):
    cls = "ao-st active" if active else "ao-st"
    return (f'<div class="{cls}" style="--st-accent: {hue}"><span style="display: grid; place-items: center; width: 16px; height: 16px; color: {hue}">{svg(icon, 12)}</span>'
            f'<span class="t">{name}</span><span class="ao-stat">Idle</span><span class="m ao-dim">{meta}</span></div>')


def old_row(name, icon, meta):
    return (f'<div class="ao-st"><i class="ao-i ao-i-{icon}"></i>'
            f'<span class="t">{name}</span><span class="ao-stat">Idle</span><span class="m ao-dim">{meta}</span></div>')


def tab(name, icon, active=False, hue="currentColor"):
    cls = "ao-tab active" if active else "ao-tab"
    return (f'<span class="{cls}" style="--tab-accent: {hue}"><span style="color: {hue if active else "var(--dim)"}">{svg(icon, 12)}</span>'
            f'<span class="n">{name}</span></span>')


names = list(ICONS)
grid_html = "".join(cell(n) for n in names)
old_html = "".join(f'<span title="{o}" style="display: grid; place-items: center; width: 24px; height: 24px; color: var(--fg)"><i class="ao-i ao-i-{o}"></i></span>' for o in OLD)
rows_new = "".join([
    side_row("PROJ-482 audit export", "rocket", "3 tabs", NEON[0]),
    side_row("acme/api", "database", "claude · implement", NEON[1]),
    side_row("docs", "book", "zsh · ~/work/docs", NEON[2], active=True),
    side_row("deploy", "cloud", "kubectl rollout", NEON[3]),
    side_row("k9s", "ghost", "k9s -n staging", NEON[4]),
    side_row("notes", "leaf", "notes.md", NEON[5]),
])
rows_old = "".join([
    old_row("PROJ-482 audit export", "group", "3 tabs"),
    old_row("acme/api", "agent", "claude · implement"),
    old_row("docs", "doc", "zsh · ~/work/docs"),
    old_row("deploy", "term", "kubectl rollout"),
    old_row("k9s", "term", "k9s -n staging"),
    old_row("notes", "doc", "notes.md"),
])
tabs_new = "".join([
    tab("zsh · ~/work/docs", "terminal", True, NEON[2]),
    tab("pnpm test", "flask"),
    tab("claude · review", "robot"),
    tab("git log", "branch"),
    tab("psql", "database"),
])

html = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Tab icons, 12 x 12</title>
<script src="./support.js"></script>
<link rel="stylesheet" href="ds/agentorch/tokens.css">
<link rel="stylesheet" href="ds/agentorch/components/bundle.css">
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Geist+Mono:wght@400;600&amp;family=Silkscreen&amp;display=swap">
<style>body{{margin:0;background:#111318}}</style>
</helmet>
<div style="width: 2960px; height: 820px; box-sizing: border-box; padding: 48px; display: flex; flex-direction: column; gap: 32px; background: var(--bg); color: var(--fg); font-family: var(--font-mono)">
  <header style="display: flex; flex-direction: column; gap: 10px">
    <h1 class="pixel-display" style="margin: 0; color: var(--fg)">Tab icons · 12 x 12</h1>
    <p class="body" style="margin: 0; max-width: 1600px; color: var(--dim)">{len(names)} icons for tab identities, drawn on a 12 x 12 grid instead of 8 x 8. A new tab takes a random icon that no other tab in the window uses yet; the side tab row shows its first tab's icon until it becomes a group. Drawn at 12 px, one grid cell is one point, so each pixel lands on two device pixels on a Retina screen and stays crisp.</p>
  </header>
  <div style="display: flex; gap: 32px; align-items: flex-start">
    <section aria-labelledby="h-set" style="box-sizing: border-box; display: flex; flex-direction: column; gap: 16px; width: 1560px; padding: 20px; background: var(--panel); box-shadow: var(--frame-amber)">
      <div style="display: flex; align-items: center; gap: 10px"><h2 id="h-set" class="pixel-heading" style="margin: 0">The set</h2><span class="ao-pixel" style="font-size: 8px; padding: 2px 6px; color: var(--on-amber); background: var(--amber)">Recommended</span><span class="small ao-dim">each at 12 px (1x), 24 px (2x) and 48 px</span></div>
      <div style="display: grid; grid-template-columns: repeat(9, 1fr); gap: 10px">{grid_html}</div>
      <div style="display: flex; align-items: center; gap: 16px; padding-top: 8px; border-top: 2px solid var(--line)">
        <span class="ao-pixel" style="font-size: 8px; color: var(--dim)">Today, 8 x 8 at 16 px</span>
        <div style="display: flex; flex-wrap: wrap; gap: 4px">{old_html}</div>
      </div>
    </section>
    <div style="display: flex; flex-direction: column; gap: 32px">
      <div style="display: flex; gap: 32px">
        <section aria-labelledby="h-now" style="box-sizing: border-box; display: flex; flex-direction: column; gap: 12px; width: 380px; padding: 20px; background: var(--panel); box-shadow: var(--frame-neutral)">
          <h2 id="h-now" class="pixel-heading" style="margin: 0">Sidebar today</h2>
          <nav class="ao-side" aria-label="Sidebar today" style="display: flex; flex-direction: column; gap: 4px; padding: 8px; background: var(--bg); border: 2px solid var(--line)">{rows_old}</nav>
        </section>
        <section aria-labelledby="h-new" style="box-sizing: border-box; display: flex; flex-direction: column; gap: 12px; width: 380px; padding: 20px; background: var(--panel); box-shadow: var(--frame-neutral)">
          <h2 id="h-new" class="pixel-heading" style="margin: 0">Sidebar, 12 x 12 + neon</h2>
          <nav class="ao-side" aria-label="Sidebar with new icons" style="display: flex; flex-direction: column; gap: 4px; padding: 8px; background: var(--bg); border: 2px solid var(--line)">{rows_new}</nav>
        </section>
      </div>
      <section aria-labelledby="h-tabs" style="box-sizing: border-box; display: flex; flex-direction: column; gap: 12px; width: 792px; padding: 20px; background: var(--panel); box-shadow: var(--frame-neutral)">
        <h2 id="h-tabs" class="pixel-heading" style="margin: 0">Tab bar, real size</h2>
        <div class="ao-tabs" style="background: var(--bg)">{tabs_new}</div>
        <p class="small ao-dim" style="margin: 0">Inactive tabs draw the icon in the dim ink; the active tab takes its row's neon. Claude's state dot stays next to the icon, so the agent icon is no longer needed to spot a session.</p>
      </section>
    </div>
  </div>
</div>
</x-dc>
</body>
</html>
'''
OUT.write_text(html)
print("wrote", OUT, len(names), "icons")
