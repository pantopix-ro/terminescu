import math, json, os
ROOT=os.path.dirname(os.path.abspath(__file__))
BG='#111318'; PANEL='#171a21'; PANEL2='#1d2029'; LINE='#2a2e38'; STRONG='#414756'
FG='#d7dae0'; DIM='#8b91a0'; FAINT='#5c6270'; SHADOW='#05060a'
AMBER='#e5a33a'; AMBER_EDGE='#f3c470'; BLUE='#539bf5'; CYAN='#39c5cf'; MAGENTA='#b083f0'; GREEN='#57ab5a'

# macOS icon grid: 824 body centred in 1024, continuous-curvature corners (superellipse n=5).
def squircle(x=100,y=100,s=824,n=5.0,steps=64):
    a=s/2; cx=x+a; cy=y+a; pts=[]
    for i in range(steps):
        t=2*math.pi*i/steps; c=math.cos(t); si=math.sin(t)
        px=cx+a*math.copysign(abs(c)**(2/n),c); py=cy+a*math.copysign(abs(si)**(2/n),si)
        pts.append(f'{px:.0f} {py:.0f}')
    return 'M'+' L'.join(pts)+' Z'
SQ=squircle()

def r(x,y,w,h,fill,extra=''): return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"{extra}/>'
def px(cells,unit,ox,oy,fill):
    return ''.join(r(ox+c*unit,oy+rw*unit,unit,unit,fill) for c,rw in cells)
CHEVRON=[(0,0),(1,1),(2,2),(1,3),(0,4)]
def body(fill=BG,shadow=True):
    # hard pixel shadow like the design system's frames, clipped by the squircle shape itself
    s=f'<path d="{SQ}" fill="{SHADOW}" transform="translate(0 18)"/>' if shadow else ''
    return s+f'<path d="{SQ}" fill="{fill}"/>'
def clip(inner,cid): return f'<g clip-path="url(#{cid})">{inner}</g>'

def current(cid):
    s=r(0,0,1024,1024,'#181b22')
    s+=r(128,64,768,64,STRONG)+r(64,128,64,768,STRONG)+r(896,128,64,768,STRONG)+r(128,896,768,64,STRONG)
    s+=px(CHEVRON,64,256,320,FG)+r(512,576,256,128,AMBER)
    return s

def rails(cid):
    # side tabs as rails on the left, one carries the amber needs-you marker; prompt on the right
    s=body()
    inner=r(100,100,236,824,PANEL)+r(336,100,12,824,STRONG)
    for i,(acc,fill) in enumerate([(BLUE,'#212f43'),(AMBER,'#383025'),(MAGENTA,'#2f2b42'),(FAINT,PANEL2)]):
        y=260+i*130
        inner+=r(180,y,120,84,fill)+r(180,y,16,84,acc)
    inner+=px(CHEVRON,48,440,400,FG)+r(620,592,176,48,FG)
    return s+clip(inner,cid)

def stack(cid):
    # three terminal windows stacked like side tabs, hard pixel shadows, front one live
    s=body(PANEL)
    inner=''
    for i,(edge,fill) in enumerate([(FAINT,PANEL2),(STRONG,PANEL2),(FG,BG)]):
        x=196+i*72; y=230+i*96; w=520; h=420
        inner+=r(x+24,y+24,w,h,SHADOW)+r(x,y,w,h,edge)+r(x+16,y+16,w-32,h-32,fill)+r(x+16,y+16,w-32,56,edge)
    x=196+144; y=230+192
    inner+=px(CHEVRON,36,x+64,y+140,FG)+r(x+200,y+284,120,36,AMBER)
    return s+clip(inner,cid)

T=[(c,0) for c in range(5)]+[(2,r_) for r_ in range(1,6)]
def pixel_t(cid):
    # Silkscreen-like pixel T monogram; one amber pixel is the cursor
    s=body()
    inner=r(100,100,824,824,BG)
    sh=''.join(r(232+c*112+16,232+rw*112+16,112,112,SHADOW) for c,rw in T)
    fg=''.join(r(232+c*112,232+rw*112,112,112,FG) for c,rw in T)
    inner+=sh+fg+r(232+3*112+40,232+5*112,112,112,AMBER)
    return s+clip(inner,cid)

def beacon(cid):
    # one terminal window; the amber dot is the session that needs you
    s=body('#e9e6df')
    inner=r(100,100,824,824,'#e9e6df')
    x,y,w,h=196,250,632,520
    inner+=r(x+24,y+24,w,h,SHADOW)+r(x,y,w,h,BG)+r(x,y,w,64,STRONG)
    inner+=''.join(r(x+28+i*48,y+20,24,24,FAINT) for i in range(3))
    inner+=px(CHEVRON,40,x+72,y+150,FG)+r(x+248,y+310,200,40,FG)+r(x+72,y+400,320,24,FAINT)
    inner+=r(704+12,188+12,152,152,SHADOW)+r(704,188,152,152,AMBER)+r(704+16,188+16,120,120,AMBER_EDGE)+r(704+40,188+40,72,72,AMBER)
    return s+clip(inner,cid)

