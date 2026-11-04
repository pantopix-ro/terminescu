import os, re, sys
ROOT=sys.argv[1]
ICONS = {
  "pin":   ["..XXXX..","..XXXX..","...XX...",".XXXXXX.","...XX...","...XX...","...XX...","...X...."],
  "check": ["........",".......X","......XX","X....XX.","XX..XX..",".XXXX...","..XX....","........"],
  "cross": ["........","XX....XX",".XX..XX.","..XXXX..","...XX...","..XXXX..",".XX..XX.","XX....XX"],
  "save":  ["XXXXXXX.","X.XXX.XX","X.XXX..X","X......X","X.XXXX.X","X.X..X.X","X.XXXX.X","XXXXXXXX"],
  "doc":   ["XXXXX...","X...XX..","X...XXX.","X.....X.","X.XXX.X.","X.....X.","X.XXX.X.","XXXXXXX."],
  "bp":    ["..XXXX..",".XXXXXX.","XXXXXXXX","XX....XX","XXXXXXXX","XXXXXXXX",".XXXXXX.","..XXXX.."],
  "play":  [".X......",".XX.....",".XXX....",".XXXX...",".XXXX...",".XXX....",".XX.....",".X......"],
  "link":  ["....XXX.","...X...X","...X..X.","..X.XX..","..XX.X..",".X..X...","X...X...",".XXX...."],
  "dot":   ["........","..XXXX..",".X....X.",".X....X.",".X....X.",".X....X.","..XXXX..","........"],
  "term":  ["XXXXXXXX","X......X","X.X....X","X..X...X","X.X..XXX","X......X","X......X","XXXXXXXX"],
  "pr":    [".X....X.","XXX..XXX",".X....X.",".X...X..",".X..X...",".X.X....","XXX.....",".X......"],
  "tpl":   ["XXXXXXXX","X......X","X.XXXX.X","X......X","X.XX.X.X","X......X","X.XXX..X","XXXXXXXX"],
  "agent": ["...XX...","X..XX..X",".X.XX.X.","..XXXX..","..XXXX..",".X.XX.X.","X..XX..X","...XX..."],
  "tab":   ["XXXX....","X..X....","X..XXXXX","X......X","X......X","X......X","X......X","XXXXXXXX"],
  "group": ["..XXXXXX","..X....X","XXXXXX.X","X....X.X","X....XXX","X....X..","XXXXXX..","........"],
  "split": ["XXXXXXXX","X..X...X","X..X...X","X..XXXXX","X..X...X","X..X...X","X..X...X","XXXXXXXX"],
  "window":["XXXXXXXX","XXXXXXXX","X......X","X......X","X......X","X......X","X......X","XXXXXXXX"],
  "var":   [".XX..XX.",".X....X.",".X....X.","X......X",".X....X.",".X....X.",".XX..XX.","........"],
  "out":   ["...XX...","..XXXX..",".X.XX.X.","...XX...","X..XX..X","X......X","X......X","XXXXXXXX"],
  "in":    ["...XX...","...XX...",".X.XX.X.","..XXXX..","X..XX..X","X......X","X......X","XXXXXXXX"],
  "merge": ["X......X","XX....XX",".XX..XX.","..XXXX..","...XX...","...XX...","...XX...","...XX..."],
  "drag":  [".XX..XX.",".XX..XX.","........",".XX..XX.",".XX..XX.","........",".XX..XX.",".XX..XX."],
  "plus":  ["...XX...","...XX...","...XX...","XXXXXXXX","XXXXXXXX","...XX...","...XX...","...XX..."],
  "chev":  ["........","........","XX....XX",".XX..XX.","..XXXX..","...XX...","........","........"],
  "chevr": ["..X.....","..XX....","...XX...","....XX..","....XX..","...XX...","..XX....","..X....."],
  "prompt":["........","XX......",".XX.....","..XX....",".XX.....","XX......","....XXXX","........"],
}
def svg(name, cls="ao-icon"):
  r="".join(f'<rect x="{x}" y="{y}" width="1" height="1"/>' for y,row in enumerate(ICONS[name]) for x,c in enumerate(row) if c=="X")
  return f'<svg class="{cls}" viewBox="0 0 8 8" aria-hidden="true">{r}</svg>'
def expand(s): return re.sub(r"\{\{i:(\w+)\}\}", lambda m: svg(m.group(1)), s)

def card(name, group, height, body, readme, subtitle=None):
  d=os.path.join(ROOT,"components",name); os.makedirs(d,exist_ok=True)
  sub=f' subtitle="{subtitle}"' if subtitle else ''
  html=f'<!-- @dsCard group="{group}" height={height}{sub} -->\n<!doctype html>\n<html lang="en">\n<head><meta charset="utf-8"><title>{name}</title></head>\n<body>\n{expand(body).strip()}\n</body>\n</html>\n'
  open(os.path.join(d,"preview.html"),"w").write(html)
  open(os.path.join(d,"README.md"),"w").write(readme.strip()+"\n")


