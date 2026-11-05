import math, os
ROOT=os.path.dirname(os.path.abspath(__file__))
BG='#111318'; PANEL='#171a21'; PANEL2='#1d2029'; STRONG='#414756'; FG='#d7dae0'; DIM='#8b91a0'; FAINT='#5c6270'; SHADOW='#05060a'
AMBER='#e5a33a'; AMBER_EDGE='#f3c470'
BLUE='#539bf5'; CYAN='#39c5cf'; GREEN='#57ab5a'; MAGENTA='#b083f0'; RED='#e5534b'
TINT={BLUE:'#212f43',CYAN:'#1c353d',GREEN:'#21312a',MAGENTA:'#2f2b42',RED:'#382328',AMBER:'#383025'}
VOID='#0a0b0f'

def squircle(x=100,y=100,s=824,n=5.0,steps=64):
    a=s/2; cx=x+a; cy=y+a; pts=[]
    for i in range(steps):
        t=2*math.pi*i/steps; c=math.cos(t); si=math.sin(t)
        pts.append(f'{cx+a*math.copysign(abs(c)**(2/n),c):.0f} {cy+a*math.copysign(abs(si)**(2/n),si):.0f}')
    return 'M'+' L'.join(pts)+' Z'
SQ=squircle()
def r(x,y,w,h,fill,extra=''): return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"{extra}/>'
def px(cells,unit,ox,oy,fill): return ''.join(r(ox+c*unit,oy+rw*unit,unit,unit,fill) for c,rw in cells)
CHEVRON=[(0,0),(1,1),(2,2),(1,3),(0,4)]
def light(h,t=.45):
    h=h.lstrip('#'); c=[int(h[i:i+2],16) for i in (0,2,4)]
    return '#'+''.join(f'{round(v+(255-v)*t):02x}' for v in c)
