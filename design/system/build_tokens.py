import json, sys
C=[
("bg","#111318","Page ground behind every pane. Text on it: fg, dim, and the signal hues."),
("panel","#171a21","Terminal panes, side tab rows, dialog bodies."),
("panel-2","#1d2029","Pane headers, the status line, hovered rows, input fields."),
("line","#2a2e38","Hairlines between rows and around inputs. Decorative only, never the sole marker of state."),
("line-strong","#414756","Pane borders, the notched pixel frame on neutral panels, keycap edges (3:1 on bg)."),
("fg","#d7dae0","Primary text on bg, panel and panel-2 (13:1 on bg)."),
("dim","#8b91a0","Secondary text, timestamps, metadata, on bg, panel and panel-2 (6:1 on bg)."),
("faint","#5c6270","Disabled glyphs, empty block-strip cells, gutters. Never body text (3:1 on bg)."),
("shadow","#05060a","The hard 6px pixel shadow under frames, chips and buttons."),
("amber","#e5a33a","The one accent: needs you. A Claude session waiting on a permission or a question, the focus ring, drop targets, the primary action. Text on bg and panel (8:1)."),
("on-amber","#1a1205","Text and icons on an amber fill (12:1)."),
("amber-edge","#f3c470","The notched outline around amber chips and the primary button."),
("green","#57ab5a","Done: a Claude turn finished, a command exited 0. Text on bg and panel (6:1). Always paired with a check glyph or the word."),
("red","#e5534b","Failed: a command exited non-zero, a session crashed. Text on bg and panel (5:1). Always paired with a cross glyph or the word."),
("blue","#539bf5","Running: a command or a Claude turn in progress. Links and artifact URLs. Text on bg and panel (6:1)."),
("magenta","#b083f0","Pins, variables and pinned excerpts. Text on bg and panel (6:1)."),
("cyan","#39c5cf","Agent output in the transcript, tool calls. Text on bg and panel (9:1)."),
("tint-blue","#212f43","Side tab and tab background a person picks to sort work. Blue mixed 16% into panel. fg holds 11:1 on it."),
("tint-cyan","#1c353d","Picked background, cyan 16% into panel."),
("tint-green","#21312a","Picked background, green 16% into panel."),
("tint-magenta","#2f2b42","Picked background, magenta 16% into panel."),
("tint-red","#382328","Picked background, red 16% into panel."),
("tint-amber","#383025","Drop targets while dragging a tab, and the background of a side tab that needs you. Not offered as a picked background."),
("selection","#294161","Text selected in a terminal or transcript pane, before Pin or Extract."),
("focus","{amber}","Keyboard focus ring: 2px solid, offset 2px. An alias of amber."),
("link","{blue}","Links and artifact URLs. An alias of blue."),
("ansi-0","#2a2e38","Terminal black. Lighter than bg so programs that paint black stay visible."),
("ansi-1","{red}","Terminal red."),
("ansi-2","{green}","Terminal green."),
("ansi-3","#d4b84a","Terminal yellow. Its own hue: amber stays the needs-you signal (D16)."),
("ansi-4","{blue}","Terminal blue."),
("ansi-5","{magenta}","Terminal magenta."),
("ansi-6","{cyan}","Terminal cyan."),
("ansi-7","#b9bdc6","Terminal white, a step below fg."),
("ansi-8","#6e7585","Terminal bright black: shell autosuggestions and comments (4:1 on bg)."),
("ansi-9","#f47067","Terminal bright red."),
("ansi-10","#6bc46d","Terminal bright green."),
("ansi-11","#e8cc6a","Terminal bright yellow."),
("ansi-12","#6cb6ff","Terminal bright blue."),
("ansi-13","#dcbdfb","Terminal bright magenta."),
("ansi-14","#56d4dd","Terminal bright cyan."),
("ansi-15","#f0f2f5","Terminal bright white."),
]
tokens={"name":"Terminescu Workbench","version":1,
"color":{"themes":[{"id":"dark","name":"Dark"}],"tokens":[{"name":n,"value":v,"usage":u} for n,v,u in C]},
"type":{"fonts":[],"families":{
  "mono":"\"Geist Mono\", ui-monospace, \"SF Mono\", Menlo, monospace",
  "pixel":"Silkscreen, \"Geist Mono\", ui-monospace, monospace"},
 "groups":[
  {"name":"Mono","family":"mono","note":"Geist Mono carries every readable word: terminal, transcript, forms, tables.","styles":[
   {"name":"title","fontSize":"18px","lineHeight":"24px","fontWeight":600,"sample":"PROJ-482 · audit export","usage":"Dialog subjects and the prompt library preview title."},
   {"name":"body","fontSize":"13px","lineHeight":"20px","fontWeight":400,"sample":"Plan written to openspec/changes/add-export/","usage":"Default for everything: transcript, forms, template bodies."},
   {"name":"body-strong","fontSize":"13px","lineHeight":"20px","fontWeight":600,"sample":"PROJ-482 · 3 tabs","usage":"Side tab names, the active tab, numbers that need weight."},
   {"name":"small","fontSize":"12px","lineHeight":"16px","fontWeight":400,"sample":"14:02 · session 3f9a · 41k tokens","usage":"Metadata, timestamps, the status line."},
   {"name":"term","fontSize":"13px","lineHeight":"18px","fontWeight":400,"sample":"$ claude --resume 3f9a","usage":"xterm.js panes. Tighter leading than body so TUIs fit."}]},
  {"name":"Pixel","family":"pixel","note":"Silkscreen is trim. Labels of eight characters or fewer, always uppercase, never sentences.","styles":[
   {"name":"pixel-label","fontSize":"8px","lineHeight":"12px","fontWeight":400,"letterSpacing":"0.04em","sample":"SAVED","usage":"Chips, keycaps, pane tabs."},
   {"name":"pixel-heading","fontSize":"16px","lineHeight":"20px","fontWeight":400,"letterSpacing":"0.02em","sample":"MERGE","usage":"Dialog titles and empty-state headings."},
   {"name":"pixel-display","fontSize":"32px","lineHeight":"32px","fontWeight":400,"sample":"TABS","usage":"The cover and the app's boot screen only."}]}]},
"spacing":{"note":"4px base. Pixel ornaments snap to 2px.","tokens":[
  {"name":"px","value":"2px","usage":"One pixel of trim: frame edges, notches, block-strip gaps."},
  {"name":"space-1","value":"4px","usage":"Glyph-to-label gap, chip padding top and bottom."},
  {"name":"space-2","value":"8px","usage":"Inside chips, keycaps, buttons."},
  {"name":"space-3","value":"12px","usage":"Pane padding, row padding."},
  {"name":"space-4","value":"16px","usage":"Dialog padding, gaps between tiled panes."},
  {"name":"space-6","value":"24px","usage":"Between sections of a dialog or the library grid."}]},
"radius":{"note":"Square. The notch replaces the rounded corner.","tokens":[
  {"name":"radius-0","value":"0","usage":"Frames, panes, buttons, chips, everything with a notched frame."},
  {"name":"radius-1","value":"2px","usage":"Text inputs and the xterm cursor."},
  {"name":"radius-2","value":"4px","usage":"Tooltips only."}]},
"shadow":{"note":"Hard offset shadows only. No blur anywhere.","tokens":[
  {"name":"shadow-hard","value":"6px 6px 0 0 #05060a","usage":"Floating frames: dialogs, toasts, the command palette."},
  {"name":"shadow-hard-sm","value":"3px 3px 0 0 #05060a","usage":"Chips and buttons at rest."},
  {"name":"frame-neutral","value":"0 -2px 0 0 #414756, 0 2px 0 0 #414756, -2px 0 0 0 #414756, 2px 0 0 0 #414756, 6px 6px 0 0 #05060a","usage":"The notched pixel frame: four 2px edges leave the corners open. Dialogs and pin cards."},
  {"name":"frame-amber","value":"0 -2px 0 0 #f3c470, 0 2px 0 0 #f3c470, -2px 0 0 0 #f3c470, 2px 0 0 0 #f3c470, 3px 3px 0 0 #05060a","usage":"Notched frame on amber fills: the primary button, the waiting chip."},
  {"name":"focus-ring","value":"0 0 0 2px #111318, 0 0 0 4px #e5a33a","usage":"Keyboard focus: a 2px ground gap, then 2px solid amber."}]},
"duration":{"note":"Arcade cues: stepped, short. Off under prefers-reduced-motion.","tokens":[
  {"name":"step-fast","value":"80ms","usage":"Block-strip cell fill, chip press."},
  {"name":"step-slow","value":"240ms","usage":"Toast slide-in, done-flash on a step."}]}}
json.dump(tokens,open(sys.argv[1],'w'),indent=2,ensure_ascii=False)
# tokens.css as compiled
css=['/* Terminescu Workbench — generated from tokens.json */',':root, [data-theme="dark"] {']
for n,v,u in C:
  css.append(f'  --{n}: {"var(--"+v[1:-1]+")" if v.startswith("{") else v};')
for t in tokens['shadow']['tokens']: css.append(f"  --{t['name']}: {t['value']};")
css.append('}'); css.append(':root {')
for fam in ('spacing','radius','duration'):
  for t in tokens[fam]['tokens']: css.append(f"  --{t['name']}: {t['value']};")
for k,v in tokens['type']['families'].items(): css.append(f'  --font-{k}: {v};')
css.append('}')
for g in tokens['type']['groups']:
  for s in g['styles']:
    p=[f"font-family: var(--font-{g['family']})",f"font-size: {s['fontSize']}",f"line-height: {s['lineHeight']}",f"font-weight: {s['fontWeight']}"]
    if 'letterSpacing' in s: p.append(f"letter-spacing: {s['letterSpacing']}")
    css.append(f".{s['name']} {{ {'; '.join(p)}; }}")
open(sys.argv[2],'w').write('\n'.join(css)+'\n')