# ---------- Navigation ----------
card("SideTab","Navigation",470,'''
<div style="display:grid;grid-template-columns:272px 1fr;gap:24px;padding:16px;align-items:start">
<nav class="ao-side" aria-label="Side tabs" style="width:272px">
 <div class="ao-st active need" style="--st-accent:var(--blue);--st-bg:var(--tint-blue)">{{i:group}}<span class="t">PROJ-482 audit export</span><span class="ao-stat need">Waiting</span>
  <span class="m"><span class="ao-strip"><i class="ao-cell wait"></i><i class="ao-cell run"></i><i class="ao-cell"></i></span>3 tabs · ~/work/api</span></div>
 <div class="ao-st-kids" style="--st-accent:var(--blue);--st-bg:var(--tint-blue)">
  <div class="ao-st-kid active"><span class="ao-dot need"></span>{{i:agent}}<span class="grow">claude · plan</span><span>⌘1</span></div>
  <div class="ao-st-kid"><span class="ao-dot run"></span>{{i:term}}<span class="grow">pnpm dev</span><span>⌘2</span></div>
  <div class="ao-st-kid"><span class="ao-dot idle"></span>{{i:term}}<span class="grow">zsh · ~/work/web</span><span>⌘3</span></div>
 </div>
 <div class="ao-st" style="--st-accent:var(--magenta);--st-bg:var(--tint-magenta)">{{i:group}}<span class="t">PROJ-477 rate limit</span><span class="ao-stat manual" style="--s-fg:var(--magenta);--s-edge:var(--magenta)">Review</span>
  <span class="m"><span class="ao-strip"><i class="ao-cell done"></i><i class="ao-cell"></i></span>2 tabs</span></div>
 <div class="ao-st" style="--st-accent:var(--green)">{{i:term}}<span class="t">dev servers</span><span class="ao-stat" style="--s-fg:var(--blue)">Running</span>
  <span class="m">docker compose up · 3h</span></div>
 <div class="ao-st" style="--st-accent:var(--red);--st-bg:var(--tint-red)">{{i:term}}<span class="t">infra hetzner</span><span class="ao-stat manual" style="--s-fg:var(--red);--s-edge:var(--red)">Blocked</span>
  <span class="m">waiting on DNS</span></div>
 <div class="ao-st">{{i:term}}<span class="t">scratch</span><span></span><span class="m">zsh · ~</span></div>
</nav>
<div class="small ao-dim" style="display:grid;gap:10px;max-width:460px">
 <div><b style="color:var(--fg)">Group, open:</b> a side tab with 3 tabs, its tabs listed under it. The strip has one cell per tab.</div>
 <div><b style="color:var(--fg)">Group, closed:</b> 2 tabs, a status the person set (the square before the label marks it as set by hand).</div>
 <div><b style="color:var(--fg)">Single:</b> one tab. The row shows that tab's command and age.</div>
 <div><b style="color:var(--fg)">Needs you:</b> the amber corner square and the amber label appear whenever a Claude session inside waits, even over a status set by hand.</div>
</div></div>''','''
# SideTab

One row in the sidebar. A side tab is a terminal window that lives inside the app: it holds one or more tabs, and one context (variables, pins, links, prompts) shared by all of them.

- One tab: a single row with the tab's command or cwd on the meta line.
- Two or more tabs: a group. The meta line shows a BlockStrip with one cell per tab and the tab count. The chevron or a click opens the group and lists its tabs as `.ao-st-kid` rows, with `⌘1` to `⌘9` hints.
- Accent: a 4px left edge in the hue the person picked (`--st-accent`). Background: one of the `tint-*` tokens (`--st-bg`). Both live in the StatusEditor. Defaults are `line-strong` and `panel`.
- Status: an `.ao-stat` label. Auto statuses come from the sessions (Running, Waiting, Done, Failed, Idle). A status set by hand gets `.manual` (a 4px square before the word) and stays until the person clears it.
- Needs you: `.need` on the row adds the amber corner square and flashes the row three times. Amber always shows when a Claude session waits, even when the person set their own status. The person can't hide it.
- Active side tab: a 2px `fg` inset frame. The sidebar is 272px wide and can collapse to a 48px rail of accent edges and dots.
- Drag a row onto another row to merge the two side tabs (see MergeDialog). Drag a tab from the TabBar into the sidebar to make it a side tab of its own (see DropZone).
- Classes: `.ao-side`, `.ao-st` (+ `.active`, `.need`, `.drop-into`), `.ao-st-kids`, `.ao-st-kid`, `.ao-stat` (+ `.need`, `.manual`), `.ao-dot` (+ `need run done fail idle`).
''')

card("TabBar","Navigation",200,'''
<div class="ao-col" style="gap:20px">
 <div class="ao-tabs" style="width:760px">
  <span class="ao-tab active" style="--tab-accent:var(--blue)"><span class="ao-dot need"></span>{{i:agent}}<span class="n">claude · plan</span><span class="x">{{i:cross}}</span></span>
  <span class="ao-tab"><span class="ao-dot run"></span><span class="n">pnpm dev</span><span class="x"></span></span>
  <span class="ao-tab"><span class="ao-dot idle"></span><span class="n">zsh · ~/work/web</span><span class="x"></span></span>
  <span class="ao-tab">{{i:doc}}<span class="n">review-plan.md</span><span class="dirty" aria-label="Unsaved"></span></span>
  <span class="ao-tab-split"><span>{{i:plus}}</span><span>{{i:chev}}</span></span>
 </div>
 <div class="ao-tabs" style="width:760px;position:relative">
  <span class="ao-tab active"><span class="ao-dot need"></span>{{i:agent}}<span class="n">claude · plan</span><span class="x">{{i:cross}}</span></span>
  <span class="ao-caret"></span>
  <span class="ao-tab"><span class="ao-dot idle"></span><span class="n">zsh · ~/work/web</span><span class="x"></span></span>
  <span class="ao-tab placeholder"><span class="n">pnpm dev</span></span>
  <span class="ao-tab ghost" style="position:absolute;left:250px;top:-18px;transform:rotate(-2deg)">{{i:drag}}<span class="ao-dot run"></span><span class="n">pnpm dev</span></span>
 </div>
 <div class="small ao-dim">Top: four tabs, the active one carries a Claude session waiting on you, the last is a text tab with unsaved changes. Bottom: "pnpm dev" dragged left, the amber caret marks where it lands.</div>
</div>''','''
# TabBar

The tabs of the active side tab, across the top like browser tabs. A tab holds a terminal, a text editor or a transcript, and can split into panes of any kind.

- A tab shows a status dot, an `agent` icon when a Claude session runs in it, the title (the running command, else `shell · cwd`), and a close button.
- The active tab joins the pane area below it: `panel` fill, no bottom edge, and a 2px top line in the side tab's accent.
- `+` or `⌘T` opens a new terminal tab running the default shell, not Claude. Claude starts by typing `claude` or from the palette.
- The `▾` half of the split button (`.ao-tab-split`) opens the new-tab menu: Terminal, Text, Open file. Text tabs carry the `doc` icon and a filled square (`.dirty`) in place of the close button while they hold unsaved changes.
- Drag inside the bar to reorder: the tab lifts as a `.ghost` with the notched frame, a dashed `.placeholder` holds its old slot, and the amber `.ao-caret` marks the drop point.
- Drag a tab into the sidebar to make a new side tab. Drag it outside the app window to open a new window holding a new side tab with that tab, like a browser.
- Titles truncate at the end. Over 10 tabs, the bar scrolls sideways and the sidebar row still lists them all.
- Classes: `.ao-tabs`, `.ao-tab` (+ `.active`, `.ghost`, `.placeholder`), `.ao-tab-new`, `.ao-tab-split`, `.ao-caret`, `.dirty`.
''')