def panes(cid):
    # split panes: four terminals, the waiting one wears the amber frame
    s=body()
    inner=r(100,100,824,824,BG)
    g=24; x0,y0=200,200; cw=(624-g)//2
    cells=[(0,0,BLUE,False),(1,0,STRONG,True),(0,1,STRONG,False),(1,1,CYAN,False)]
    for c,rw,edge,hot in cells:
        x=x0+c*(cw+g); y=y0+rw*(cw+g)
        e=AMBER if hot else edge
        inner+=r(x,y,cw,cw,e)+r(x+16,y+16,cw-32,cw-32,PANEL)
        if hot: inner+=px(CHEVRON,28,x+60,y+80,FG)+r(x+170,y+192,80,28,AMBER)
        else: inner+=r(x+56,y+90,cw-150,20,FAINT)+r(x+56,y+140,cw-200,20,FAINT)
    return s+clip(inner,cid)

def sidebar(cid):
    # the app in miniature: status-coloured side tabs on the left, a shell on the right
    s=body(BG)
    inner=r(100,100,824,824,BG)+r(100,100,824,96,PANEL2)
    inner+=''.join(r(170+i*56,136,32,32,FAINT) for i in range(3))
    inner+=r(100,196,300,728,PANEL)+r(400,196,12,728,STRONG)
    rows=[('#212f43',BLUE),('#383025',AMBER),('#1c353d',CYAN),('#2f2b42',MAGENTA),(PANEL2,FAINT)]
    for i,(fill,acc) in enumerate(rows):
        y=236+i*112
        inner+=r(140,y,224,80,fill)+r(140,y,14,80,acc)+r(176,y+30,120,20,DIM if acc!=AMBER else AMBER_EDGE)
    inner+=px(CHEVRON,44,480,380,FG)+r(650,556,150,44,FG)
    return s+clip(inner,cid)

ICONS=[('current','Current','The shipped icon: a pixel >_ with an amber cursor in a square frame. No macOS shape, it fills the whole tile.',current),
 ('rails','Rails','Side tabs as rails beside a prompt. The amber rail is the session that needs you.',rails),
 ('stack','Stack','Three stacked terminal windows with hard pixel shadows: tickets kept apart, the front one live.',stack),
 ('pixel-t','Pixel T','A pixel T monogram drawn like Silkscreen, with an amber pixel as the blinking cursor.',pixel_t),
 ('beacon','Beacon','One dark terminal on a light ground with an amber beacon in the corner: "this one needs you".',beacon),
 ('panes','Panes','Four split panes. The waiting one wears the amber frame, as in the app.',panes),
 ('sidebar','Sidebar','The app in miniature: side tabs in status colours on the left, a shell on the right.',sidebar)]

def svg(key,fn,size,label=None):
    cid=f'c-{key}-{size}'
    defs=f'<defs><clipPath id="{cid}"><path d="{SQ}"/></clipPath></defs>'
    a=f'role="img" aria-label="{label}"' if label else 'aria-hidden="true"'
    return f'<svg viewBox="0 0 1024 1024" width="{size}" height="{size}" {a} style="display: block; flex: none">{defs}{fn(cid)}</svg>'

cards=[]
for key,name,line,fn in ICONS:
    big=svg(key,fn,256,f'{name} app icon')
    sizes=''.join(f'<div style="display: flex; flex-direction: column; align-items: center; gap: 8px">{svg(key,fn,s)}<span class="small" style="color: var(--dim)">{s}</span></div>' for s in (128,32,16))
    def dock(bg,edge,ph):
        others=''.join(f'<svg viewBox="0 0 1024 1024" width="56" height="56" aria-hidden="true" style="display: block; flex: none"><path d="{SQ}" fill="{ph}"/></svg>' for _ in range(2))
        return (f'<div style="display: flex; align-items: center; justify-content: center; gap: 10px; height: 80px; padding: 0 16px; background: {bg}; border: 1px solid {edge}; border-radius: 22px">'
                f'{others[:len(others)//2]}{svg(key,fn,64)}{others[len(others)//2:]}</div>')
    tag='<span class="ao-pixel" style="font-size: 8px; color: var(--dim)">now</span>' if key=='current' else ''
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
<title>App icon options</title>
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
    <h1 class="pixel-display" style="margin: 0; color: var(--fg)">App icon</h1>
    <p class="body" style="margin: 0; color: var(--dim)">Six options next to the current icon. Each sits on the macOS grid: an 824 px body inside a 1024 px tile, with continuous corners. Amber appears only as the needs-you signal.</p>
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
open(os.path.join(ROOT,'project','AppIcon.dc.html'),'w').write(html)
print(len(html))
