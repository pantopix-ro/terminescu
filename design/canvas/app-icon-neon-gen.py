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

def glow_sidebar(cid):
    # the picked base: sidebar rows lit, narrow shell on the right
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    inner+=r(100,196,520,728,BG)+r(620,196,10,728,STRONG)
    rows=[(BLUE,96),(AMBER,140),(CYAN,96),(MAGENTA,96)]
    y=236; lit=''
    for c,h in rows:
        lit+=neon(140,y,440,h,c); y+=h+40
    inner+=glow(lit,cid)+lit
    inner+=px(CHEVRON,26,680,440,FG)+r(780,560,90,26,FG)
    return body(VOID)+clip(inner,cid)

INK='#06070a'
# a chunky >: two cells wide on a 5x7 grid, so the strokes join into one shape
CHUNKY=[(c,rw) for rw in range(7) for c in ((rw,rw+1) if rw<=3 else (6-rw,7-rw))]
def prompt(ox,oy,u,ink=INK):
    # >_ at cell size u: 9u wide, 7u high, underscore on the last row
    return px(CHUNKY,u,ox,oy,ink)+r(ox+6*u,oy+6*u,3*u,u,ink)
def smooth_prompt(ox,oy,h,ink=INK,sw=None):
    # a stroked >_ with square ends, h high
    sw=sw or round(h*0.2); w=round(h*0.5)
    return (f'<polyline points="{ox},{oy} {ox+w},{oy+h/2} {ox},{oy+h}" fill="none" stroke="{ink}" stroke-width="{sw}" stroke-linejoin="miter" stroke-linecap="square"/>'
            +r(ox+w+round(h*0.22),oy+h-sw//2,round(h*0.55),sw,ink))

def rows_full(rows,x0,x1,y0,gap):
    y=y0; lit=''
    for c,h in rows:
        lit+=neon(x0,y,x1-x0,h,c); y+=h+gap
    return lit

def full_rows(cid):
    # Glow Sidebar rows run the full width; a black >_ sits over them on the right
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    lit=rows_full([(BLUE,96),(AMBER,140),(CYAN,96),(MAGENTA,96)],140,884,236,40)
    inner+=glow(lit,cid)+lit
    inner+=prompt(480,370,40)
    return body(VOID)+clip(inner,cid)

def tight(cid):
    # same rows with thin gaps, so the colour reads as one block and the >_ stays whole
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    lit=rows_full([(BLUE,120),(AMBER,170),(CYAN,120),(MAGENTA,120)],140,884,250,20)
    inner+=glow(lit,cid)+lit
    inner+=prompt(468,355,44)
    return body(VOID)+clip(inner,cid)

def bleed(cid):
    # no title bar, rows bleed to the icon edge, a big >_ cut over them
    inner=r(100,100,824,824,VOID)
    lit=rows_full([(BLUE,150),(AMBER,200),(CYAN,150),(MAGENTA,150),(GREEN,134)],100,924,100,10)
    inner+=lit
    inner+=prompt(440,300,52)
    return body(VOID)+clip(inner,cid)

def bleed4(cid):
    # Edge to Edge with four rows: no green, the rows grow to fill the icon
    inner=r(100,100,824,824,VOID)
    lit=rows_full([(BLUE,180),(AMBER,254),(CYAN,180),(MAGENTA,180)],100,924,100,10)
    inner+=lit
    inner+=prompt(440,330,52)
    return body(VOID)+clip(inner,cid)

def amber_prompt(cid):
    # the >_ sits inside the tall amber row, the row that means "needs you"
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    rows=[(BLUE,88),(AMBER,300),(CYAN,88),(MAGENTA,88)]
    lit=rows_full(rows,140,884,236,32)
    inner+=glow(lit,cid)+lit
    inner+=prompt(560,286,30)
    return body(VOID)+clip(inner,cid)

def ink_outline(cid):
    # full rows, the >_ in black with a pale rim so it also reads on the gaps
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    lit=rows_full([(BLUE,96),(AMBER,140),(CYAN,96),(MAGENTA,96)],140,884,236,40)
    inner+=glow(lit,cid)+lit
    u=40; ox,oy=480,370
    m=10
    rim=''.join(r(ox+c*u-m,oy+rw*u-m,u+2*m,u+2*m,FG) for c,rw in CHUNKY)+r(ox+6*u-m,oy+6*u-m,3*u+2*m,u+2*m,FG)
    inner+=rim+prompt(ox,oy,u)
    return body(VOID)+clip(inner,cid)

def smooth(cid):
    # Tight Stack rows with a drawn, non-pixel >_ in black
    inner=r(100,100,824,824,VOID)+titlebar(fill=PANEL)
    lit=rows_full([(BLUE,120),(AMBER,170),(CYAN,120),(MAGENTA,120)],140,884,250,20)
    inner+=glow(lit,cid)+lit
    inner+=smooth_prompt(500,390,280)
    return body(VOID)+clip(inner,cid)

ICONS=[('glow','Glow Sidebar','The base you picked, unchanged, for comparison.',glow_sidebar),
 ('full','Full Rows','The four lit rows run the full width. A black >_ sits over them, right of centre; the gaps show the void through it.',full_rows),
 ('tight','Tight Stack','Taller rows with thin gaps, so the colour reads as one block and the black >_ stays almost whole.',tight),
 ('bleed','Edge to Edge','No title bar: five rows run to the icon edge and a big black >_ is cut over them.',bleed),
 ('bleed4','Edge to Edge · 4','Edge to Edge without the green row: four taller rows fill the icon under the black >_.',bleed4),
 ('amberp','Amber Prompt','The amber row grows tall and holds the black >_. The other rows stay thin.',amber_prompt),
 ('smooth','Drawn Prompt','Tight Stack with a drawn >_ in place of the pixel one: thick black strokes with square ends.',smooth),
 ('rim','Rimmed Prompt','Full Rows with a pale rim round the black >_, so it also reads across the gaps.',ink_outline)]

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
    tag='<span class="ao-pixel" style="font-size: 8px; color: var(--dim)">base</span>' if key=='glow' else ''
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

W=2880; H=1100
html=f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>App icon: Neon rows</title>
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
    <h1 class="pixel-display" style="margin: 0; color: var(--fg)">App icon · Neon rows</h1>
    <p class="body" style="margin: 0; color: var(--dim)">Glow Sidebar with its neon rows run across the full width and the shell prompt laid over them in black, on the right.</p>
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
open(os.path.join(ROOT,'project','AppIconNeon.dc.html'),'w').write(html)
print(len(html))