# ---------- Layout ----------
card("Pane","Layout",320,'''
<div style="padding:16px">
<div class="ao-split" style="height:280px;grid-template-columns:1.3fr 1fr;grid-template-rows:1fr 1fr;border:2px solid var(--line-strong)">
 <div class="ao-tp focused" style="grid-row:span 2">
  <div class="ao-tp-head">{{i:agent}}<span>claude · ~/work/api</span><span class="grow"></span><span>⌘⌥←</span></div>
  <div class="ao-tp-body ao-term"><span class="p">❯</span> Write the plan for PROJ-482 in OpenSpec.
<span class="t">● Read</span> <span class="d">src/routes/audit.ts</span>
<span class="t">● Write</span> <span class="d">openspec/changes/add-audit-export/proposal.md</span>
Plan ready: <span class="u">claude.ai/artifact/9f2c…</span>
<span class="p">❯</span> <span class="ao-cursor"></span></div>
 </div>
 <div class="ao-tp">
  <div class="ao-tp-head">{{i:doc}}<span>transcript · claude · plan</span></div>
  <div class="ao-tp-body" style="font-size:12px;line-height:16px"><div class="ao-dim">assistant · 14:22</div>The export streams CSV in batches of 5,000.</div>
 </div>
 <div class="ao-tp">
  <div class="ao-tp-head">{{i:term}}<span>zsh · ~/work/api</span></div>
  <div class="ao-tp-body ao-term"><span class="p">$</span> git status -sb
<span class="d">## feature/PROJ-482-audit-export</span></div>
 </div>
</div></div>''','''
# Pane

One terminal inside a tab. A tab splits into panes like iTerm or tmux: `⌘D` splits right, `⌘⇧D` splits down, `⌘⌥` plus an arrow moves focus, dragging the 2px divider resizes.

- Kinds: `terminal` (xterm.js on a pty) and `transcript` (the rendered JSONL of a Claude session in a sibling pane, where text is easiest to select, pin and extract).
- Head, 22px: kind icon, process and cwd in `small faint`. The focused pane gets a 2px amber inset and an amber head. Exactly one pane has focus in a window.
- Terminal colors: prompt `green`, tool calls `cyan`, links `blue` underlined, secondary output `dim`. The xterm theme reads the same tokens.
- Dividers are the `line-strong` ground showing through a 2px grid gap. No pane has its own outer border.
- Classes: `.ao-split` (set the grid template inline), `.ao-tp` (+ `.focused`), `.ao-tp-head`, `.ao-tp-body`, `.ao-term`.
''')

card("DropZone","Layout",300,'''
<div style="display:grid;grid-template-columns:272px 1fr;gap:28px;padding:16px;align-items:start">
<nav class="ao-side" style="width:272px">
 <div class="ao-st" style="--st-accent:var(--blue)">{{i:group}}<span class="t">PROJ-482 audit export</span><span></span><span class="m">3 tabs</span></div>
 <div class="ao-drop-line"></div>
 <div class="ao-st drop-into" style="--st-accent:var(--magenta)">{{i:group}}<span class="t">PROJ-477 rate limit</span><span></span><span class="m">drop to add as a tab</span></div>
 <div class="ao-st">{{i:term}}<span class="t">scratch</span><span></span><span class="m">zsh · ~</span></div>
 <div class="ao-drop">{{i:plus}}New side tab from “pnpm dev”</div>
</nav>
<div style="display:grid;gap:16px">
 <div class="small ao-dim">Line between rows: a new side tab at that position. A framed row: the tab joins that side tab. The dashed zone at the end: a new side tab at the bottom.</div>
 <div style="display:flex;align-items:center;gap:12px"><span class="ao-tab ghost">{{i:drag}}<span class="ao-dot run"></span><span class="n">pnpm dev</span></span><span class="ao-chip wait">{{i:window}}<span class="ao-pixel">New window</span></span></div>
 <div class="small ao-dim">Outside the app window, the ghost carries a New window chip. Dropping there opens a window with one side tab holding the tab.</div>
</div></div>''','''
# DropZone

Where a dragged tab or side tab can land. Every target is amber while a drag is live and disappears on drop or Esc.

- `.ao-drop-line` between sidebar rows: the tab becomes a new side tab at that position.
- `.ao-st.drop-into` on a row: the tab moves into that side tab. Its context items move with it (see MergeDialog for side tab onto side tab).
- `.ao-drop` at the end of the sidebar: a new side tab at the bottom.
- The TabBar shows `.ao-caret` between tabs for reorder and for tabs arriving from another side tab or window.
- Outside every window, the ghost carries a `New window` chip. Dropping opens a new app window with a default side tab holding the tab, like a browser.
- Esc cancels a drag. Every drag has a palette equivalent ("Move tab to…", "Merge side tab into…") for keyboard use.
''')

# ---------- Frames ----------
card("PixelFrame","Frames",230,'''
<div class="ao-row" style="align-items:flex-start;gap:28px">
 <div class="ao-frame" style="width:300px">
  <div class="ao-frame-head">{{i:var}}<span class="ao-pixel">Context</span></div>
  <div class="ao-frame-body"><div>PROJ-482 audit export</div><div class="ao-dim small">5 variables · 3 pins · 2 links · shared by 3 tabs</div></div>
 </div>
 <div class="ao-frame amber" style="width:300px">
  <div class="ao-frame-head">{{i:bp}}<span class="ao-pixel">Waiting</span></div>
  <div class="ao-frame-body"><div>claude · plan asks to run a command</div><div class="ao-dim small">PROJ-482 · ⌘⇧A jumps to it</div></div>
 </div>
</div>''','''
# PixelFrame

The notched container: four 2px edges in `line-strong` with open corners, and a blurless `shadow-hard` offset 6px down and right.

Use it for anything that floats above the panes or asks for a decision: dialogs, the palette, the StatusEditor, pin cards, the drag ghost. Panes and side tab rows stay flat.

- Head: an 8x8 icon plus a `pixel-label` of eight characters or fewer. Body in `body`. Optional foot with buttons.
- `amber` variant only when the frame holds something waiting on the person. One amber frame on screen at a time.
- Never round the corners and never blur the shadow. The notch is the corner.
- Classes: `.ao-frame`, `.ao-frame.amber`, `.ao-frame-head`, `.ao-frame-body`, `.ao-frame-foot`.
''')