def body(fill=BG): return f'<path d="{SQ}" fill="{SHADOW}" transform="translate(0 18)"/><path d="{SQ}" fill="{fill}"/>'
def clip(inner,cid): return f'<g clip-path="url(#{cid})">{inner}</g>'
def glow(shapes,cid,sd=26): return f'<g filter="url(#g-{cid})">{shapes}</g>' if sd else shapes
def neon(x,y,w,h,col,inner=True):
    # the bar itself, plus a lighter core so it reads as lit from inside
    s=r(x,y,w,h,col)
    if inner and h>=40: s+=r(x+12,y+h//2-max(6,h//10),w-24,max(12,h//5),light(col,.55),' opacity="0.85"')
    return s
def titlebar(y=100,h=96,fill=PANEL2): return r(100,y,824,h,fill)+''.join(r(170+i*56,y+h//2-16,32,32,FAINT) for i in range(3))

def original(cid):
    inner=r(100,100,824,824,BG)+titlebar()
    inner+=r(100,196,300,728,PANEL)+r(400,196,12,728,STRONG)
    for i,(fill,acc) in enumerate([(TINT[BLUE],BLUE),(TINT[AMBER],AMBER),(TINT[CYAN],CYAN),(TINT[MAGENTA],MAGENTA),(PANEL2,FAINT)]):
        y=236+i*112
        inner+=r(140,y,224,80,fill)+r(140,y,14,80,acc)+r(176,y+30,120,20,DIM if acc!=AMBER else AMBER_EDGE)
    inner+=px(CHEVRON,44,480,380,FG)+r(650,556,150,44,FG)
    return body()+clip(inner,cid)

def three_bars(cid):
    # three thick full-width neon side tabs; the middle, amber one is the tallest
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    bars=[(260,150,CYAN),(450,210,AMBER),(700,150,MAGENTA)]
    lit=''.join(neon(170,y,684,h,c) for y,h,c in bars)
    inner+=glow(lit,cid)+lit
    inner+=px(CHEVRON,16,196,268+40,BG)
    return body(VOID)+clip(inner,cid)

def tiles(cid):
    # status colours as big glowing tiles; the shell is a small >_ chip
    inner=r(100,100,824,824,VOID)
    g=40; s=322; x0=170; y0=170
    cols=[BLUE,MAGENTA,CYAN,AMBER]
    lit=''
    for i,c in enumerate(cols):
        x=x0+(i%2)*(s+g); y=y0+(i//2)*(s+g)
        lit+=neon(x,y,s,s,c,inner=False)+r(x+28,y+28,s-56,s-56,light(c,.25))
    inner+=glow(lit,cid)+lit
    cx,cy=x0+s+g+60,y0+s+g+150
    inner+=r(cx-8,cy-8,190,120,VOID)+px(CHEVRON,20,cx+20,cy+6,FG)+r(cx+90,cy+70,70,20,FG)
    return body(VOID)+clip(inner,cid)

def glow_sidebar(cid):
    # the original layout, but the sidebar takes 60% and its rows are lit neon
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    inner+=r(100,196,520,728,BG)+r(620,196,10,728,STRONG)
    rows=[(BLUE,96),(AMBER,140),(CYAN,96),(MAGENTA,96)]
    y=236; lit=''
    for c,h in rows:
        lit+=neon(140,y,440,h,c); y+=h+40
    inner+=glow(lit,cid)+lit
    inner+=px(CHEVRON,26,680,440,FG)+r(780,560,90,26,FG)
    return body(VOID)+clip(inner,cid)

def amber_lead(cid):
    # amber needs-you row dominates; the other statuses are thin neon lines
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    lit=neon(160,250,704,240,AMBER)
    for i,c in enumerate([CYAN,MAGENTA,GREEN,BLUE]):
        lit+=neon(160,560+i*84,704-i*110,44,c)
    inner+=glow(lit,cid,30)+lit
    inner+=px(CHEVRON,28,212,300,'#1a1205')+r(330,412,100,28,'#1a1205')
    return body(VOID)+clip(inner,cid)

def sidebar_only(cid):
    # no shell pane: the sidebar is the whole icon, five lit side tabs
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    rows=[BLUE,AMBER,CYAN,MAGENTA,GREEN]
    lit=''; tints=''
    for i,c in enumerate(rows):
        y=236+i*130
        tints+=r(150,y,724,96,TINT[c])
        lit+=neon(150,y,36,96,c,inner=False)+r(222,y+34,300 if c!=AMBER else 420,28,c)
    inner+=tints+glow(lit,cid)+lit
    return body(VOID)+clip(inner,cid)

def spectrum(cid):
    # all six status hues as one bright stack, amber widest; reads as a band of colour at 16 px
    inner=r(100,100,824,824,VOID)
    rows=[(BLUE,560),(CYAN,620),(AMBER,724),(GREEN,620),(MAGENTA,560),(RED,500)]
    lit=''; y=170
    for c,w in rows:
        h=118 if c==AMBER else 86
        lit+=neon(150,y,w,h,c); y+=h+28
    inner+=glow(lit,cid,24)+lit
    return body(VOID)+clip(inner,cid)

ICONS=[('orig','Sidebar','The option you picked, unchanged, for comparison.',original),
 ('bars','Three Bars','Three thick neon side tabs across the full width. The amber one in the middle is the tallest.',three_bars),
 ('tiles','Tiles','Four big glowing status tiles. The shell shrinks to a small >_ chip in a corner.',tiles),
 ('glow','Glow Sidebar','Your layout with a wider sidebar: four lit rows glowing on a near-black ground, with a narrow shell.',glow_sidebar),
 ('amber','Amber Lead','The amber needs-you row dominates. The other statuses become thin neon lines below it.',amber_lead),
 ('only','Sidebar Only','No shell pane: five lit side tabs fill the icon, each with a neon edge and label.',sidebar_only),
 ('spectrum','Spectrum','All six status hues stacked as glowing bars, with amber the widest. At 16 px it reads as a band of colour.',spectrum)]

def svg(key,fn,size,label=None):
    cid=f'c-{key}-{size}'
    # glow strength stays the same share of the tile at any size, since it lives in the viewBox
    defs=(f'<defs><clipPath id="{cid}"><path d="{SQ}"/></clipPath>'
          f'<filter id="g-{cid}" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="26"/></filter></defs>')
    a=f'role="img" aria-label="{label}"' if label else 'aria-hidden="true"'
    return f'<svg viewBox="0 0 1024 1024" width="{size}" height="{size}" {a} style="display: block; flex: none">{defs}{fn(cid)}</svg>'

cards=[]
for key,name,line,fn in ICONS:
    big=svg(key,fn,256,f'{name} app icon')
    sizes=''.join(f'<div style="display: flex; flex-direction: column; align-items: center; gap: 8px">{svg(key,fn,s)}<span class="small" style="color: var(--dim)">{s}</span></div>' for s in (128,32,16))
    def dock(bg,edge,ph):
        o=f'<svg viewBox="0 0 1024 1024" width="56" height="56" aria-hidden="true" style="display: block; flex: none"><path d="{SQ}" fill="{ph}"/></svg>'
        return (f'<div style="display: flex; align-items: center; justify-content: center; gap: 10px; height: 80px; padding: 0 16px; background: {bg}; border: 1px solid {edge}; border-radius: 22px">'
                f'{o}{svg(key,fn,64)}{o}</div>')
    tag='<span class="ao-pixel" style="font-size: 8px; color: var(--dim)">picked</span>' if key=='orig' else ''
    cards.append(f'''
    <section aria-labelledby="h-{key}" style="box-sizing: border-box; display: flex; flex-direction: column; gap: 20px; width: 320px; padding: 20px; background: var(--panel); box-shadow: var(--frame-neutral)">
      <div style="display: flex; align-items: baseline; gap: 10px"><h2 id="h-{key}" class="pixel-heading" style="margin: 0; color: var(--fg)">{name}</h2>{tag}</div>
      <div style="display: flex; justify-content: center; padding: 8px 0">{big}</div>
      <p class="body" style="margin: 0; min-height: 80px; color: var(--fg)">{line}</p>
      <div style="display: flex; align-items: flex-end; justify-content: space-between; padding: 12px 8px; background: var(--bg); border: 2px solid var(--line)">{sizes}</div>
      <div style="display: grid; gap: 10px">
        <span class="ao-pixel" style="font-size: 8px; color: var(--dim)">Dock, light and dark</span>
        {dock('#f2f1ee','#d6d4cf','#c9c6bf')}
        {dock('#2b2c31','#3c3e45','#4a4c54')}
      </div>
    </section>''')

W=2528; H=1100
html=f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>App icon: Sidebar variants</title>
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
a{{color:#539bf5}}a:hover{{color:#8bbcf8}}
</style>
</helmet>
<div style="width: {W}px; height: {H}px; box-sizing: border-box; padding: 48px; display: flex; flex-direction: column; gap: 36px; background: var(--bg); color: var(--fg); font-family: var(--font-mono)">
  <header style="display: flex; align-items: baseline; gap: 24px">
    <h1 class="pixel-display" style="margin: 0; color: var(--fg)">App icon · Sidebar variants</h1>
    <p class="body" style="margin: 0; color: var(--dim)">The Sidebar option, with its status colours made bigger and lit. A blur halo and a lighter core on each shape make them glow. Amber still marks only the session that needs you.</p>
  </header>
  <div style="display: flex; gap: 32px; align-items: stretch">{"".join(cards)}
  </div>
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":{W},"height":{H}}}}}'>
class Component extends DCLogic {{}}
</script>
</body>
</html>
'''
open(os.path.join(ROOT,'project','AppIconSidebar.dc.html'),'w').write(html)
print(len(html))