card("MergeDialog","Frames",420,'''
<div class="ao-row" style="justify-content:center">
<div class="ao-frame" style="width:600px" role="dialog" aria-labelledby="mg-t">
 <div class="ao-frame-head">{{i:merge}}<span id="mg-t" class="pixel-heading">Merge</span><span style="flex:1"></span><span class="small ao-dim">PROJ-477 into PROJ-482</span></div>
 <div class="ao-frame-body" style="display:grid;gap:12px;padding:0">
  <div style="padding:12px 16px 0" class="small ao-dim">2 tabs and their panes move. 6 context items merge, 1 name clashes.</div>
  <div class="ao-ctx">
   <div class="ao-ctx-row conflict"><span class="k"><span class="ao-var">{{branch}}</span><span class="ao-stat need">Clash</span></span><span class="small ao-dim">pick one</span>
    <span class="v" style="display:grid;gap:4px"><label style="display:flex;gap:8px;align-items:center"><span class="ao-sw on" style="--sw:var(--blue);width:12px;height:12px"></span>feature/PROJ-482-audit-export <span class="ao-dim">· keep</span></label><label style="display:flex;gap:8px;align-items:center"><span class="ao-sw" style="--sw:var(--panel-2);width:12px;height:12px"></span>feature/PROJ-477-rate-limit <span class="ao-dim">· rename to {{branch_477}}</span></label></span></div>
   <div class="ao-ctx-row"><span class="k"><span class="ao-var">{{pr_api}}</span></span><span class="small" style="color:var(--green)">{{i:plus}} new</span><span class="v"><a href="#">github.com/acme/api/pull/918</a></span></div>
   <div class="ao-ctx-row"><span class="k">{{i:pin}}<span>3 pins</span></span><span class="small" style="color:var(--green)">{{i:plus}} added</span></div>
  </div>
 </div>
 <div class="ao-frame-foot">
  <button class="ao-btn ghost">Cancel <span class="ao-key">Esc</span></button>
  <button class="ao-btn primary">{{i:merge}} Merge <span class="ao-key">↵</span></button>
 </div>
</div></div>''','''
# MergeDialog

Shown when a side tab is dropped onto another side tab. The tabs move, and the two contexts merge into one.

- Subject line: source into target. Summary: tabs that move, items that merge, names that clash.
- Items without a clash merge silently and show a green `new` or `added`.
- A clash is a variable name present in both with different values. It gets an amber edge and two choices: keep the target's value, or keep both and rename the source's (`{{branch_477}}` by default, editable).
- Pins, links and prompts never clash: they are lists and simply concatenate, each keeping its origin tab.
- No clash: the dialog is skipped and a `Merged` toast appears with Undo (`⌘Z`) for 10 s.
''')

# ---------- Status ----------
card("Chip","Status",120,'''
<div class="ao-col">
 <div style="display:flex;gap:16px;flex-wrap:wrap">
  <span class="ao-chip save">{{i:save}}<span class="ao-pixel">Saved</span></span>
  <span class="ao-chip wait">{{i:bp}}<span class="ao-pixel">Waiting</span></span>
  <span class="ao-chip run">{{i:play}}<span class="ao-pixel">Running</span></span>
  <span class="ao-chip done">{{i:check}}<span class="ao-pixel">Done</span></span>
  <span class="ao-chip fail">{{i:cross}}<span class="ao-pixel">Failed</span></span>
  <span class="ao-chip pin">{{i:pin}}<span class="ao-pixel">Pinned</span></span>
  <span class="ao-chip pin">{{i:var}}<span class="ao-pixel">Extract</span></span>
 </div>
 <div style="display:flex;gap:12px;flex-wrap:wrap"><span class="ao-chip tag small">backend</span><span class="ao-chip tag small">openspec</span><span class="ao-chip tag small">review</span></div>
</div>''','''
# Chip

A short state label: an 8x8 icon plus one or two words in `pixel-label`, inside a small notched frame.

- `save` and `wait` fill with `amber` and use `on-amber` text. A save chip appears for 2 s after a prompt, pin or variable is saved, then fades in two steps.
- `run` (blue), `done` (green), `fail` (red) and `pin` (magenta, also for extracted variables) color only the glyph and word.
- `tag` is the quiet variant for prompt tags and filters, in `small` Mono.
- Always pair a status color with its icon and word.
- Side tab statuses use the smaller StatusLabel (`.ao-stat`, see SideTab), whose colors the person can change.
- Classes: `.ao-chip` plus one of `save wait run done fail pin tag`.
''')

card("StatusEditor","Status",420,'''
<div class="ao-row" style="align-items:flex-start;gap:28px">
<div class="ao-frame" style="width:360px" role="dialog" aria-labelledby="se-t">
 <div class="ao-frame-head">{{i:dot}}<span id="se-t" class="ao-pixel">Status</span><span style="flex:1"></span><span class="small ao-dim">PROJ-477 rate limit</span></div>
 <div class="ao-frame-body" style="display:grid;gap:14px">
  <label class="ao-field"><span class="ao-pixel ao-dim">Label</span><input class="ao-input" value="Review"></label>
  <div class="small ao-dim">auto would say: Done · 1 of 2 tabs <a href="#">back to auto</a></div>
  <div style="display:flex;gap:8px;flex-wrap:wrap"><span class="ao-stat manual" style="--s-fg:var(--magenta);--s-edge:var(--magenta)">Review</span><span class="ao-stat manual" style="--s-fg:var(--red);--s-edge:var(--red)">Blocked</span><span class="ao-stat manual" style="--s-fg:var(--cyan);--s-edge:var(--cyan)">Testing</span><span class="ao-stat manual">Parked</span></div>
  <div class="ao-field"><span class="ao-pixel ao-dim">Accent</span><div class="ao-swatches"><span class="ao-sw none"></span><span class="ao-sw" style="--sw:var(--blue)"></span><span class="ao-sw" style="--sw:var(--cyan)"></span><span class="ao-sw" style="--sw:var(--green)"></span><span class="ao-sw on" style="--sw:var(--magenta)"></span><span class="ao-sw" style="--sw:var(--red)"></span><span class="ao-sw" style="--sw:var(--fg)"></span></div></div>
  <div class="ao-field"><span class="ao-pixel ao-dim">Background</span><div class="ao-swatches"><span class="ao-sw none"></span><span class="ao-sw" style="--sw:var(--tint-blue);--swe:var(--blue)"></span><span class="ao-sw" style="--sw:var(--tint-cyan);--swe:var(--cyan)"></span><span class="ao-sw" style="--sw:var(--tint-green);--swe:var(--green)"></span><span class="ao-sw on" style="--sw:var(--tint-magenta);--swe:var(--magenta)"></span><span class="ao-sw" style="--sw:var(--tint-red);--swe:var(--red)"></span></div></div>
 </div>
</div>
<div style="display:grid;gap:8px;width:272px"><span class="small ao-dim">preview</span>
 <div class="ao-st" style="--st-accent:var(--magenta);--st-bg:var(--tint-magenta)">{{i:group}}<span class="t">PROJ-477 rate limit</span><span class="ao-stat manual" style="--s-fg:var(--magenta);--s-edge:var(--magenta)">Review</span><span class="m">2 tabs</span></div>
</div></div>''','''
# StatusEditor

Opens from a side tab's or tab's context menu, or `⌘⇧S`. Every status, accent and background is the person's to change. The defaults are generated.

- Label: free text, uppercase in the pixel face, 12 characters at most. Below it, what the auto status would say, and "back to auto".
- Presets: the person's own saved statuses (Review, Blocked, Testing, Parked), one click to apply. A new label typed here can be saved as a preset.
- Accent: none, or one of six hues (`blue cyan green magenta red fg`). Background: none, or one of five `tint-*`; each swatch is edged in its hue so the dark tints stay tellable apart. Amber is not offered: it stays the one signal for "needs you".
- Applies to a side tab or to one tab. A tab's accent shows as the active tab's top line and its dot.
- Sorting: the sidebar can sort by background, then status, then name. Manual order is the default.
''')

card("BlockStrip","Status",150,'''
<div class="ao-col" style="gap:14px">
 <div style="display:flex;align-items:center;gap:16px"><span class="body-strong" style="width:200px">PROJ-482 audit export</span>
  <span class="ao-strip lg" aria-label="3 tabs: 1 waiting, 1 running, 1 idle"><i class="ao-cell wait"></i><i class="ao-cell run"></i><i class="ao-cell"></i></span>
  <span class="ao-chip wait">{{i:bp}}<span class="ao-pixel">Waiting</span></span></div>
 <div style="display:flex;align-items:center;gap:16px"><span class="body-strong" style="width:200px">PROJ-477 rate limit</span>
  <span class="ao-strip"><i class="ao-cell done"></i><i class="ao-cell fail"></i></span>
  <span class="ao-dim small">compact strip, for sidebar rows</span></div>
</div>''','''
# BlockStrip

A side tab's tabs as a row of blocks, one per tab, left to right in tab order. It shows on group rows in the sidebar, in palette results and in the MergeDialog.

- Cell states: `faint` idle shell, `blue` running (blinks in two steps), `amber` a Claude session waiting on you, `green` last command or turn done, `red` failed.
- Sizes: `lg` (18x22) in dialogs, default (10x14) in sidebar rows.
- Give the strip an `aria-label` that counts tabs by state ("3 tabs: 1 waiting, 1 running, 1 idle"). Hover names the tab.
- Classes: `.ao-strip`, `.ao-strip.lg`, `.ao-cell` plus `done run wait fail`.
''')

# ---------- Actions ----------
card("Keycap","Actions",110,'''
<div class="ao-row" style="gap:22px">
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">K</span> palette</span>
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">T</span> new tab</span>
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">N</span> new side tab</span>
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">D</span> split</span>
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">I</span> insert</span>
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">E</span> extract</span>
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">⇧</span><span class="ao-key">P</span> pin</span>
 <span class="ao-hint"><span class="ao-key">⌘</span><span class="ao-key">1</span>–<span class="ao-key">9</span> tab</span>
</div>''','''
# Keycap

One key drawn as a pixel cap: 2px `line-strong` edges with a 4px bottom edge for depth, the label in `pixel-label`.

Put one next to every action that has a key: buttons, palette rows, the selection toolbar, empty states.

- The app is a terminal, so plain keys always go to the focused pane. App actions use `⌘` chords, which shells and Claude don't read on macOS: `⌘K` palette, `⌘T` tab, `⌘N` side tab, `⌘D` / `⌘⇧D` split, `⌘W` close, `⌘I` insert a prompt or variable, `⌘E` extract the selection, `⌘⇧P` pin it, `⌘1`–`⌘9` switch tab, `⌃⌘1`–`⌃⌘9` switch side tab, `⌘⇧A` jump to the oldest session waiting on you.
- Inside a primary button the cap inverts to `amber-edge` on `on-amber`.
- Classes: `.ao-key`; wrap with a label in `.ao-hint`.
''')

card("PixelButton","Actions",110,'''
<div class="ao-row" style="gap:20px">
 <button class="ao-btn primary">{{i:in}} Insert <span class="ao-key">↵</span></button>
 <button class="ao-btn">{{i:save}} Save as prompt</button>
 <button class="ao-btn">{{i:pin}} Pin <span class="ao-key">⌘⇧P</span></button>
 <button class="ao-btn ghost">Cancel</button>
 <button class="ao-btn danger">{{i:cross}} Close tab</button>
 <button class="ao-btn" disabled>Merge</button>
</div>''','''
# PixelButton

Square button with a notched frame and a 3px hard shadow. Pressing drops the face 3px onto its shadow.

- `primary` (amber fill, `on-amber` text, `amber-edge` notch): the one action a dialog is for. At most one per view.
- default: neutral actions on `panel-2`.
- `ghost`: dismiss and cancel, no frame.
- `danger`: destructive actions (close a tab with a running process, kill a session), red label only.
- Labels start with a verb in sentence case, Geist Mono 600. Add the icon when it helps scanning and a Keycap when a key exists.
''')

card("SelectionToolbar","Actions",210,'''
<div class="ao-col" style="gap:8px;max-width:720px">
 <div class="ao-term" style="padding:12px;background:var(--panel);white-space:pre-wrap">Opened the draft PR: <span class="ao-sel">https://github.com/acme/api/pull/918</span>
Branch feature/PROJ-482-audit-export is pushed.</div>
 <div style="padding-left:150px;margin-top:-6px"><span class="ao-seltool" role="toolbar" aria-label="Selection">
  <button class="on">{{i:var}}Extract<span class="ao-key">⌘E</span></button>
  <button>{{i:pin}}Pin<span class="ao-key">⌘⇧P</span></button>
  <span class="sep"></span>
  <button>{{i:in}}Send to…</button>
  <button>Copy<span class="ao-key">⌘C</span></button>
 </span></div>
 <div style="padding-left:150px;display:flex;gap:8px;align-items:center"><span class="ao-var">{{</span><input class="ao-input" value="pr_api" style="width:120px;height:24px;padding:0 6px"><span class="ao-var">}}</span><span class="small ao-dim">suggested from the URL · ↵ saves to PROJ-482</span></div>
</div>''','''
# SelectionToolbar

Floats above text selected in a terminal or transcript pane. It is how text leaves a session and lands in the side tab's context.

- Extract `⌘E`: saves the selection as a variable. The name is suggested from the content (`pr_api` for a GitHub PR URL in the api repo, `branch` for a branch name) and editable inline. `↵` saves.
- Pin `⌘⇧P`: saves the selection as a pin, linked back to the transcript entry when the pane is a Claude session.
- Send to…: pastes the selection into another tab's prompt, as bracketed paste, without pressing Enter.
- Copy `⌘C`: plain copy.
- Selection paints in `selection`. The toolbar sits 6px above the selection end and never covers the selected text.
''')

# ---------- Feedback ----------
card("Toast","Feedback",290,'''
<div class="ao-col" style="gap:14px">
 <div class="ao-toast wait">{{i:bp}}<span class="ao-pixel">Waiting</span><span class="grow">PROJ-482 · claude · plan asks to run a command</span><span class="ao-key">⌘⇧A</span></div>
 <div class="ao-toast done">{{i:check}}<span class="ao-pixel">Done</span><span class="grow">PROJ-477 · claude finished its turn</span></div>
 <div class="ao-toast pin">{{i:var}}<span class="ao-pixel">Saved</span><span class="grow">{{pr_api}} in PROJ-482</span><span class="ao-key">⌘Z</span></div>
 <div class="ao-toast fail">{{i:cross}}<span class="ao-pixel">Failed</span><span class="grow">dev servers · pnpm dev exited 1</span></div>
</div>''','''
# Toast

A typed notice in the bottom-right corner: icon, a `pixel-label` type word, one line of Mono, and an optional Keycap for the obvious next action.

- Types: `wait` (amber frame, stays until handled), `done`, `pin` (pins and extracted variables), `fail`. Only `wait` uses an amber frame.
- `done` and `pin` leave after 4 s. `fail` and `wait` stay until acted on or dismissed.
- Name the side tab first, then the tab. One line; truncate the middle of long paths.
- When the app is in the background, `wait` and `fail` also post a macOS notification. The chime is off by default.
''')

card("StatusLine","Feedback",80,'''
<div style="padding:16px 0">
<div class="ao-status">
 <span class="need">{{i:bp}}<span class="ao-pixel">1 needs you</span></span>
 <span class="seg" style="color:var(--blue)">{{i:play}} 3 running</span>
 <span class="seg">{{i:group}} 5 side tabs · 9 tabs</span>
 <span class="seg">{{i:agent}} claude · plan · ~/work/api</span>
 <span class="spacer"></span>
 <span class="seg"><span class="ao-key">⌘</span><span class="ao-key">K</span></span>
 <span class="seg">window 1 of 2</span>
</div></div>''','''
# StatusLine

The 24px bar across the bottom of every window, like a multiplexer's.

- Left to right: the amber "needs you" segment (only when a session waits; clicking it jumps there), running count, side tabs and tabs, the focused pane's process and cwd, then the palette hint and which window this is.
- Text is `small` in `dim`; segments color only their icon and count.
- The amber segment is the only fill.
''')

# ---------- Content ----------
card("ContextRow","Content",440,'''
<div style="padding:16px;width:320px">
<section class="ao-pane" aria-label="Context">
 <div class="ao-pane-head">{{i:var}}<span class="ao-pixel">Variables</span><span class="grow"></span><span class="small ao-dim">⌘I inserts</span></div>
 <div class="ao-ctx">
  <div class="ao-ctx-row"><span class="k"><span class="ao-var">{{ticket}}</span></span><span class="small ao-dim">text</span><span class="v">PROJ-482 · Add audit export endpoint…</span><span class="src">pasted · 412 words · 2 images</span></div>
  <div class="ao-ctx-row"><span class="k"><span class="ao-var">{{branch}}</span></span><span class="small ao-dim">⌘E</span><span class="v">feature/PROJ-482-audit-export</span><span class="src">extracted · claude · plan · 14:20</span></div>
  <div class="ao-ctx-row"><span class="k"><span class="ao-var">{{pr_api}}</span><span class="ao-chip pin" style="height:16px">{{i:link}}<span class="ao-pixel">Auto</span></span></span><span class="small ao-dim">link</span><span class="v"><a href="#">github.com/acme/api/pull/918</a></span><span class="src">auto · PR URL in claude · plan · 14:31</span></div>
  <div class="ao-ctx-row session"><span class="k"><span class="ao-var">{{from_date}}</span><span class="ao-store">Session</span></span><span class="small ao-dim">text</span><span class="v">2026-10-01</span><span class="src">not stored · gone on reboot</span></div>
  <div class="ao-ctx-row secret"><span class="k"><span class="ao-var">{{api_token}}</span><span class="ao-store secret">Secret</span></span><span class="small ao-dim">text</span><span class="v">••••••••••••</span><span class="src">inserts as $API_TOKEN · 52m left</span></div>
 </div>
</section></div>''','''
# ContextRow

One item a side tab keeps: a variable, a link or a block of text. All tabs in the side tab share the list. It renders in the context panel on the right of the window.

- Key: the variable name as an `.ao-var` chip. Kind at the right (`text`, `link`, `path`, `json`).
- Value on one line, truncated, links in `link`. Source on the last line: pasted, typed, extracted (with the tab and time), or auto (from an extraction rule such as "GitHub PR URL").
- Click copies the value. `⌘I` in any pane inserts it at the cursor. In a prompt template, `{{name}}` resolves from this list.
- Storage: saved by default. `.session` (dashed outline, `Session` tag) lives in memory only and never reaches disk; it survives an app restart and ends on reboot or when its side tab closes. `.secret` (red dashed outline, `Secret` tag) is session only, masked, left out of pins, copies, merges and exports, and can expire. `⌘I` on a secret types ` export API_TOKEN=…` into the shell and inserts `$API_TOKEN`, so Claude never sees the value.
- A row with `.conflict` (amber edge) only appears in the MergeDialog.
- When a tab moves to another side tab, items it created move with it. Items typed or pasted by hand stay.
''')

card("PinCard","Content",230,'''
<div class="ao-row" style="align-items:flex-start;gap:24px">
 <div class="ao-pin">
  <div class="ao-pin-head">{{i:doc}}<span class="ao-pixel">Artifact</span><span class="grow"></span><span class="ao-key">1</span></div>
  <div class="ao-pin-body"><div class="body-strong">Plan · audit export</div><a href="#">claude.ai/artifact/9f2c…</a></div>
  <div class="ao-pin-meta"><span>auto · claude · plan</span><span>14:06</span></div>
 </div>
 <div class="ao-pin">
  <div class="ao-pin-head">{{i:pin}}<span class="ao-pixel">Excerpt</span><span class="grow"></span><span class="ao-key">2</span></div>
  <div class="ao-pin-body"><q>Cursor pagination on id, batches of 5,000, one transaction per batch.</q></div>
  <div class="ao-pin-meta"><span>claude · plan · assistant</span><span>14:04</span></div>
 </div>
</div>''','''
# PinCard

One pinned item in a side tab's context: a URL, a whole Claude message, any text selected inside a message, or a block of terminal output.

- A selection pin keeps the entry id, the character range and the quoted text with a few words around it, so it finds its place again in the transcript.

- Head: kind icon, a `pixel-label` kind (`Artifact`, `PR`, `Excerpt`, `Output`, `Url`), and a number. `⌥1` to `⌥9` copies that pin from anywhere in the side tab.
- Body: a `body-strong` title, then the link in `link` or the excerpt prefixed with a magenta `>`.
- Meta: origin (`auto` for extraction rules, else the tab and speaker) and time. A pin from a Claude session opens its transcript at that entry.
- Pins render into a prompt through `{{pins}}`.
''')

# ---------- Forms ----------
card("CommandRow","Navigation",250,'''
<nav class="ao-side" aria-label="Commands" style="width:272px;margin:16px">
 <div style="display:flex;align-items:center;gap:8px;height:30px;padding:0 12px"><span class="ao-pixel ao-dim">Commands</span><span class="small ao-dim">PROJ-482</span><span style="flex:1"></span>{{i:plus}}</div>
 <div class="ao-cmds">
  <div class="ao-cmd">{{i:play}}<span class="c">pnpm test audit</span><span class="to">new tab</span></div>
  <div class="ao-cmd">{{i:play}}<span class="c">gh pr checks 918</span><span class="to">pane</span></div>
 </div>
 <div style="display:flex;align-items:center;height:24px;padding:0 12px"><span class="ao-pixel ao-dim">Global</span></div>
 <div class="ao-cmds">
  <div class="ao-cmd">{{i:play}}<span class="c">docker compose ps</span><span class="to">pane</span></div>
  <div class="ao-cmd">{{i:play}}<span class="c">claude --resume</span><span class="to">new tab</span></div>
 </div>
</nav>''','''
# CommandRow

A saved shell command in the sidebar, under the side tabs. The side tab's own commands come first, then global ones from the library.

- Text may hold `{{variables}}` from the side tab's context; they resolve when the command runs.
- The hint on the right is the default target: `new tab`, `pane` (the focused pane) or `side tab`. Click runs there; `⌥`-click asks.
- Run sends Enter. Insert (`⌘I` on the row) pastes the command without Enter.
- A launcher side tab is a side tab made of commands: it runs them in its tabs when it opens or after a reboot, shows a restart action, and takes its status from those processes (`2/2 up`).
''')

card("ContextEdge","Layout",340,'''
<div style="position:relative;display:grid;grid-template-columns:minmax(0,1fr) 12px;height:300px;margin:16px;border:2px solid var(--line-strong);background:var(--bg)">
 <div class="ao-term" style="padding:12px;color:var(--dim)"><span style="color:var(--green)">$</span> pnpm dev<br>ready on :4801</div>
 <div class="ao-edge hot"><span class="pip need"></span><span class="pip"></span><span class="ao-pixel">Context</span></div>
 <aside class="ao-ctx-over" style="right:12px;width:220px">
  <div style="display:flex;align-items:center;gap:8px;height:30px;padding:0 12px;border-bottom:2px solid var(--line-strong)"><span class="ao-pixel ao-dim">Context</span><span style="flex:1"></span><span class="small ao-dim">⌘\\ pin</span></div>
  <div class="ao-ctx ao-scroll">
   <div class="ao-ctx-row"><span class="k"><span class="ao-var">{{branch}}</span></span><span class="v">feature/PROJ-482-audit-export</span></div>
   <div class="ao-ctx-row"><span class="k"><span class="ao-var">{{plan}}</span></span><span class="v"><a href="#">claude.ai/artifact/9f2c41…</a></span></div>
  </div>
 </aside>
</div>''','''
# ContextEdge

What stays of the context panel when it is set to auto-hide: a 12px strip on the right edge of the window.

- Pips show what the panel holds: magenta for new context items, amber when a pin or variable came from a session that waits.
- Hover the edge for 150 ms (`.hot`) and the panel slides in as `.ao-ctx-over`, on top of the panes with a hard shadow. Terminals keep their size, so no shell or Claude session redraws.
- The panel slides out when the pointer leaves it. `⌘\\` switches between pinned and auto-hide.
''')

card("TextPane","Layout",420,'''
<div style="position:relative;margin:16px;width:640px">
<section class="ao-tp focused" aria-label="Text editor" style="height:380px">
 <div class="ao-tp-head">{{i:doc}}<span>review-plan.md</span><span>~/work/api/prompts</span><span class="grow"></span><span>Markdown</span></div>
 <div class="ao-ed" style="flex:1">
  <div class="ao-ed-l"><span class="n">1</span><span class="c"><span class="mk"># </span><span class="h1">Review the plan</span></span></div>
  <div class="ao-ed-l"><span class="n">2</span><span class="c"></span></div>
  <div class="ao-ed-l"><span class="n">3</span><span class="c">Read the plan at <span class="ao-vchip">{{plan}}</span> for branch <span class="ao-vchip">{{branch}}</span>.</span></div>
  <div class="ao-ed-l"><span class="n">4</span><span class="c">Ticket: <span class="ao-vchip unknown">{{ticket_url}}</span></span></div>
  <div class="ao-ed-l"><span class="n">5</span><span class="c">Call the API with <span class="ao-vchip secret">$API_TOKEN</span>.</span></div>
  <div class="ao-ed-l cur"><span class="n">6</span><span class="c"><span class="li">- </span>Check <span class="code">`openspec validate`</span> passes<span class="ao-ed-caret"></span></span></div>
 </div>
 <div class="ao-ed-bar"><span class="dirty">● unsaved</span><span>Ln 6, Col 34</span><span class="grow"></span><span>UTF-8 · LF</span><span>⌘⇧↵ send to pane</span></div>
</section></div>''','''
# TextPane

A pane that edits one document: a file, an Untitled buffer, a context variable, a prompt version, or an editor request from another program. CodeMirror 6 underneath; the text on disk stays exactly as typed, and color comes from decorations.

- Lines (`.ao-ed-l`) with a gutter number and the text. `.cur` marks the cursor line.
- Markdown: markers in `faint`, the first heading in `blue`, code in `cyan`, list bullets in `magenta`.
- `{{variable}}` from the side tab's context renders as `.ao-vchip`. An unknown name gets `.unknown` (red wavy underline). A secret only ever appears as `$NAME` with `.secret`.
- `.ao-complete` lists context variables after `{{`, value previews dim, secrets offered as `$NAME`.
- The pane bar (`.ao-ed-bar`): unsaved state, line and column, encoding and line endings, the send action.
- `.ao-bar-info` (blue) says the file changed on disk, with Reload and Keep mine. `.ao-bar-wait` (amber) says a program waits for this text: `⌘↵` sends it back.
- `⌘S` saves; an Untitled buffer asks where. Unsaved text survives an app restart.
''')

card("NewTabMenu","Navigation",300,'''
<div style="position:relative;height:270px;margin:16px">
 <div class="ao-tabs" style="width:560px">
  <span class="ao-tab active" style="--tab-accent:var(--blue)"><span class="ao-dot need"></span>{{i:agent}}<span class="n">claude · plan</span><span class="x">{{i:cross}}</span></span>
  <span class="ao-tab-split open"><span>{{i:plus}}</span><span>{{i:chev}}</span></span>
 </div>
 <div class="ao-menu" style="left:150px;top:34px">
  <div class="mi">{{i:term}}<span>Terminal</span><span><span class="ao-key">⌘</span><span class="ao-key">T</span></span></div>
  <div class="mi on">{{i:doc}}<span>Text</span><span><span class="ao-key">⌥</span><span class="ao-key">⌘</span><span class="ao-key">T</span></span></div>
  <div class="mi">{{i:out}}<span>Open file…</span><span><span class="ao-key">⌘</span><span class="ao-key">O</span></span></div>
  <div class="sep"></div>
  <div class="mi">{{i:agent}}<span>Terminal with claude</span><span></span></div>
  <div class="hint">Hold ⌥ to open beside the focused pane.</div>
 </div>
</div>''','''
# NewTabMenu

The split new-tab button. `+` opens a terminal tab with the login shell; `▾` opens this menu.

- Terminal `⌘T`, Text `⌥⌘T` (an Untitled buffer), Open file `⌘O` (search in the side tab's folder, `⌘O` again for the macOS dialog).
- Holding `⌥` with any of them opens a split pane beside the focused pane instead of a tab.
- Other ways in: `⌘`-click a path in terminal output, `terminescu edit <file>` in a shell, a file dropped from Finder, a variable or prompt clicked in the context panel.
- Every shortcut here is editable in Settings › Keys.
''')

card("FileRow","Content",200,'''
<section class="ao-pane" aria-label="Files" style="width:300px;margin:16px">
 <div class="ao-pane-head">{{i:doc}}<span class="ao-pixel">Files</span><span class="grow"></span><span class="small ao-dim">edited by claude</span></div>
 <div class="ao-file"><span class="s a">A</span><span class="p">openspec/changes/add-audit-export/proposal.md</span><span class="t">14:08</span></div>
 <div class="ao-file"><span class="s a">A</span><span class="p">openspec/changes/add-audit-export/design.md</span><span class="t">14:12</span></div>
 <div class="ao-file"><span class="s m">M</span><span class="p">src/routes/audit.ts</span><span class="t">14:31</span></div>
</section>''','''
# FileRow

A file a Claude session in this side tab wrote (`A`, green) or edited (`M`, blue), caught by the `PostToolUse` hook. One click opens it in a text tab.

- Paths are relative to the side tab's folder and truncate from the left, so the file name stays visible.
- Time is the last edit. The list is a shortcut, not a file tree.
''')

card("Field","Forms",184,'''
<div class="ao-col" style="max-width:560px">
 <label class="ao-field"><span class="ao-pixel ao-dim">Prompt name</span><input class="ao-input" value="review-plan"></label>
 <label class="ao-field"><span class="ao-pixel ao-dim">Body</span><div class="ao-input" style="min-height:52px">Review the plan for <span class="ao-var">{{ticket}}</span> on <span class="ao-var">{{branch}}</span>. Pinned so far: <span class="ao-var">{{pins}}</span> <span class="ao-var" style="text-decoration:underline wavy var(--red)">{{owner}}</span></div></label>
</div>''','''
# Field

Label above input. The label is a `pixel-label` in `dim`; the input is Mono on `panel-2` with a 2px `line-strong` border and `radius-1`, turning `amber` on focus.

- Prompt variables render inline as `{{name}}` chips in `magenta` on `bg`. A variable the current side tab doesn't have gets a red wavy underline; inserting the prompt then asks for its value.
- The prompt editor is a Tiptap field in this style; pasted images sit in the flow at their own size, framed by a 2px `line` border.
''')

card("Icons","Iconography",160,'''
<div class="ao-row" style="gap:20px">'''+"".join(f'<span style="display:inline-flex;flex-direction:column;align-items:center;gap:6px;width:44px">{{{{i:{k}}}}}<span class="small ao-dim">{k}</span></span>' for k in ICONS)+'''</div>''','''
# Icons

The app's own icon set: 8x8 bitmaps drawn as SVG squares with `shape-rendering: crispEdges`, filled with `currentColor`. Shown at 16px (2x) by default, 8px (1x) in dense rows.

Set: '''+", ".join(ICONS)+'''.

- In markup, `<i class="ao-i ao-i-<name>"></i>` paints the same bitmap as a CSS mask.
- `agent` marks a tab or pane running a Claude session. `group` a side tab with several tabs, `term` one with a single shell.
- Draw new icons on the same 8x8 grid, one-pixel strokes, no anti-aliasing. Never mix in a stroke icon library. No emoji, no logos.
''')

# icon mask classes appended to bundle.css
import urllib.parse
def mask(name):
  r="".join(f"<rect x='{x}' y='{y}' width='1' height='1'/>" for y,row in enumerate(ICONS[name]) for x,c in enumerate(row) if c=="X")
  svgs=f"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 8 8' shape-rendering='crispEdges'>{r}</svg>"
  return urllib.parse.quote(svgs, safe="=:/' ")
bp=os.path.join(ROOT,"components","bundle.css"); b=open(bp).read()
b=b[:b.index('/* Icons by name')]
b+='/* Icons by name: <i class="ao-i ao-i-pin"></i>. 8x8 bitmaps as masks, painted in currentColor. */\n'
b+='.ao-i { display: inline-block; width: 16px; height: 16px; flex: none; background: currentColor; -webkit-mask: var(--ao-i) center / 100% 100% no-repeat; mask: var(--ao-i) center / 100% 100% no-repeat; image-rendering: pixelated; vertical-align: -3px; }\n.ao-i.x1 { width: 8px; height: 8px; vertical-align: 0; }\n'
for k in ICONS: b+=f'.ao-i-{k} {{ --ao-i: url("data:image/svg+xml,{mask(k)}"); }}\n'
open(bp,'w').write(b)
print("ok", sorted(os.listdir(os.path.join(ROOT,"components"))))
