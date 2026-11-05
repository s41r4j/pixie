#!/usr/bin/env python3
"""Reproducible OpenClicky v1 pixel design asset generator. No external image/font binaries bundled."""
from pathlib import Path
import os
from PIL import Image, ImageDraw, ImageFont
import numpy as np, math, random, struct, json, csv, re, shutil, hashlib, subprocess, zipfile, io, colorsys

_SELF=Path(__file__).resolve()
ROOT=Path(os.environ.get('OPENCLICKY_OUT',str(_SELF.parent.parent if _SELF.parent.name=='scripts' else _SELF.parent/'OpenClicky_Pixel_Design_System_v1')))
SRC=ROOT.parent
ROOT.mkdir(parents=True,exist_ok=True)
P={'black':'#000000','ink':'#080808','deep':'#141414','charcoal':'#404040','mid':'#A0A0A0','silver':'#E5E5E5','white':'#FFFFFF'}
SIZES=[16,24,32,48,64,128,256,512,1024]
WHITE=(255,255,255,255)
BLACK=(0,0,0,255)

def makepath(rel):
 p=ROOT/rel;p.parent.mkdir(parents=True,exist_ok=True);return p

def resize_nearest(im,n): return im.resize((n,n),Image.Resampling.NEAREST)

def pixel_svg(im,padding=0):
 """Lossless SVG path representation of logical pixel art; preserves transparent and grey pixels."""
 pix=im.convert('RGBA');w,h=pix.size; pixels=pix.load(); paths={}
 for y in range(h):
  x=0
  while x<w:
   color=pixels[x,y]; start=x
   while x<w and pixels[x,y]==color: x+=1
   if color[3]==0:continue
   key=color; paths.setdefault(key,[]).append(f'M{start} {y}h{x-start}v1h{- (x-start)}z')
 head=[f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" shape-rendering="crispEdges" aria-hidden="true">']
 for (r,g,b,a),d in paths.items():
  attrs=f'fill="#{r:02X}{g:02X}{b:02X}"'
  if a<255:attrs+=f' opacity="{a/255:.5f}"'
  head.append(f'<path {attrs} d="{" ".join(d)}"/>')
 return '\n'.join(head+['</svg>'])+'\n'

def draw_step_round(d,xy,r,fill,outline=None,width=1):
 x1,y1,x2,y2=xy
 pts=[(x1+r,y1),(x2-r,y1),(x2,y1+r),(x2,y2-r),(x2-r,y2),(x1+r,y2),(x1,y2-r),(x1,y1+r)]
 d.polygon(pts,fill=fill)
 if outline:d.line(pts+[pts[0]],fill=outline,width=width,joint='curve')

def symbol_draw(d,k,origin=(0,0),color=WHITE,unit=1):
 """64px grid master smile; small sizes have separately drawn glyph."""
 ox,oy=origin
 def R(x,y,w,h):d.rectangle((ox+x*unit,oy+y*unit,ox+(x+w)*unit-1,oy+(y+h)*unit-1),fill=color)
 R(23,22,6,6);R(23,36,6,6)
 # Distinctive right parenthesis with stepped caps and balanced stroke.
 for x,y,w,h in [(35,16,5,5),(39,19,5,5),(42,23,4,5),(44,28,4,8),(42,36,4,5),(39,40,5,5),(35,44,5,5)]:R(x,y,w,h)

def icon_base(variant,sz=64):
 if sz==16:return icon_small(variant,16)
 if sz==32:return icon_small(variant,32)
 S=64
 bg=P['black'] if variant!='light' else P['white']
 im=Image.new('RGBA',(S,S),bg);d=ImageDraw.Draw(im)
 if variant=='default':
  draw_step_round(d,(2,2,61,61),7,P['black'],P['white'],2)
  # sparse checker dither restricted to the inner lower-right corner
  for y in range(42,59):
   for x in range(39,59):
    if x+y>=94:
     bayer4=((0,8,2,10),(12,4,14,6),(3,11,1,9),(15,7,13,5))
     if bayer4[y%4][x%4] < (x+y-94)*0.44:d.point((x,y),fill=P['white'])
 elif variant=='outline':
  draw_step_round(d,(3,3,60,60),7,P['black'],P['white'],1)
 elif variant=='light':
  draw_step_round(d,(1,1,62,62),7,P['white'])
  for x in range(49,60):
   for y in range(52,61):
    if x+y>109 and (x+y)%5==0:d.point((x,y),fill=P['silver'])
 elif variant=='dither':
  draw_step_round(d,(1,1,62,62),7,P['silver'],P['white'],1)
  bayer=[[0,8,2,10],[12,4,14,6],[3,11,1,9],[15,7,13,5]]
  for y in range(4,61):
   for x in range(4,61):
    if (x<9 and y<9) or (x>55 and y<9) or (x<9 and y>55) or (x>55 and y>55):continue
    shade=max(0.,min(1.,(y-14)/50.))
    if bayer[y%4][x%4]<shade*16:d.point((x,y),fill=P['charcoal'] if y<46 else P['black'])
 # Symbol must always read clearly over background.
 col=P['black'] if variant in ('light','dither') else P['white']
 symbol_draw(d,64,(0,0),col,1)
 return im

def icon_small(variant,n):
 bg=P['white'] if variant=='light' else P['black'];fg=P['black'] if variant in ('light','dither') else P['white']
 im=Image.new('RGBA',(n,n),bg);d=ImageDraw.Draw(im)
 if n==16:
  if variant=='dither':d.rectangle((0,0,15,15),fill=P['silver'])
  if variant=='default' or variant=='outline':
   pts=[(3,0),(12,0),(15,3),(15,12),(12,15),(3,15),(0,12),(0,3),(3,0)]
   d.line(pts,fill=P['white'],width=1)
  d.rectangle((5,5,6,6),fill=fg);d.rectangle((5,9,6,10),fill=fg)
  for xy in [(9,3,10,4),(11,5,11,6),(12,7,12,8),(11,9,11,10),(9,11,10,12)]:d.rectangle(xy,fill=fg)
 else:
  if variant=='dither':d.rectangle((0,0,n-1,n-1),fill=P['silver'])
  if variant in ('default','outline'):
   pts=[(4,1),(27,1),(30,4),(30,27),(27,30),(4,30),(1,27),(1,4),(4,1)]
   d.line(pts,fill=P['white'],width=1 if variant=='outline' else 2)
  d.rectangle((11,10,13,12),fill=fg);d.rectangle((11,19,13,21),fill=fg)
  for xy in [(18,8,20,9),(21,10,22,12),(23,13,24,18),(21,19,22,21),(18,22,20,23)]:d.rectangle(xy,fill=fg)
 return im

def app_icon(variant,n):
 logical=icon_base(variant,16 if n==16 else 32 if n==32 else 64)
 return resize_nearest(logical,n)

def write_ico(path,ims):
 # ICO with explicitly prepared PNG frames, preserving nearest-neighbour detail.
 data=[io.BytesIO() for _ in ims]
 for b,im in zip(data,ims):im.save(b,format='PNG')
 raw=[b.getvalue() for b in data]
 ofs=6+16*len(raw)
 head=struct.pack('<HHH',0,1,len(raw)); chunks=[]
 for im,b in zip(ims,raw):
  n=im.size[0];head+=struct.pack('<BBBBHHII',0 if n==256 else n,0 if n==256 else n,0,0,1,32,len(b),ofs);ofs+=len(b);chunks.append(b)
 path.write_bytes(head+b''.join(chunks))

def write_icns(path,ims):
 data=[]
 for code,n in [('icp4',16),('icp5',32),('icp6',64),('ic07',128),('ic08',256),('ic09',512),('ic10',1024)]:
  b=io.BytesIO();ims[n].save(b,format='PNG');p=b.getvalue();data.append(code.encode()+struct.pack('>I',len(p)+8)+p)
 payload=b''.join(data);path.write_bytes(b'icns'+struct.pack('>I',len(payload)+8)+payload)

for variant in ['default','light','outline','dither']:
 images={n:app_icon(variant,n) for n in SIZES}
 for n,im in images.items():im.save(makepath(f'assets/app-icons/png/{variant}-{n}.png'))
 makepath(f'assets/app-icons/svg/{variant}.svg').write_text(pixel_svg(icon_base(variant,64)))
 for sz in [16,32,64,128,256,512]:
  images[sz].save(makepath(f'assets/platform/macos/{variant}.iconset/icon_{sz}x{sz}.png'))
 for sz in [16,32,128,256,512]:
  if sz*2<=1024:images[sz*2].save(makepath(f'assets/platform/macos/{variant}.iconset/icon_{sz}x{sz}@2x.png'))
 write_icns(makepath(f'assets/platform/macos/OpenClicky-{variant}.icns'),images)
 write_ico(makepath(f'assets/platform/windows/OpenClicky-{variant}.ico'),[images[x] for x in (16,24,32,48,64,128,256)])

# Simple monochrome pixel system glyphs authored at 16x16 logical pixels.
def glyph(k):
 im=Image.new('RGBA',(16,16),(0,0,0,0));d=ImageDraw.Draw(im)
 def line(pts,w=1):d.line(pts,fill='white',width=w)
 def rect(p):d.rectangle(p,fill='white')
 if k in ('mic','mic-off'):
  line([(7,2),(8,2),(9,3),(9,8),(8,9),(7,9),(6,8),(6,3),(7,2)])
  line([(4,7),(4,9),(6,11),(9,11),(11,9),(11,7)])
  line([(8,11),(8,14)]);line([(5,14),(11,14)])
  if k=='mic-off':line([(2,13),(13,2)],2)
 elif k=='sparkle':
  line([(7,1),(7,4),(9,7),(13,8),(9,9),(7,12),(7,15),(5,10),(2,8),(5,6),(7,1)])
 elif k=='focus':
  for x,y,xx,yy in [(1,1,5,1),(1,1,1,5),(10,1,14,1),(14,1,14,5),(1,10,1,14),(1,14,5,14),(10,14,14,14),(14,10,14,14)]:line([(x,y),(xx,yy)])
 elif k=='cursor':
  line([(3,1),(3,13),(6,10),(8,15),(10,14),(8,10),(13,10),(3,1)])
 elif k=='layers':
  line([(2,5),(8,2),(14,5),(8,8),(2,5)]);line([(2,9),(8,12),(14,9)]);line([(2,12),(8,15),(14,12)])
 elif k=='chat':
  line([(3,2),(12,2),(14,4),(14,10),(12,12),(7,12),(3,15),(3,12),(1,10),(1,4),(3,2)])
 elif k=='waveform':
  for x,h in [(2,3),(4,6),(6,10),(8,13),(10,7),(12,5),(14,3)]:line([(x,8-h//2),(x,8+h//2)])
 elif k=='settings':
  d.rectangle((4,4,11,11),outline='white');d.rectangle((6,6,9,9),fill='white')
  for xy in [(7,1,8,3),(7,12,8,14),(1,7,3,8),(12,7,14,8),(2,2,3,3),(12,12,13,13),(12,2,13,3),(2,12,3,13)]:rect(xy)
 elif k=='bolt':
  line([(9,1),(4,8),(8,8),(6,15),(13,6),(9,6),(9,1)])
 elif k=='terminal':
  d.rectangle((1,2,14,13),outline='white');line([(3,5),(6,7),(3,9)]);line([(8,10),(12,10)])
 elif k=='grid':
  for x in [1,9]:
   for y in [1,9]:d.rectangle((x,y,x+5,y+5),outline='white')
 elif k=='sliders':
  for y,x in [(4,5),(8,10),(12,6)]:
   line([(1,y),(14,y)]);d.rectangle((x-1,y-2,x+1,y+2),fill='black',outline='white')
 elif k=='sync':
  line([(4,3),(8,1),(12,3),(14,6)]);line([(11,3),(12,3),(12,1)])
  line([(12,12),(8,14),(4,12),(2,9)]);line([(3,12),(4,12),(4,14)])
 elif k=='check':line([(2,8),(6,12),(14,3)],2)
 elif k=='close':line([(2,2),(13,13)],2);line([(13,2),(2,13)],2)
 elif k=='plus':line([(8,2),(8,13)],2);line([(2,8),(13,8)],2)
 elif k=='arrow':line([(2,8),(12,8)]);line([(9,4),(13,8),(9,12)])
 elif k=='eye':line([(1,8),(5,4),(10,4),(14,8),(10,12),(5,12),(1,8)]);d.ellipse((6,6,9,9),fill='white')
 elif k=='speaker':
  rect((2,6,4,10));d.polygon([(4,6),(9,2),(9,14),(4,10)],fill='white');line([(11,5),(13,8),(11,11)])
 elif k=='pause':rect((4,3,6,12));rect((10,3,12,12))
 elif k=='stop':rect((4,4,11,11))
 elif k=='record':d.ellipse((3,3,12,12),fill='white')
 elif k=='screen':d.rectangle((1,2,14,11),outline='white');line([(8,11),(8,14)]);line([(5,14),(11,14)])
 elif k=='lock':d.rectangle((3,7,12,14),outline='white');line([(5,7),(5,4),(7,2),(9,2),(11,4),(11,7)])
 elif k=='keyboard':
  d.rectangle((1,3,14,12),outline='white')
  for y in (5,7):
   for x in (3,5,7,9,11):d.point((x,y),fill='white')
  line([(4,10),(11,10)])
 else:raise ValueError(k)
 return im

GLYPHS=['mic','mic-off','sparkle','focus','cursor','layers','chat','waveform','settings','bolt','terminal','grid','sliders','sync','check','close','plus','arrow','eye','speaker','pause','stop','record','screen','lock','keyboard']
for key in GLYPHS:
 g=glyph(key)
 makepath(f'assets/system-icons/svg/{key}.svg').write_text(pixel_svg(g))
 for n in (16,24,32,48,64,128):resize_nearest(g,n).save(makepath(f'assets/system-icons/png/{n}/{key}.png'))

# Microphone states: black/white high-contrast tiles and explicit state pictograms.
MIC_STATES=['idle','listening','processing','muted']
for state in MIC_STATES:
 b=Image.new('RGBA',(64,64),'black');d=ImageDraw.Draw(b)
 draw_step_round(d,(3,3,60,60),7,'black','white',2 if state=='listening' else 1)
 g=glyph('waveform' if state=='processing' else 'mic-off' if state=='muted' else 'mic').resize((32,32),Image.Resampling.NEAREST)
 b.alpha_composite(g,(16,17))
 if state=='listening':
  for x,y in [(20,8),(31,6),(43,8),(50,17),(13,17)]:d.rectangle((x,y,x+1,y+1),fill='white')
 elif state=='processing':
  for xy in [(13,14),(50,14),(13,50),(50,50)]:d.point(xy,fill='white')
 elif state=='idle':d.line((22,52,42,52),fill='white',width=1)
 elif state=='muted':d.line((16,48,49,15),fill='white',width=2)
 makepath(f'assets/microphone-states/svg/{state}.svg').write_text(pixel_svg(b))
 for n in (32,64,128,256):resize_nearest(b,n).save(makepath(f'assets/microphone-states/png/{state}-{n}.png'))

# Menubar: template is BLACK with alpha; OS recolors black template when dark/light.
base=glyph('mic')
for n in (22,44):
 a=resize_nearest(base,n);pixels=a.load()
 for y in range(n):
  for x in range(n):
   r,g,b,al=pixels[x,y];pixels[x,y]=(0,0,0,al)
 a.save(makepath(f'assets/platform/macos/tray-template-{n}.png'))
for n in (16,32,48,128):app_icon('default',n).save(makepath(f'assets/platform/browser/extension-icon-{n}.png'))
for n in (16,32):
 resize_nearest(glyph('mic'),n).save(makepath(f'assets/platform/browser/toolbar-{n}.png'))
 resize_nearest(glyph('mic-off'),n).save(makepath(f'assets/platform/browser/toolbar-{n}-off.png'))
for n in (16,32,48):
 resize_nearest(glyph('mic'),n).save(makepath(f'assets/platform/windows/tray-light-{n}.png'))
 # inverted alpha glyph in white square to remain visible on a light context
 canvas=Image.new('RGBA',(n,n),'white');im=resize_nearest(glyph('mic'),n)
 p=im.load();q=canvas.load()
 for y in range(n):
  for x in range(n):
   if p[x,y][3]:q[x,y]=BLACK
 canvas.save(makepath(f'assets/platform/windows/tray-dark-{n}.png'))
write_ico(makepath('assets/platform/windows/OpenClicky-tray.ico'),[app_icon('outline',n) for n in (16,24,32,48,64,128,256)])

# Brand marks/wordmarks from bitmap vector source; no font binary is distributed.
textfont=ImageFont.load_default()
def bitmap_text(text,scale=4,fg='white'):
 bb=textfont.getbbox(text);mw=max(1,bb[2]-bb[0]);mh=max(1,bb[3]-bb[1]);im=Image.new('RGBA',(mw+4,mh+4),(0,0,0,0))
 d=ImageDraw.Draw(im);d.text((2-bb[0],2-bb[1]),text,font=textfont,fill=fg)
 return im.resize((im.width*scale,im.height*scale),Image.Resampling.NEAREST)
for name,fg in [('wordmark-white','white'),('wordmark-black','black')]:
 w=bitmap_text('OpenClicky :)',4,fg)
 w.save(makepath(f'assets/brand/{name}.png'))
 # SVG pixel font editable geometry; shrink logical to original bitmap size.
 low=w.resize((w.width//4,w.height//4),Image.Resampling.NEAREST)
 makepath(f'assets/brand/{name}.svg').write_text(pixel_svg(low))
for name,fg in [('mark-white','white'),('mark-black','black')]:
 im=Image.new('RGBA',(64,64),(0,0,0,0));d=ImageDraw.Draw(im);symbol_draw(d,64,(0,0),fg,1)
 im.save(makepath(f'assets/brand/{name}.png'))
 makepath(f'assets/brand/{name}.svg').write_text(pixel_svg(im))

print('STEP 1: icons, platform exports, microphone states and marks done')

# --- Wallpaper design: 4 original pixel-art scenes, dark+inverted light, native 3840x2160.
W,H=960,540
BG=(0,0,0)
WH=(255,255,255)
G1=(229,229,229)
G2=(160,160,160)
G3=(64,64,64)
SEED=1329

def starfield(d,which,n=180,margin=20):
 rnd=random.Random(SEED+which)
 for _ in range(n):
  x=rnd.randint(margin,W-margin);y=rnd.randint(margin,int(H*.65))
  size=2 if rnd.random()<.08 else 1
  d.rectangle((x,y,x+size-1,y+size-1),fill=rnd.choice([WH,WH,G1,G2,G3]))
 for x,y in [(72,78),(783,120),(545,58),(875,190)]:
  if y>H*.5:continue
  d.line((x-4,y,x+4,y),fill=G1);d.line((x,y-4,x,y+4),fill=G1)

def stipple_region(d,pts,seed,density=3):
 # dither is real 1-bit/4-tone stochastic stippling, not a smooth gradient.
 r=random.Random(seed)
 minx=max(0,int(min(x for x,y in pts)));maxx=min(W-1,int(max(x for x,y in pts)))
 miny=max(0,int(min(y for x,y in pts)));maxy=min(H-1,int(max(y for x,y in pts)))
 # use rectangle bounding but cheap: noise dots only, masked by PIL polygon.
 mask=Image.new('1',(W,H),0);dm=ImageDraw.Draw(mask);dm.polygon(pts,fill=1)
 for y in range(miny,maxy,3):
  for x in range(minx,maxx,3):
   if mask.getpixel((x,y)) and r.random()<density/10:d.point((x,y),fill=G2 if r.random()<.45 else WH)

def wallpaper_scene(name):
 im=Image.new('RGB',(W,H),BG);d=ImageDraw.Draw(im)
 if name=='01-moonrise':
  starfield(d,1,215)
  d.ellipse((610,78,760,228),fill=WH)
  for y in range(81,220,5):
   for x in range(614,755,5):
    if ((x-685)**2+(y-153)**2)<(72**2) and ((x//5+y//5)%7==0):d.rectangle((x,y,x+1,y+1),fill=G3)
  back=[(0,382),(95,306),(174,356),(251,270),(338,358),(435,289),(530,349),(620,287),(717,375),(834,293),(960,382),(960,H),(0,H)]
  d.polygon(back,fill=G3)
  for j in range(len(back)-3):
   x0,y0=back[j];x1,y1=back[j+1];d.line([(x0,y0),(x1,y1)],fill=G1,width=2)
  peaks=[(0,443),(96,370),(160,420),(250,298),(331,392),(442,339),(535,412),(671,280),(759,382),(845,347),(960,428),(960,H),(0,H)]
  d.polygon(peaks,fill=WH)
  d.line(peaks[:-2],fill=WH,width=4)
  for px,py in [(96,370),(250,298),(442,339),(671,280),(845,347)]:
   d.line([(px,py),(px-27,py+57)],fill=BG,width=6);d.line([(px,py),(px+20,py+33)],fill=G3,width=4)
  d.polygon([(0,507),(178,450),(301,519),(520,458),(716,530),(835,469),(960,516),(960,540),(0,540)],fill=BG)
  for y in range(483,538,9):d.line([(0,y),(W,y)],fill=G3,width=1)
 elif name=='02-contour':
  starfield(d,2,150)
  horizon=305
  for i in range(27):
   y0=120+i*13
   pts=[]
   for x in range(0,W+5,4):
    off=70*math.sin((x/W)*math.pi*1.4+0.4)+(40+i*1.4)*math.sin((x/W)*math.pi*2.1-1.3)
    y=int(y0+off*(1-(i/40)))
    pts.append((x,y))
   d.line(pts,fill=WH if i%4==0 else G2,width=2 if i%4==0 else 1)
  # high-density underside white scanlines fading into blank black
  for k in range(31):
   y=int(420+k*3);x0=int(190+math.sin(k)*8)
   d.line((x0,y,min(W,795+k*3),y),fill=G3 if k%3 else G2,width=1)
 elif name=='03-night-city':
  starfield(d,3,150)
  d.ellipse((748,75,842,169),fill=WH)
  d.ellipse((731,67,818,155),fill=BG) # crescent moon
  rnd=random.Random(423)
  d.line((0,392,W,392),fill=G1,width=3)
  x=8
  while x<W:
   bw=rnd.choice([27,35,43,53,59]);top=rnd.randint(202,395)
   if rnd.random()<.12:top-=60
   d.rectangle((x,top,x+bw,490),fill=G1 if rnd.random()<.11 else G3,outline=WH,width=2)
   if rnd.random()<.25:d.line((x+bw//2,top-16,x+bw//2,top),fill=WH,width=2)
   for xx in range(x+7,x+bw-4,10):
    for yy in range(top+9,488,12):
     if rnd.random()<.62:d.rectangle((xx,yy,xx+3,yy+5),fill=WH if rnd.random()<.6 else G2)
   x+=bw+rnd.randint(5,11)
  d.rectangle((0,494,W,540),fill=BG)
  for y in range(494,540,7):
   d.line((0,y,W,y),fill=G3,width=1)
   for x in range(0,W,17):
    if (x+y)%4==0:d.point((x,y),fill=G2)
 elif name=='04-grid-world':
  starfield(d,4,205)
  van=(W//2,285)
  for x in range(-800,1900,100):d.line((van[0],van[1],x,H),fill=G2,width=1)
  for t in [0,8,18,32,50,73,100,138,185,250]:
   y=van[1]+t
   if y<H:d.line((0,y,W,y),fill=G2 if t%2 else WH,width=1)
  mountains=[(0,310),(96,230),(170,302),(261,171),(354,284),(432,245),(520,312),(642,196),(731,280),(826,221),(960,310)]
  d.line(mountains,fill=WH,width=4)
  for i in range(1,len(mountains)-1,2):
   px,py=mountains[i];d.line([(px,py),(px-48,298),(px+42,298)],fill=G2,width=2)
  # core's central reticle / scanning prompt, subtle and consistent with app glyphs
  cx,cy=480,225
  for coords in [[(cx-38,cy-14),(cx-38,cy-38),(cx-14,cy-38)],[(cx+14,cy-38),(cx+38,cy-38),(cx+38,cy-14)],[(cx-38,cy+14),(cx-38,cy+38),(cx-14,cy+38)],[(cx+14,cy+38),(cx+38,cy+38),(cx+38,cy+14)]]:
   d.line(coords,fill=WH,width=2)
 return im

for name in ['01-moonrise','02-contour','03-night-city','04-grid-world']:
 dark=wallpaper_scene(name)
 dark.resize((3840,2160),Image.Resampling.NEAREST).save(makepath(f'assets/wallpapers/4k/dark/{name}.png'),optimize=True)
 dark.resize((1920,1080),Image.Resampling.NEAREST).save(makepath(f'assets/wallpapers/1080p/dark/{name}.png'),optimize=True)
 light=Image.eval(dark,lambda v:255-v)
 light.resize((3840,2160),Image.Resampling.NEAREST).save(makepath(f'assets/wallpapers/4k/light/{name}.png'),optimize=True)
 light.resize((1920,1080),Image.Resampling.NEAREST).save(makepath(f'assets/wallpapers/1080p/light/{name}.png'),optimize=True)

# Seamless 32px tile patterns and CSS-ready dithering patterns.
BAYER=[[0,8,2,10],[12,4,14,6],[3,11,1,9],[15,7,13,5]]
for name in ['checker','scanlines','bayer25','bayer50','bayer75','stipple','grid']:
 tile=Image.new('RGBA',(32,32),'black');d=ImageDraw.Draw(tile)
 for y in range(32):
  for x in range(32):
   if name=='checker' and (x//4+y//4)%2==0:d.point((x,y),fill='white')
   if name=='scanlines' and y%4==0:d.point((x,y),fill=P['charcoal'])
   if name.startswith('bayer') and BAYER[y%4][x%4]<(int(name[5:])/100)*16:d.point((x,y),fill='white')
   if name=='stipple' and (x*7+y*11)%23==0:d.point((x,y),fill='white')
   if name=='grid' and (x%16==0 or y%16==0):d.point((x,y),fill=P['charcoal'])
 tile.save(makepath(f'assets/patterns/{name}-32.png'))
 makepath(f'assets/patterns/{name}-32.svg').write_text(pixel_svg(tile))

# Five original pixel onboarding fallback screens, 1280x720, no fake OS screenshots.
HEAD='/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf'
BODY='/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf'
def draw_pixel_text(im,xy,string,scale=2,size=14,fg=WH,bold=False):
 font=ImageFont.truetype(HEAD if bold else BODY,size=size)
 bb=font.getbbox(string)
 low=Image.new('RGBA',(bb[2]-bb[0]+4,bb[3]-bb[1]+4),(0,0,0,0));dl=ImageDraw.Draw(low)
 dl.text((2-bb[0],2-bb[1]),string,font=font,fill=(*fg,255))
 high=low.resize((low.width*scale,low.height*scale),Image.Resampling.NEAREST)
 im.paste(high,xy,high)
 return high.width,high.height

def onboard(idx,title,subtitle,steps):
 im=Image.new('RGB',(1280,720),BG);d=ImageDraw.Draw(im)
 d.rectangle((24,24,1255,695),outline=WH,width=4)
 for y in range(28,110,10):
  for x in range(965,1250,10):
   if (x+y)%30==0:d.rectangle((x,y,x+2,y+2),fill=G3)
 draw_pixel_text(im,(60,60),'OPENCLICKY  /  SETUP',2,15,WH,True)
 draw_pixel_text(im,(60,130),f'0{idx} / 05',3,14,G2,True)
 draw_pixel_text(im,(60,220),title.upper(),2,23,WH,True)
 draw_pixel_text(im,(60,320),subtitle,2,14,G1,False)
 y=424
 for item in steps:
  d.rectangle((60,y+5,74,y+19),fill=WH)
  draw_pixel_text(im,(95,y),item,2,13,G1)
  y+=48
 d.line((60,625,1220,625),fill=G3,width=2)
 for j in range(5):d.rectangle((60+j*33,649,82+j*33,659),fill=WH if j+1<=idx else G3)
 m=resize_nearest(glyph('mic' if idx==4 else 'sparkle' if idx==5 else 'focus' if idx==3 else 'keyboard' if idx==2 else 'chat'),120)
 im.paste(m,(1030,462),m)
 im.save(makepath(f'assets/onboarding/onboarding-0{idx}.png'))

onboard(1,'HELLO, HUMAN. :)','A desktop assistant that shows its work.',['Screen-aware, when you allow it','Talk or type naturally','Open source from day one'])
onboard(2,'SET YOUR HOTKEY','Pick a shortcut that fits your workflow.',['Choose your own global shortcut','Hold to talk, release to send','Change it anytime in Settings'])
onboard(3,'POINT. EXPLAIN.','The assistant can highlight interface elements.',['Capture the current screen on request','Ask what something means','Follow the pixel target highlight'])
onboard(4,'SAY IT. TYPE IT.','Voice and typed prompts share the same context.',['Microphone only when permitted','See listening and processing states','Stop instantly with one click'])
onboard(5,'YOU ARE IN CONTROL','Open settings. Inspect actions. Revoke access.',['Approve sensitive desktop actions','Choose a cloud or local model','Keep permissions reversible'])

# Dedicated UI fallback graphics and cursor targets.
for key in ['empty-note','connector-placeholder','target-reticle','permission-shield']:
 z=Image.new('RGBA',(64,64),(0,0,0,0));d=ImageDraw.Draw(z)
 if key=='empty-note':
  d.rectangle((10,7,54,56),outline='white',width=2);d.line((17,20,46,20),fill='white',width=2);d.line((17,28,46,28),fill='white',width=2);d.line((17,36,35,36),fill='white',width=2)
 elif key=='connector-placeholder':
  d.rectangle((6,6,57,57),outline='white',width=2);d.line((16,32,46,32),fill='white',width=2);d.line((32,16,32,46),fill='white',width=2)
 elif key=='target-reticle':
  for pts in [[(6,22),(6,6),(22,6)],[(42,6),(58,6),(58,22)],[(6,42),(6,58),(22,58)],[(42,58),(58,58),(58,42)]]:d.line(pts,fill='white',width=2)
  d.line((29,32,35,32),fill='white',width=2);d.line((32,29,32,35),fill='white',width=2)
 else:
  d.polygon([(32,4),(52,12),(49,42),(32,59),(15,42),(12,12)],outline='white',width=2)
  d.line((23,32,29,38,42,23),fill='white',width=3)
 makepath(f'assets/ui-illustrations/{key}.svg').write_text(pixel_svg(z))
 resize_nearest(z,256).save(makepath(f'assets/ui-illustrations/{key}-256.png'))

print('STEP 2: 4K/1080p wallpapers, 1-bit patterns, onboarding, UI graphics done')

# --- Original original retro/minimal sound palette, generated from synthesis (no third-party recordings).
# Slot names are reused from the user's original Clicky replacement inventory.
import wave
if (SRC/'OpenClicky_Design_System_v2.zip').exists():
 v2=zipfile.ZipFile(SRC/'OpenClicky_Design_System_v2.zip')
 filenames=sorted({n[n.index('/audio/')+7:] for n in v2.namelist() if '/audio/' in n and n.endswith(('.wav','.mp3'))})
else:
 with (ROOT/'assets/audio/audio-slots.csv').open() as _slotf:
  filenames=sorted(row['slot'] for row in csv.DictReader(_slotf))
assert len(filenames)==82,('audio count',len(filenames))
AUDIO_SLOTS=[]
for rel in filenames:
 name=Path(rel).name;folder=Path(rel).parent.as_posix()
 if folder=='ambient':dur=2975
 elif name.startswith('agent-'):dur=1000
 elif name.startswith('badge-'):dur=300
 elif name.startswith('boing-'):dur=450
 elif name.startswith('build-'):dur=300
 elif name.startswith('button-'):dur=400
 elif name=='clicky-question.wav':dur=850
 elif name=='clicky-surprised.wav':dur=500
 elif name.startswith('clicky-text-'):dur={'open':700,'close':850,'send':500,'receive':700}.get(name.split('-')[-1].split('.')[0],700)
 elif name=='connection-question.wav':dur=1500
 elif name.startswith('copy-'):dur=300
 elif name.startswith('ding-'):dur=800
 elif name=='enter.mp3':dur=120
 elif name.startswith('gel-pop-'):dur=350
 elif name=='hatching.wav':dur=3958
 elif name.startswith('highlighter-'):dur=900
 elif name=='home-reveal.wav':dur=1760
 elif name.startswith('nope-'):dur=550
 elif name.startswith('page-clear-'):dur=750
 elif name.startswith('paper-place-'):dur=700
 elif name.startswith('pen-flick-'):dur=250
 elif name.startswith('photo-drop-'):dur=450
 elif name=='reveal-boot.wav':dur=1360
 elif name.startswith('skill-'):dur=750 if 'down' in name else 790
 elif name.startswith('slide-'):dur=450
 elif name.startswith('sparkle-'):dur=800
 elif name.startswith('stamp-'):dur=400
 elif name.startswith('sticky-'):dur=400
 elif name.startswith('tada-'):dur=1000
 elif name.startswith('tapback-'):dur=497
 elif name.startswith('tape-'):dur=540
 elif name.startswith('target-'):dur=700
 elif name.startswith('whoosh-'):dur=600
 elif name=='eshop.mp3':dur=1680
 else:dur=500
 AUDIO_SLOTS.append((rel,dur))

def gen_sound(filename,ms):
 seed=int(hashlib.md5(filename.encode()).hexdigest()[:8],16);rnd=np.random.default_rng(seed)
 fs=44100;N=max(64,round(fs*ms/1000));T=N/fs;t=np.arange(N,dtype=np.float64)/fs
 tag=Path(filename).stem
 fam=hashlib.md5(tag.encode()).digest();f0=320+(fam[0]%12)*36
 if filename.startswith('ambient/'):
  # Periodic low-level waves + lowpassed granular texture: exact period, almost seamless.
  cyc1=int(f0*ms/1000)//3; cyc2=cyc1*2
  sig=.55*np.sin(2*np.pi*cyc1*np.arange(N)/N)+.22*np.sin(2*np.pi*cyc2*np.arange(N)/N)
  points=rnd.normal(size=1+round(ms/30))
  xp=np.linspace(0,N,len(points));grain=np.interp(np.arange(N),xp,points)
  ramp=np.sin(np.pi*np.arange(N)/N)**2
  sig+=.15*grain*ramp
  sig*=.12
  fade=int(fs*.006);sig[:fade]*=np.linspace(0,1,fade);sig[-fade:]*=np.linspace(1,0,fade)
 else:
  sweep=(-.35 if 'down' in tag or 'close' in tag or 'nope' in tag else .25 if 'up' in tag or 'appear' in tag or 'launch' in tag else 0)
  freq=f0*(1+sweep*np.exp(-t*14))
  phase=2*np.pi*np.cumsum(freq)/fs
  sq=np.sign(np.sin(phase));
  # Rounded, console-like square core, tempered with sine for low fatigue.
  sig=.59*np.sin(phase)+.27*sq+.09*np.sin(2*phase)
  env=(1-np.exp(-t*230))*(np.exp(-t/(max(.08,ms/2100))))
  if 'sparkle' in tag or 'ding' in tag or 'tada' in tag:
   sig+=.12*np.sin(phase*1.5)*np.exp(-t*4)
  if 'whoosh' in tag or 'slide' in tag or 'page-clear' in tag:
   noise=rnd.normal(size=N); filt=np.cumsum(noise);filt=(filt-np.mean(filt))/(np.max(np.abs(filt))+1e-6)
   sig=.28*sig+.72*filt
  if 'type-' in tag or 'pen-' in tag or 'copy-' in tag or 'clicky-text-send' in tag:
   env*=np.maximum(0,np.cos(2*np.pi*t*9))**2
  if 'nope' in tag:freq2=f0*.62;sig=.5*sig+.35*np.sin(2*np.pi*freq2*t)
  sig*=env
  fade=int(fs*.016 if ms>=300 else fs*.009)
  sig[:fade]*=np.linspace(0,1,fade)
  sig[-fade:]*=np.linspace(1,0,fade)
  sig*=.23
 sig-=np.mean(sig)
 peak=np.max(np.abs(sig))
 if peak>.68:sig=sig*(.68/peak)
 samples=np.clip(sig,-1,1)
 return (samples*32767).astype('<i2'),fs

def write_wav(path,data,fs):
 with wave.open(str(path),'wb') as wav:
  wav.setnchannels(1);wav.setsampwidth(2);wav.setframerate(fs);wav.writeframes(data.tobytes())

for rel,ms in AUDIO_SLOTS:
 audio,rate=gen_sound(rel,ms)
 dst=makepath('assets/audio/'+rel)
 if dst.suffix=='.wav':write_wav(dst,audio,rate)
 else:
  tmp=dst.with_suffix('.wav');write_wav(tmp,audio,rate)
  cp=subprocess.run(['ffmpeg','-v','error','-y','-i',str(tmp),'-codec:a','libmp3lame','-qscale:a','4',str(dst)],capture_output=True)
  if cp.returncode:raise RuntimeError(cp.stderr.decode())
  tmp.unlink()
makepath('assets/audio/audio-slots.csv').write_text('slot,duration_ms,format,notes\n'+''.join(f'{name},{ms},{Path(name).suffix[1:]},original pixel-synth; no licensed samples\n' for name,ms in AUDIO_SLOTS))

print('STEP 3: original 82 custom pixel-theme sound files generated')

# --- Token exports (JSON/CSS/TS): one source of truth for desktop and website.
T={
 'name':'OpenClicky Pixel Design System', 'version':'1.0.0',
 'identity':{'mark':':)','style':'monochrome pixel','approvedReference':'references/approved-design-sheet.png'},
 'primitives':{'black':'#000000','ink':'#080808','deep':'#141414','charcoal':'#404040','mid':'#A0A0A0','silver':'#E5E5E5','white':'#FFFFFF'},
 'palettes':{
  'dark':{'canvas':'#000000','surface':'#080808','raised':'#141414','foreground':'#FFFFFF','secondary':'#E5E5E5','muted':'#A0A0A0','border':'#404040','borderStrong':'#FFFFFF','selectedBackground':'#FFFFFF','selectedForeground':'#000000','disabled':'#404040'},
  'light':{'canvas':'#FFFFFF','surface':'#FFFFFF','raised':'#E5E5E5','foreground':'#000000','secondary':'#404040','muted':'#404040','border':'#A0A0A0','borderStrong':'#000000','selectedBackground':'#000000','selectedForeground':'#FFFFFF','disabled':'#A0A0A0'},
  'highContrast':{'canvas':'#000000','surface':'#000000','raised':'#000000','foreground':'#FFFFFF','secondary':'#FFFFFF','muted':'#FFFFFF','border':'#FFFFFF','borderStrong':'#FFFFFF','selectedBackground':'#FFFFFF','selectedForeground':'#000000','disabled':'#FFFFFF'},
  'dither':{'canvas':'#000000','surface':'#080808','raised':'#141414','foreground':'#FFFFFF','secondary':'#E5E5E5','muted':'#A0A0A0','border':'#404040','borderStrong':'#FFFFFF','selectedBackground':'#FFFFFF','selectedForeground':'#000000','disabled':'#404040','pattern':'../assets/patterns/bayer25-32.png'}
 },
 'typography':{'display':{'family':'Press Start 2P','fallback':'monospace','sizes':[12,16,20,24,32],'lineHeight':1.45},'ui':{'family':'VT323','fallback':'monospace','sizes':[18,22,26,30],'lineHeight':1.1},'body':{'family':'IBM Plex Mono','fallback':'monospace','sizes':[12,14,16],'lineHeight':1.55},'fontFilesBundled':False},
 'geometry':{'gridPx':4,'pixelGrid':{'glyph':16,'appIcon':64,'illustration':960},'strokePx':[1,2,4],'chamferPx':[4,8,12],'radiiPx':[0],'shadow':'4px 4px 0 #404040','antiAlias':'off for pixel art; on for regular body text'},
 'spacePx':{'xxs':4,'xs':8,'sm':12,'md':16,'lg':24,'xl':32,'xxl':48,'section':64},
 'componentSizesPx':{'appIcon':{'logical':64,'bundleMax':1024},'systemIcon':[16,24,32],'trayIcon':{'pt':22,'retinaPx':44},'pill':{'width':320,'height':48,'minWidth':180},'notchCollapsed':{'width':380,'height':48},'notchExpanded':{'width':420,'height':640},'edgeJobCard':{'width':300,'height':96},'noteCard':{'width':420},'button':{'sm':32,'md':40,'lg':48},'touchTarget':44,'overlayTargetBorder':2},
 'motion':{'tapMs':80,'hoverMs':100,'panelMs':160,'stateMs':120,'easing':'steps(3,end)','reducedMotion':'no steps, no looping, still clear'},
 'audio':{'format':'WAV 44.1kHz mono 16-bit; enter and eshop MP3','maxPeakDbFS':-3,'uiGain':0.7,'agentGain':0.8,'ambientGain':0.35,'voiceDucksAmbientTo':0.3,'maxConcurrency':'1 UI + 1 bed + 1 voice'},
 'screenOverlay':{'default':'pixel-border','strokePx':2,'halo':'none','ditheredFill':True},
 'rules':{'color':'Grayscale only; no accent hues','shadows':'hard-edged only; never blurred','gradients':'Dither ordered textures, never smooth gradients','icons':'Pixel aligned, no anti-aliasing, preserve integer scaling','accessibility':'Do not use sound alone; visible focus; reduced motion; voice always optional'}
}
makepath('tokens/tokens.json').write_text(json.dumps(T,indent=2)+'\n')
makepath('tokens/tokens.ts').write_text('/* Generated OpenClicky monochrome tokens, no runtime dependencies. */\nexport const openclickyTokens = '+json.dumps(T,indent=2)+' as const;\nexport type OpenClickyTheme = keyof typeof openclickyTokens.palettes;\n')

css=['/* OpenClicky Pixel v1: tokens and reusable visual rules. */', ':root {', '  --oc-grid: 4px;', '  --oc-font-display: "Press Start 2P", monospace;', '  --oc-font-ui: "VT323", monospace;', '  --oc-font-body: "IBM Plex Mono", monospace;', '  --oc-size-pill-h: 48px;', '  --oc-chamfer: 8px;', '  --oc-ease: steps(3,end);', '  --oc-duration-fast: 80ms;', '  --oc-duration-medium: 160ms;', '  --oc-shadow: 4px 4px 0 var(--oc-border);', '}']
for name,pl in T['palettes'].items():
 selector=':root,[data-oc-theme="dark"]' if name=='dark' else '[data-oc-theme="'+name+'"]'
 css.append(selector+' {')
 for k,v in pl.items():
  if k!='pattern':css.append('  --oc-'+re.sub('([A-Z])',lambda m:'-'+m.group(1).lower(),k)+': '+v+';')
 css.append('}')
css+=['''
* { box-sizing:border-box; }
.oc-root { background:var(--oc-canvas); color:var(--oc-foreground); font-family:var(--oc-font-body); }
.oc-pixel-image { image-rendering:pixelated; image-rendering:crisp-edges; }
.oc-display { font-family:var(--oc-font-display); line-height:1.45; letter-spacing:-.015em; }
.oc-terminal { font-family:var(--oc-font-ui); font-size:26px; line-height:1.1; }
.oc-panel { background:var(--oc-surface); border:1px solid var(--oc-border-strong); padding:16px; clip-path:polygon(8px 0,calc(100% - 8px) 0,100% 8px,100% calc(100% - 8px),calc(100% - 8px) 100%,8px 100%,0 calc(100% - 8px),0 8px); }
.oc-button { display:inline-flex;align-items:center;gap:8px;min-height:40px;padding:0 16px;background:var(--oc-selected-background); color:var(--oc-selected-foreground);border:2px solid var(--oc-border-strong);font-family:var(--oc-font-body);font-size:14px;cursor:pointer;box-shadow:var(--oc-shadow);transition:transform var(--oc-duration-fast) var(--oc-ease); }
.oc-button:active { transform:translate(2px,2px);box-shadow:none; }
.oc-button--secondary { background:var(--oc-surface);color:var(--oc-foreground); }
.oc-button--ghost { border:2px dashed var(--oc-border);color:var(--oc-foreground);background:transparent;box-shadow:none; }
.oc-input { background:var(--oc-surface); color:var(--oc-foreground);border:2px solid var(--oc-border-strong);height:44px;padding:0 12px;outline:0;font-family:var(--oc-font-body);font-size:14px; }
.oc-input:focus-visible,.oc-button:focus-visible,.oc-interactive:focus-visible {outline:2px dashed var(--oc-foreground);outline-offset:4px;}
.oc-badge { border:1px solid var(--oc-foreground); padding:4px 8px; font:12px var(--oc-font-body);text-transform:uppercase; }
.oc-key { border:1px solid var(--oc-border-strong); border-bottom-width:3px; padding:3px 6px; font-size:12px; }
.oc-status-active { outline:2px dashed var(--oc-foreground);outline-offset:3px; }
.oc-dither { background-image:url('../assets/patterns/bayer25-32.png');background-size:32px 32px;image-rendering:pixelated; }
.oc-wave { display:flex;align-items:center;justify-content:center;gap:4px;height:28px; }
.oc-wave span { display:block;width:3px;background:var(--oc-foreground);height:8px;animation:oc-wave 600ms steps(3,end) infinite alternate; }
.oc-wave span:nth-child(2n) { animation-delay:100ms; }
.oc-wave span:nth-child(3n) { animation-delay:200ms; }
@keyframes oc-wave { from {height:4px;} to {height:24px;} }
@media (prefers-reduced-motion:reduce) { .oc-root * {animation:none!important;transition:none!important;} }
''' ]
makepath('tokens/openclicky.css').write_text('\n'.join(css))
for th in T['palettes']:
 defn=T['palettes'][th]
 makepath(f'tokens/palette-{th}.json').write_text(json.dumps(defn,indent=2)+'\n')

# Compatibility-friendly names for known image slots.
for n in [16,32,64,128,256,512,1024]:shutil.copy2(ROOT/f'assets/app-icons/png/default-{n}.png',makepath(f'assets/compat/app-icon-{n}.png'))
shutil.copy2(ROOT/'assets/app-icons/png/default-1024.png',makepath('assets/compat/AppIcon.png'))
shutil.copy2(ROOT/'assets/platform/macos/tray-template-22.png',makepath('assets/compat/tray-template.png'))
for n in (16,32,48,128):shutil.copy2(ROOT/f'assets/platform/browser/extension-icon-{n}.png',makepath(f'assets/compat/ext-icon-{n}.png'))
for n in (16,32):
 for suf in ('','-off'):shutil.copy2(ROOT/f'assets/platform/browser/toolbar-{n}{suf}.png',makepath(f'assets/compat/ext-toolbar-{n}{suf}.png'))

print('STEP 4: CSS, TS, JSON token exports, compatibility assets done')

# --- Local interactive showroom (no build step, no network required).
preview_html=r'''<!doctype html>
<html lang="en" data-oc-theme="dark"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark light"><title>OpenClicky :) / Pixel Design System</title>
<style>
@import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600;700&family=Press+Start+2P&family=VT323&display=swap');
:root{--black:#000;--white:#fff;--grey:#a0a0a0;--line:#404040;--raised:#141414;--surface:#080808;--bg:#000;--fg:#fff;--select:#fff;--text-inverse:#000;--sans:'IBM Plex Mono','Courier New',monospace;--pixel:'Press Start 2P',monospace;--terminal:'VT323',monospace}
:root[data-oc-theme=light]{--bg:#fff;--fg:#000;--select:#000;--text-inverse:#fff;--surface:#fff;--raised:#e5e5e5;--line:#a0a0a0;--grey:#404040}
*{box-sizing:border-box}html{background:var(--bg)}body{margin:0;color:var(--fg);background:var(--bg);font-family:var(--sans);font-size:14px;line-height:1.6;min-height:100vh}button,input{font:inherit}button{cursor:pointer}button:focus-visible,input:focus-visible,a:focus-visible{outline:2px dashed var(--fg);outline-offset:4px}a{color:inherit}small{color:var(--grey)}.pixel{image-rendering:pixelated}.layout{max-width:1460px;margin:auto;padding:24px}.frame{border:2px solid var(--fg);clip-path:polygon(10px 0,calc(100% - 10px) 0,100% 10px,100% calc(100% - 10px),calc(100% - 10px) 100%,10px 100%,0 calc(100% - 10px),0 10px)}
header{display:flex;align-items:center;gap:24px;padding:18px 24px;flex-wrap:wrap}.brand{font-family:var(--pixel);font-weight:400;font-size:22px;letter-spacing:-1px;line-height:1.6}.vsep{width:1px;height:44px;background:var(--line)}.headnote{font-size:12px;text-transform:uppercase;letter-spacing:2px}.headnote strong{color:var(--fg)}.headnote span{display:block;letter-spacing:0;text-transform:none;color:var(--grey)}nav{display:flex;gap:6px;margin-left:auto;flex-wrap:wrap}nav button{border:1px solid transparent;background:transparent;color:var(--fg);padding:9px 12px}.on{color:var(--text-inverse)!important;background:var(--select)!important}.page{padding-top:34px}.eyebrow{color:var(--grey);letter-spacing:2px;text-transform:uppercase;font-size:11px}.headline{font-family:var(--pixel);font-size:20px;line-height:1.65;margin:8px 0 14px}.subtext{color:var(--grey);max-width:700px;font-size:13px}.controls{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.btn{border:2px solid var(--fg);background:var(--select);color:var(--text-inverse);padding:11px 18px;min-height:42px}.btn.outline{background:transparent;color:var(--fg)}.btn.dotted{border-style:dashed;background:transparent;color:var(--fg)}.btn:active{transform:translate(2px,2px)}.tabs{display:flex;gap:8px;margin-bottom:20px}.tab{padding:6px 10px;background:none;border:1px solid var(--line);color:var(--fg)}.tab.active{border:1px solid var(--fg);background:var(--select);color:var(--text-inverse)}
.grid{display:grid;grid-template-columns:1fr 1fr;gap:18px}.panel{border:1px solid var(--line);background:var(--surface);padding:22px;min-width:0}.panel h2{font-family:var(--pixel);font-size:12px;margin:0 0 22px;line-height:1.6}.panel h2 span{font:13px var(--sans);color:var(--grey);letter-spacing:1px}.hero{display:grid;grid-template-columns:240px 1fr;gap:28px;align-items:center;min-height:250px;border-bottom:1px solid var(--line);padding:0 0 28px;margin-bottom:26px}.heroIcon{width:220px;max-width:100%;filter:contrast(1.04)}.hero h1{font-size:clamp(24px,3.6vw,52px);font-family:var(--pixel);line-height:1.45;margin:5px 0 15px}.hero h1 span{color:var(--grey)}.hero p{font-size:14px;color:var(--grey)}.hero .pill{display:inline-block;margin-top:16px;border:1px solid var(--fg);padding:5px 9px;font-size:11px}
.swatches{display:grid;grid-template-columns:repeat(5,1fr);gap:8px}.swatch{aspect-ratio:1.7;border:1px solid var(--line)}.swatchwrap{font-size:11px}.swatchwrap code{font-size:10px}.swatchwrap b{display:block;margin-top:7px}.icons{display:grid;grid-template-columns:repeat(4,1fr);gap:12px}.icCard{min-width:0;text-align:center;cursor:pointer}.icCard img{display:block;width:100%;max-width:130px;aspect-ratio:1;margin:auto;image-rendering:pixelated}.icCard b{display:block;margin-top:8px}.icCard small{display:block;font-size:10px}.icCard[aria-selected=true] img{outline:2px dashed var(--fg);outline-offset:4px}.system{display:grid;grid-template-columns:repeat(8,1fr);gap:14px}.sys{text-align:center;font-size:10px}.sys img{display:block;width:32px;height:32px;margin:0 auto 7px;image-rendering:pixelated}.micStates{display:flex;gap:14px;flex-wrap:wrap}.micbtn{background:none;color:var(--fg);border:1px solid var(--line);padding:10px;text-align:center;width:100px}.micbtn img{display:block;width:64px;height:64px;margin:auto;image-rendering:pixelated}.micbtn.active{border-color:var(--fg);outline:2px dashed var(--fg);outline-offset:3px}.micbtn span{display:block;font-size:11px;margin-top:7px}
.notchDemo{background:#000;border:1px solid #404040;min-height:150px;display:flex;align-items:flex-start;justify-content:center;padding-top:20px;position:relative;overflow:hidden}.notchDemo:after{content:'';position:absolute;left:0;bottom:0;width:100%;height:53px;background-image:url('../assets/patterns/bayer25-32.png');opacity:.35}.notch{display:flex;align-items:center;gap:17px;border:2px solid #fff;padding:8px 18px;background:#000;color:#fff;border-radius:0;z-index:1;min-width:300px;justify-content:space-between}.notch span{font-family:var(--pixel);font-size:15px}.notch #notchStatus{font:12px var(--sans);color:#e5e5e5}.bars{height:24px;display:flex;align-items:center;gap:3px}.bars i{width:3px;height:9px;background:#fff;display:block}.bars i:nth-child(2n){height:16px}.bars i:nth-child(3n){height:22px}.waveon i{animation:bar 500ms steps(3,end) infinite alternate}.waveon i:nth-child(2n){animation-delay:130ms}.waveon i:nth-child(3n){animation-delay:200ms}@keyframes bar{from{height:4px}to{height:23px}}
.palettecopy{border:none;background:none;text-align:left;color:var(--fg);padding:0;display:block}.palettecopy:hover{text-decoration:underline}.uirow{display:flex;gap:10px;flex-wrap:wrap;align-items:center}.textbox{border:2px solid var(--fg);height:44px;background:var(--surface);color:var(--fg);padding:0 12px;min-width:220px;flex:1}.badge{border:1px solid var(--fg);padding:3px 8px;font-size:10px}.code{background:var(--raised);border:1px solid var(--line);padding:16px;overflow:auto;font-size:12px;white-space:pre-wrap;color:var(--fg)}kbd{padding:3px 7px;border:1px solid var(--fg);border-bottom-width:3px}.wallgrid{display:grid;grid-template-columns:repeat(4,1fr);gap:10px}.wallgrid img{width:100%;border:1px solid var(--line);image-rendering:pixelated}.wallgrid small{display:block}.section{margin-top:22px}.labelrow{display:flex;align-items:center;justify-content:space-between;gap:20px}.tiny{font-size:11px}.footer{border-top:1px solid var(--line);margin-top:36px;padding:22px 0;display:flex;justify-content:space-between;color:var(--grey);gap:16px;flex-wrap:wrap}.dither-mode{background-image:url('../assets/patterns/stipple-32.png');background-size:32px}
@media(max-width:890px){.grid{grid-template-columns:1fr}.hero{grid-template-columns:140px 1fr}.heroIcon{width:140px}.system{grid-template-columns:repeat(6,1fr)}.wallgrid{grid-template-columns:repeat(2,1fr)}.brand{font-size:15px}}@media(max-width:520px){.layout{padding:12px}.hero{grid-template-columns:1fr}.hero h1{font-size:18px}.icons{grid-template-columns:repeat(2,1fr)}.system{grid-template-columns:repeat(4,1fr)}.swatches{grid-template-columns:repeat(3,1fr)}header{padding:13px}.vsep{display:none}.panel{padding:14px}}
@media(prefers-reduced-motion:reduce){*,*:before,*:after{animation:none!important;scroll-behavior:auto!important;transition:none!important}}
</style></head>
<body><div class="layout"><header class="frame"><div class="brand">OpenClicky :)</div><div class="vsep"></div><div class="headnote"><strong>DESIGN SYSTEM</strong> [v1.0]<span>Pixel · Monochrome · Desktop AI Assistant</span></div><nav><button class="on" onclick="go('style',this)">Style</button><button onclick="go('icons',this)">Icons</button><button onclick="go('components',this)">Components</button><button onclick="go('assets',this)">Assets</button></nav></header>
<main class="page"><div class="labelrow"><span class="eyebrow">OPENCLICKY / PIXEL DESIGN SYSTEM / v1.0</span><div class="controls"><button class="tab active" id="darkbtn" onclick="theme('dark')">Dark</button><button class="tab" id="lightbtn" onclick="theme('light')">Light</button><button class="tab" id="ditherbtn" onclick="theme('dither')">Dither</button></div></div>
<div class="hero"><img class="heroIcon pixel" id="heroIcon" src="../assets/app-icons/png/default-256.png" alt="OpenClicky pixel icon"><div><div class="eyebrow">THE OPEN DESKTOP ASSISTANT</div><h1>YOUR COMPUTER.<br>YOUR RULES. <span>:)</span></h1><p>A native-feeling assistant with 1-bit character, strict monochrome design, and no visual noise.</p><span class="pill">macOS · Windows · open source</span></div></div>
<div class="grid"><section class="panel" id="icons"><h2>01 / APP ICONS <span>[4 VARIANTS]</span></h2><div class="icons" id="appIcons"></div></section><section class="panel" id="style"><h2>02 / COLOR PALETTE <span>[MONOCHROME]</span></h2><div class="swatches" id="swatches"></div><div class="section tiny">Pure black + white, with three defined greys. No chromatic accents.<br><span style="color:var(--grey)">Click a hex code to copy it.</span></div></section></div>
<div class="grid section"><section class="panel"><h2>03 / SYSTEM ICONS <span>[26 PIXEL GLYPHS]</span></h2><div class="system" id="systemIcons"></div></section><section class="panel"><h2>04 / MICROPHONE <span>[4 STATES]</span></h2><div class="micStates" id="micStates"></div><div class="section notchDemo"><div class="notch"><span>:)</span><div class="bars" id="barline"><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div><span id="notchStatus">IDLE</span></div></div></section></div>
<div class="grid section" id="components"><section class="panel"><h2>05 / TYPOGRAPHY <span>[PIXEL × MONO]</span></h2><div style="font-family:var(--pixel);font-size:20px;line-height:1.9">Aa 01 :)</div><p style="font-family:var(--pixel);font-size:12px;line-height:2">PRESS START 2P</p><p style="font-family:var(--terminal);font-size:33px;line-height:1.1;margin:0">VT323 — STATUS & TERMINAL</p><p style="font-family:var(--sans);font-size:13px;color:var(--grey)">IBM Plex Mono — readable paragraph and UI copy.</p></section><section class="panel"><h2>06 / UI COMPONENTS <span>[LIVE PREVIEW]</span></h2><div class="uirow"><button class="btn" onclick="playSound()">PRIMARY</button><button class="btn outline" onclick="playSound()">SECONDARY</button><button class="btn dotted">GHOST</button><span class="badge">READY</span></div><div class="section uirow"><input class="textbox" value="Ask OpenClicky..." aria-label="Sample prompt"><kbd>⌘</kbd><kbd>K</kbd><button class="btn outline" onclick="playSound()">↵</button></div><div class="section tiny" style="color:var(--grey)">2px outlines · 4px spacing · no round shadows · visible keyboard focus.</div></section></div>
<section class="panel section" id="assets"><h2>07 / WALLPAPERS <span>[4 SCENES × DARK & LIGHT × 4K / 1080P]</span></h2><div class="wallgrid" id="wallpaperCards"></div></section>
<div class="grid section"><section class="panel"><h2>08 / RULES <span>[NON-NEGOTIABLE]</span></h2><div class="code">COLOR    monochrome only\nGRID     4px layout / 16px glyph\nSTROKE   1 / 2 / 4px\nCORNERS  stepped, not rounded\nDITHER   ordered 4x4 Bayer\nMOTION   80 / 120 / 160ms, stepped\nSOUNDS   optional; no surprise autoplay\nTEXT     keep paragraph glyphs readable\nOS       native permission boundaries</div></section><section class="panel"><h2>09 / PLATFORM EXPORTS <span>[DESKTOP READY]</span></h2><div class="code">macOS   four .icns + .iconset\nWindows four .ico + tray variants\nWeb     favicon & extension PNGs\nNative  macOS dark/light templates\nUI      Tauri + React CSS tokens\nDocs    reference.md + integration guide\nAudio   82 newly synthesized soft retro SFX</div></section></div>
</main><footer class="footer"><span>OpenClicky :) — Pixel Design System v1.0</span><span>LOCAL DESIGN PREVIEW · ALL ASSETS INCLUDED IN THE ZIP</span></footer></div>
<script>
const rel='../';const icons=['default','light','outline','dither'];const glyphs=['mic','mic-off','sparkle','focus','cursor','layers','chat','waveform','settings','bolt','terminal','grid','sliders','sync','check','close','plus','arrow','eye','speaker','pause','stop','record','screen','lock','keyboard'];const colors=[['WHITE','#FFFFFF'],['SILVER','#E5E5E5'],['MID','#A0A0A0'],['CHARCOAL','#404040'],['BLACK','#000000']];let currentIcon='default';
appIcons.innerHTML=icons.map(k=>`<button class="icCard" style="background:none;color:var(--fg);border:0" aria-selected="${k==='default'}" onclick="selectIcon('${k}',this)"><img src="${rel}assets/app-icons/png/${k}-128.png" alt="${k} icon"><b>${k.toUpperCase()}</b><small>SVG · PNG · ICNS · ICO</small></button>`).join('');
swatches.innerHTML=colors.map(([name,hex])=>`<div class="swatchwrap"><div class="swatch" style="background:${hex}"></div><b>${name}</b><button class="palettecopy" onclick="copy('${hex}')"><code>${hex}</code></button></div>`).join('');
systemIcons.innerHTML=glyphs.map(k=>`<div class="sys"><img src="${rel}assets/system-icons/png/32/${k}.png" alt=""><span>${k}</span></div>`).join('');
micStates.innerHTML=['idle','listening','processing','muted'].map((k,i)=>`<button class="micbtn ${i===0?'active':''}" onclick="selectMic('${k}',this)"><img src="${rel}assets/microphone-states/png/${k}-64.png" alt="${k}"><span>${k.toUpperCase()}</span></button>`).join('');
wallpaperCards.innerHTML=['01-moonrise','02-contour','03-night-city','04-grid-world'].map(k=>`<div><img src="${rel}assets/wallpapers/1080p/dark/${k}.png" alt="${k} monochrome pixel wallpaper" loading="lazy"><small>${k.replace(/^\d\d-/, '').toUpperCase()} · 3840×2160</small></div>`).join('');
function selectIcon(k,el){currentIcon=k;document.querySelectorAll('.icCard').forEach(x=>x.setAttribute('aria-selected','false'));el.setAttribute('aria-selected','true');heroIcon.src=rel+'assets/app-icons/png/'+k+'-256.png'}
function selectMic(k,el){document.querySelectorAll('.micbtn').forEach(x=>x.classList.remove('active'));el.classList.add('active');notchStatus.textContent=k.toUpperCase();barline.classList.toggle('waveon',k==='listening'||k==='processing');if(k==='listening')playSound('agent/agent-launch.wav')}
function theme(k){let h=document.documentElement;h.dataset.ocTheme=k==='light'?'light':'dark';document.body.classList.toggle('dither-mode',k==='dither');['darkbtn','lightbtn','ditherbtn'].forEach(x=>document.getElementById(x).classList.toggle('active',x===k+'btn'));document.querySelectorAll('.wallgrid img').forEach(img=>img.src=img.src.replace(/\/(dark|light)\//,'/'+(k==='light'?'light':'dark')+'/'))}
function copy(txt){if(navigator.clipboard){navigator.clipboard.writeText(txt).catch(()=>{});}}
function playSound(file='ui/gel-pop-1.wav'){let a=new Audio(rel+'assets/audio/'+file);a.volume=.35;a.play().catch(()=>{})}
function go(id,el){document.querySelectorAll('nav button').forEach(x=>x.classList.remove('on'));el.classList.add('on');document.getElementById(id)?.scrollIntoView({behavior:'smooth',block:'center'})}
</script></body></html>'''
makepath('preview/index.html').write_text(preview_html)

# Template app shell implementation hint; link to the real audio/icons/colors.
makepath('examples/README.md').write_text('''# Integration examples\n\n- `preview/index.html`: open this directly for the interactive local style viewer.\n- `../tokens/openclicky.css`: load as shared Tauri/React CSS variables.\n- `../tokens/tokens.ts`: import TypeScript tokens.\n- `../assets/platform/macos/OpenClicky-default.icns`: macOS bundle icon.\n- `../assets/platform/windows/OpenClicky-default.ico`: Windows bundle icon.\n- `../assets/platform/macos/tray-template-22.png` & `...44.png`: monochrome menu bar templates.\n- `../assets/system-icons/svg`: use the SVG glyphs as `img` sources or React components.\n- `../assets/microphone-states/svg`: use the distinct states rather than recoloring one mark.\n- `../assets/audio`: user-toggleable short soft sounds; always default off in sensitive applications.\n\nAll colors come from `tokens/tokens.json`. No third-party font binaries are included.\n''')
print('STEP 5: interactive showroom and integration examples ready')

# --- Documentation, replacement mapping, reproducible implementation decisions.
STYLE=r'''# OpenClicky — Monochrome Pixel Design System

**Version:** 1.0.0  
**Identity:** `:)`  
**Platforms:** macOS and Windows (shared visual system; OS APIs remain native)  
**Source of visual direction:** approved black-and-white pixel style sheet, `references/approved-design-sheet.png`.

## 0. Single-sentence art direction

**A premium desktop assistant expressed in the language of an early computer: pure black/white, intentional grayscale and dithering, bitmap glyphs, precise grid alignment, and polished modern interactions.**

The nostalgic rendering method is intentional, not a reason to compromise interaction quality. It should feel like a design-forward current app rather than an emulated DOS terminal.

## 1. Never-do rules

- Never reintroduce purple, gold, rainbow accents, neon, glassmorphism, bloom or 3D beveled icons.
- Never use smooth color gradients; use **ordered 4×4 Bayer dithering** when grayscale transitions are required.
- Never use blurred drop shadows; an optional 4px hard offset is permitted.
- Never scale low-resolution icons with bicubic/bilinear interpolation: use nearest-neighbor or render from SVG at pixel grid boundaries.
- Never let decorative pixel type reduce legibility for body text, technical code or permissions.
- Never use prerecorded voice samples without provider licensing or the user's explicit choice.
- Never copy modern HeyClicky proprietary illustrations/videos. This is independently drawn OpenClicky artwork.

## 2. Exact color primitives (approved sheet)

| Token | Hex | Purpose |
|---|---|---|
| `--oc-black` | `#000000` | Primary canvas, text on white |
| `--oc-charcoal` | `#404040` | Muted borders, shadows, inactive dark areas |
| `--oc-mid` | `#A0A0A0` | Secondary content and 1-bit dithering transitions |
| `--oc-silver` | `#E5E5E5` | Secondary foreground / near-white surfaces |
| `--oc-white` | `#FFFFFF` | Text, glyphs, borders, high-contrast highlight |

Supporting near-black layout surfaces: `#080808` (surface), `#141414` (raised surface). These are grayscale only, never colored highlights.

### Semantic palettes

- **Dark — default:** `canvas #000000`, `surface #080808`, `raised #141414`, `fg #FFFFFF`, `muted #A0A0A0`, `border #404040`, selected bg `#FFFFFF`, selected fg `#000000`.
- **Light:** `canvas #FFFFFF`, `surface #FFFFFF`, `raised #E5E5E5`, `fg #000000`, `muted #404040`, `border #A0A0A0`, selected bg `#000000`, selected fg `#FFFFFF`.
- **High contrast:** strictly `#000000` / `#FFFFFF` foreground, background, borders, state indicators. No state conveyed by grayscale alone.
- **Dither:** same semantic colors as Dark; apply `assets/patterns/bayer25-32.png`, `bayer50-32.png`, or `bayer75-32.png` selectively as textures, never beneath dense body text.

Only these four themes are supported in v1.0. They are palettes, not brand variants: the same smiling brand symbol is used everywhere.

**Contrast:** White-on-black and black-on-white give 21:1; `#A0A0A0` on pure black is above common 4.5:1 normal-text requirements; `#404040` on white is high contrast. **Do not** use `#404040` as small text against black, or `#A0A0A0` as small text against white. Disabled content can be exempt from textual contrast requirements, but still needs other state cues.

## 3. Typography and licensing

| Use | Face | Size | Notes |
|---|---|---|---|
| Brand and display | **Press Start 2P** | 12/16/20/24/32px | Use sparingly; line-height 1.45, no cramped blocks |
| Status and expressive UI | **VT323** | 18/22/26/30px | Labels, badge counters, terminal output |
| Readable UI, code, paragraphs | **IBM Plex Mono** | 12/14/16px | Standard 14–16px, line-height 1.55 |
| OS-native fallback | `monospace` | ≥14px | Always work without online fonts |

Reference links:
- Press Start 2P: https://fonts.google.com/specimen/Press+Start+2P
- VT323: https://fonts.google.com/specimen/VT323
- IBM Plex Mono: https://fonts.google.com/specimen/IBM+Plex+Mono

**No font binary is included in this archive.** Add licensed font subsets to the app build only after reviewing and preserving the applicable redistribution licenses. The sample preview imports Google Fonts if available, and falls back offline.

## 4. Grids, spacing, corner shapes

### Layout grid

- Spacing unit: **4 CSS px**. Allowed rhythm: **4, 8, 12, 16, 24, 32, 48, 64**.
- Icon source grids: **16×16** system glyphs; **64×64** app icon art. All raster exports scale with nearest-neighbor.
- Pixel art may have stepped/chamfered 4/8/12px corners; **do not** apply 24px smooth rounding to pixel-art cards.
- Border weight: 1px substructure, 2px primary panel outlines and current focus, 4px special heavy divider.
- Keep high-DPI Windows/macOS scaling correct: CSS px for layout, pixel grid for image assets. At non-integer OS scaling, keep raster nearest and prefer pixel-aligned SVG.

### Layout presets

| UI element | Default | Notes |
|---|---|---|
| Tray/menubar | 22×22 pt / 44px @2x | black alpha mask template for macOS |
| Floating pill | 320×48px | can contract to 180px; system notch positions differ |
| Collapsed notch | 380×48px | macOS top-center optional; honor safe areas |
| Expanded assistant | 420×640px | scrollable; no forced screen obstruction |
| Agent edge job card | 300×96px | stack up to 3 before scrolling |
| Note card | 420px width | content height flexible |
| Chat message | ≤320px width | clear sender and time affordances |
| Button | 32/40/48px high | hit target at least 44×44 logical px where feasible |
| Screen target | 2px outline | pixel corners; avoid glow; add high-contrast arrows |
| Shortcut HUD | 8px padding increments | keyboard discoverable |

### Motion / transitions

- 80ms: button tap and key state.
- 100–120ms: hover and mic state changes.
- 160ms: panel open/close.
- `steps(3, end)` for cosmetic transitions, but **do not** apply stepped animation to real pointer tracking or screen reticle movement; tracking should remain responsive and accurate.
- For `prefers-reduced-motion` or native OS equivalent, disable decorative loops, background motion, and scrolling animations. Voice capture and focus indicators must still be perceptible.

## 5. Official logo: `:)`

The glyph is the brand. It is not a wordmark emoji. Do not substitute a yellow face, round eyes or smile emoji. The colon should consist of two distinctly separated square pixels/squares and the smile is a **right parenthesis**, not a curved cartoon mouth.

### Four approved application icons

1. **Default** — black field, white stepped frame, light 1-bit stippling in bottom-right, white `:)`.
2. **Light** — white field, black `:)` and minimal stepped footprint.
3. **Outline** — black field, thin white outline, white `:)`, minimal interior.
4. **Dither** — monochrome stippling texture and black `:)`; only five approved grey/white/black levels.

The icon variants are not separate platform brands: bundle the **Default** variant for production, with the other three kept for personalization, documentation, or an OS-aware alternative. The package includes Windows ICO and macOS ICNS for each variant.

**Small-size rule:** 16px and 32px glyphs use their own hand-aligned pixel shapes, so colon dots and smile do not collapse. Larger sizes use nearest-neighbor upscale from the 64px source.

## 6. System icon family

SVG and 16/24/32/48/64/128px PNG assets are included for: `mic`, `mic-off`, `sparkle`, `focus`, `cursor`, `layers`, `chat`, `waveform`, `settings`, `bolt`, `terminal`, `grid`, `sliders`, `sync`, `check`, `close`, `plus`, `arrow`, `eye`, `speaker`, `pause`, `stop`, `record`, `screen`, `lock`, `keyboard`.

- Each SVG uses a `16×16` logical viewBox, crisp rect/path geometry, and no Gaussian blur.
- Use `24px` for regular desktop UI, `16px` for dense menus, `32px` for primary controls.
- For interactive icons, add native `aria-label` or `accessibilityLabel` equivalents. Never label actions by graphic alone.
- Map glyph roles semantically: speaker for audio output, mic for input, lock for permissions, focus for screen-target selection.

## 7. Microphone states

| State | UI | What the user should perceive |
|---|---|---|
| Idle | base outlined microphone | Mic **not listening** |
| Listening | heavy frame, orbit ticks | Actively **capturing microphone** (consent required) |
| Processing | explicit waveform | Capture stopped, request being processed |
| Muted | crossed microphone | Microphone disabled; tap to unmute if authorized |

Use the four separate exported images, not color-shifted versions of the same image. All state changes must have a text label and an accessible announcement. Animations are decorative and can be disabled.

## 8. Wallpaper and pattern family

Four separately authored original scenes, each in Dark and Light at 3840×2160 (4K) and 1920×1080:

1. `01-moonrise` — moon, stars, mountain tiers.
2. `02-contour` — topographic contour wavefront.
3. `03-night-city` — high-contrast city skyline.
4. `04-grid-world` — perspective grid, wire mountains, focus marker.

`assets/patterns/`: checker, scanlines, Bayer 25/50/75%, stipple and grid. These are **tileable 32×32** PNG and SVG patterns. Use transparent overlays sparingly, never below text-heavy fields.

## 9. UI components

### Buttons

- **Primary:** white background with black text in Dark; invert in Light.
- **Secondary:** black/white outline and transparent interior.
- **Ghost:** dotted outline, no fill.
- **Danger:** maintain monochrome but require warning icon, descriptive text, confirmation step; do not imply danger with unsupported color hue.
- Pressed = 2px offset, shadow removed. Keyboard focus = 2px dashed visible outline offset by 4px.

### Inputs, toggles, sliders, dialogs

- Input `44px` height, 2px strong outline; focus must remain clear over dither textures.
- Toggle must contain on/off status text or explicit check glyph; do not communicate solely through position.
- Sliders: square handles, 2px track, keyboard arrow support.
- Toasts: short, text-first, icon-assisted; optional soft audio, never sound-only.
- Dialog: retain focus, explicit cancel, no click-through of controlled overlay while active.
- Notch: never overlap system menu items or cutouts; clamp to safe area and per-monitor work area.

### Screen capture and pointer overlays

- Use native OS permission prompts; do not draw fake permission dialogs.
- No permanent recording indicator unless actually capturing; always display capture state accurately.
- Overlay should be click-through except when user deliberately enters annotation/control mode.
- Reticles and screen coordinate pointers must be responsive, and target validation should use accessibility tree data and OCR when permitted. Accuracy matters more than animation.

## 10. Sound identity

This package includes **82 independently synthesized pixel/retro UI sound files**, using the exact legacy slot names in the supplied inventory where available. They are soft, minimal electronic cues, not borrowed HeyClicky recordings. Audio format: **44.1kHz, 16-bit PCM mono WAV**; the two legacy MP3 slots remain MP3.

- Peak is kept below -3dBFS with fades; final product must still be listened to on actual hardware before shipping.
- UI gain 0.7; agent gain 0.8; ambient gain 0.35; voice ducks ambient to 30%.
- Allow one UI sound, one ambient bed, and one voice stream simultaneously.
- Provide a **Sound Effects: Off / Quiet / Normal** setting and honor OS sound preferences.
- Voice samples for model providers are not generated or bundled: fetch from explicitly licensed voices/providers at runtime.

## 11. Platform integration

**macOS:** Use `assets/platform/macos/OpenClicky-default.icns` in the bundle; use `tray-template-22.png` / `tray-template-44.png` as menu-bar template images. On macOS explicitly mark menu-bar images as template, allowing the OS to tint them. Capture and control must use permitted ScreenCaptureKit, Accessibility, and event APIs.

**Windows:** Use `assets/platform/windows/OpenClicky-default.ico` as window/installer icon; use appropriate taskbar/tray icon with dark/light contrast. Screen capture uses Windows Graphics Capture, and input/accessibility interactions use Windows APIs with appropriate permission checks.

**Web/extension:** `assets/platform/browser/extension-icon-{16,32,48,128}.png`, plus toolbar variants. Extension icon size requirements can vary by browser store and manifest version.

**Shared React/Tauri:** Import `tokens/openclicky.css` and/or `tokens/tokens.ts`. SVG glyphs can be loaded as images or component imports; avoid raster scaling artifacts. Native API integration is separate from this asset pack.

## 12. Source, licenses, and release checklist

- Visual concept: generated in this conversation and approved as reference. Actual assets in the pack are newly constructed programmatically.
- Logo master: original pixel geometry; brand name OpenClicky, logo `:)`.
- Audio: procedural originals synthesized in `scripts/generate_assets.py` (no external samples).
- Third-party app/platform logos, copyrighted workflow screenshots, existing proprietary video assets, identities, and voice previews are **not re-created and not included**. Use licensed official marks or record new footage.
- Font links are references only; **no font files are bundled**.
- Run scripts or inspect packaged assets after any changes. Review accessibility, the app security model, at least 100/125/150% Windows scaling, macOS Retina, 16px logo readability, and animation preference support before release.

## 13. Directory reference

- `assets/app-icons` — 4 variants in SVG + 9 PNG sizes.
- `assets/platform/macos` — ICNS, iconsets, menu bar images.
- `assets/platform/windows` — ICO, native tray images.
- `assets/platform/browser` — extension/toolbar icons.
- `assets/system-icons` — 26 original monochrome glyphs.
- `assets/microphone-states` — idle/listening/processing/muted.
- `assets/wallpapers` — 4 scenes × 2 themes × 2 resolutions.
- `assets/patterns` — 7 tileable ordered pixel patterns.
- `assets/onboarding` — 5 designed illustrative fallbacks.
- `assets/ui-illustrations` — note, connector placeholder, reticle, permissions.
- `assets/audio` — 82 slot-compatible new SFX plus slot manifest.
- `assets/compat` — slot-named export copies.
- `tokens` — JSON, TypeScript, CSS palettes.
- `preview/index.html` — interactive design viewer.
- `preview/contact-sheet.png` — asset specimen image.
- `reference.md` — replacement guidance and missing-proprietary-content policy.
- `REPLACEMENT_MAP.csv` and `MANIFEST.csv` — per-file mapping and deliverable inventory.
- `scripts/generate_assets.py` — reproducible generator source.
'''
makepath('STYLE_GUIDE.md').write_text(STYLE)

# Original uploaded inventory is the mapping's authoritative slot-name source.
original_input=SRC/'Pasted text(20261105-050134).txt'
original=(original_input if original_input.exists() else ROOT/'references/original-asset-slots.md').read_text()
approved=SRC/'a_highly_detailed_pixel_8_bit_monochrome_black.png'
if approved.exists():shutil.copy2(approved,makepath('references/approved-design-sheet.png'))
makepath('references/original-asset-slots.md').write_text(original)

mapping=[]
def row(src,output,status,note):mapping.append((src,output,status,note))
for name,ms in AUDIO_SLOTS:row(name,'assets/audio/'+name,'supplied','original synthesized pixel sound; original legacy filename preserved; duration %dms'%ms)
for n in [16,32,64,128,256,512,1024]:row('app-icon-'+str(n)+'.png','assets/compat/app-icon-'+str(n)+'.png','supplied','Default brand variant, nearest-neighbor png; also alternate variants')
row('AppIcon','assets/compat/AppIcon.png','supplied','1024px brand master; source svg and all four variants under app-icons')
row('tray-template.png','assets/compat/tray-template.png','supplied','black-alpha macOS template; flag isTemplate=true in native UI')
for n in [16,32,48,128]:row('ext-icon-'+str(n)+'.png','assets/compat/ext-icon-'+str(n)+'.png','supplied','pixel-fidelity browser extension icon')
for n in [16,32]:
 for suf in ('','-off'):row('ext-toolbar-'+str(n)+suf+'.png','assets/compat/ext-toolbar-'+str(n)+suf+'.png','supplied','matching monochrome toolbar state')
row('heyclicky-logo','assets/brand/mark-white.svg','rebrand-replacement','replace all HeyClicky brand UI with new OpenClicky :) mark')
row('ClickyCharacters','assets/onboarding/onboarding-01.png','replacement-concept','new original onboarding image; not the same original mascot sprite')
for target,source in [('empty-note.png','assets/ui-illustrations/empty-note-256.png'),('connector-placeholder.png','assets/ui-illustrations/connector-placeholder-256.png')]:row(target,source,'supplied','monochrome original fallback')
for i in range(1,6):row('onboarding-%02d.png'%i,'assets/onboarding/onboarding-%02d.png'%i,'supplied','original 1280x720 illustrative fallback, not proprietary footage')
for name in ['01-moonrise','02-contour','03-night-city','04-grid-world']:
 row('wallpaper:'+name,'assets/wallpapers/4k/dark/'+name+'.png','supplied','native 3840x2160 and light/1080p alternates')
for k in GLYPHS:row('pixel-icon:'+k,'assets/system-icons/svg/'+k+'.svg','supplied','SVG master with PNG exports')
for k in MIC_STATES:row('mic-state:'+k,'assets/microphone-states/svg/'+k+'.svg','supplied','explicit separate microphone state graphic')
# Old first-party workflow screenshots cannot be truthfully replaced by invented captures.
for name in ['add-a-project-in-codex','add-a-project-in-codex-from-sidebar','codex-app-screenshot','codex-home-screen','codex-permissions','git-tools-prompt','inside-the-makesomething-project-folder','makesomething-project-folder-in-downloads','onboarding-google-unverified-warning','onboarding-keychain-allow-prompt']:
 row(name,'','capture-required','record a true screenshot of your own workflow; do not fabricate the OS/app UI')
thirdparty=['airtable-logo','apple-notes-logo','apple-reminders-logo','chatgpt-logo','claude-code-logo','claude-logo','discord-logo','excalidraw-logo','findmy-logo','github-logo','gmail-logo','google-logo','google-meet-logo','imessage-logo','instagram-logo','linear-logo','linkedin-logo','ms-teams-logo','notion-logo','obsidian-logo','openai-logo','polymarket-logo','powerpoint-logo','reddit-logo','spotify-logo','tiktok-logo','whatsapp-logo','word-logo','x-logo','youtube-logo','zoom-logo']
for name in thirdparty:row(name,'assets/ui-illustrations/connector-placeholder-256.png','placeholder-only','use licensed, official icon of actual connected service in product; not included')
for name in ['farza-avatar','steve','referral-sample-farza','referral-sample-kelli','referral-sample-nilesh']:
 row(name,'','not-included','identity-specific or testimonial photography; use own licensed images or remove')
for name in ['catmode-red-notch-atlas','catmode-red-overlay-atlas']:
 row(name,'','deliberately-not-recreated','old red style incompatible with strict monochrome; if feature retained, design new black/white animation')
for name in ['onboarding-intro-v2.mp4','paywall-intro-v2.mp4','clicky-drive-sample.mp4','clicky-spatial-demo.mp4']:
 row(name,'','production-required','capture original OpenClicky workflows; no proprietary HeyClicky footage included')
for name in ['voice-preview-* (13 files)','realtime-voice-preview-* (10 files)','gemini-voice-preview-* (30 files)']:
 row(name,'','provider-license-required',"generate samples at runtime through user's selected licensed voice/provider; not bundled")
for name in ['Barlow-Regular.ttf','Barlow-Medium.ttf','Barlow-SemiBold.ttf','PatrickHand.ttf']:
 row(name,'','style-replaced-no-font-files','new design suggests Press Start 2P, VT323, IBM Plex Mono; obtain fonts from publisher with license')
with makepath('REPLACEMENT_MAP.csv').open('w',newline='') as f:
 w=csv.writer(f);w.writerow(['original_slot','openclicky_asset','status','notes']);w.writerows(mapping)

reference=f'''# OpenClicky Pixel v1 — asset replacement reference

Source: attached user-provided slot inventory (`references/original-asset-slots.md`) and the user-approved monochrome pixel style sheet (`references/approved-design-sheet.png`).

**This is an original recreation, not a copy of the current proprietary HeyClicky app.** Existing filenames below are described only as integration/replacement targets.

## Exact role mapping

| Existing target | Use from this pack | Integration note |
|---|---|---|
| `app-icon-1024.png` | `assets/compat/app-icon-1024.png` | Canonical pixel Default `:)` app art |
| `app-icon-512.png` | `assets/compat/app-icon-512.png` | Finder/taskbar bitmap rendition |
| `app-icon-{{16,32,64,128,256}}.png` | `assets/compat/app-icon-<size>.png` | Multi-resolution fallback ladder |
| `AppIcon` | `assets/compat/AppIcon.png` | Source 1024×1024 brand PNG |
| `tray-template.png` | `assets/compat/tray-template.png` | Mark as macOS template image |
| `ext-icon-{{16,32,48,128}}.png` | `assets/compat/ext-icon-<size>.png` | Browser extension icons |
| `ext-toolbar-{{16,32}}[-off].png` | `assets/compat/ext-toolbar-<size>[-off].png` | Connected/disconnected toolbar |
| `heyclicky-logo` | `assets/brand/mark-white.svg` or `...mark-black.svg` | Rebrand to **OpenClicky** on all surfaces |
| Existing microphone artwork | `assets/microphone-states` | Four separate explicit states |
| Legacy UI icon set | `assets/system-icons` | 26 crisp editable system glyphs |
| Four background previews | `assets/wallpapers` | 4 new 4K black/white scenes + 4 inverted |
| `empty-note.png` | `assets/ui-illustrations/empty-note-256.png` | Original empty-state illustration |
| `connector-placeholder.png` | `assets/ui-illustrations/connector-placeholder-256.png` | Generic placeholder only; not an official mark |
| `onboarding-01..05.png` | `assets/onboarding` | New original static onboarding designs |
| 82 original legacy audio filenames | `assets/audio/<group>/<filename>` | 82 newly synthesized sounds, original names |
| `Barlow` / `PatrickHand` | Refer to `STYLE_GUIDE.md` | Suggest Press Start 2P / VT323 / IBM Plex Mono; font binaries not included |

## Not directly replaceable from a design pack

1. **Original HeyClicky video clips** (`onboarding-intro-v2.mp4`, `paywall-intro-v2.mp4`, demo samples): remake with actual OpenClicky UI recording, not a copied video.
2. **Provider voice sample libraries** (13+10+30 samples in the reference inventory): produce from permitted TTS providers at runtime. Speech cannot be accurately represented by synthesized UI sound effects.
3. **Third-party service logos** (OpenAI, GitHub, Gmail, etc.): retain properly sourced, licensed official assets only where relevant, otherwise use the placeholder.
4. **Screenshots of other applications and OS permission dialogs:** capture true screens from supported versions and permission states; never fabricate misleading OS images.
5. **Photos and people-specific testimonials:** do not invent or reuse likenesses; ask owners for rights or remove.
6. **Cat mode red atlases:** deliberately retired because the v1 theme is strictly grayscale; needs an independently authored monochrome animation if feature remains.

## What is actually supplied

- Four distinct app icon variants with ICNS and ICO exports.
- PNG compatibility ladder and SVG master art.
- 26 system glyphs in SVG plus six PNG dimensions each.
- Four microphone state graphics, four sizes each.
- Four themes via JSON, TypeScript and CSS tokens.
- 4 illustrations × 2 themes × 2 resolutions = 16 wallpaper PNGs.
- Seven reusable dithering, checkerboard and pixel grid patterns (PNG and SVG).
- Five 1280×720 onboarding static fallbacks.
- Four utility illustration templates.
- 82 original synthesized UI sounds.
- Local clickable sample design board and integration guides.

**See `REPLACEMENT_MAP.csv` for {len(mapping)} explicit slot mappings and statuses.** A blank destination in that CSV means the content is *not* included.
'''
makepath('reference.md').write_text(reference)
makepath('LICENSE-NOTES.md').write_text('''# Packaging and rights notes\n\nThe OpenClicky visual assets and synthesized SFX in this pack were independently generated for this project in this conversation. No existing HeyClicky binary icon, proprietary video, screenshot, voice recording, third-party logo, or font file has been copied into this pack.\n\nThird-party software names appear only as factual interoperability targets in `reference.md` and `REPLACEMENT_MAP.csv`. They are not endorsements.\n\nBefore public distribution, the project maintainers should choose an explicit repository/license policy and check any added third-party font, voice, screenshot, and integration art rights. This file is descriptive rather than a copyright license grant.\n''')
makepath('README.md').write_text('''# OpenClicky :) — Monochrome Pixel Design System v1.0\n\nThe production-oriented **black/white pixel** brand kit matching the approved visual sheet.\n\n## Start here\n\n1. Read `STYLE_GUIDE.md` (brand system, exact tokens, layout, mic states, accessibility and cross-platform rules).\n2. Open `preview/index.html` locally to browse the actual icons, wallpapers, components and theme switcher. The only optional network use is for web font imports; it falls back offline.\n3. Wire `tokens/openclicky.css` and `tokens/tokens.ts` into the Tauri frontend.\n4. Use `assets/platform/macos/OpenClicky-default.icns` and `assets/platform/windows/OpenClicky-default.ico` for installers.\n5. Read `reference.md` and `REPLACEMENT_MAP.csv` before replacing legacy HeyClicky resource slots.\n6. Use `assets/audio` for original soft pixel-themed cues, and provide controls to disable sounds.\n\n## Palette\n\n`#000000` · `#404040` · `#A0A0A0` · `#E5E5E5` · `#FFFFFF`\n\nSub-surface neutrals: `#080808`, `#141414`. No color gradients.\n\n## Build / audit\n\nThe programmatic construction source is `scripts/generate_assets.py`. Re-run using Python, Pillow, NumPy and FFmpeg. It regenerates everything in the same output folder in `/mnt/data`. `VALIDATION.txt` records integrity and size checks of the delivered run.\n\n## Not included\n\nModern HeyClicky proprietary assets, third-party copyrighted logos, actual voice samples, and app footage. See `reference.md` for honest statuses and replacements. No font binaries distributed.\n''')

print('STEP 6: detailed style guide, mapping, metadata, and licensing notes written')

# --- A true specimen board assembled from the REAL included assets (not a speculative mockup).
SW,SH=960,640
sheet=Image.new('RGB',(SW,SH),BG); sd=ImageDraw.Draw(sheet)
font=ImageFont.truetype(HEAD,14)
font10=ImageFont.truetype(BODY,10)
font11=ImageFont.truetype(BODY,11)
font12=ImageFont.truetype(HEAD,12)
font18=ImageFont.truetype(HEAD,18)
def tx(x,y,txt,color=WH,f=font10):sd.text((x,y),txt,font=f,fill=color)
def hr(y):sd.line((20,y,940,y),fill=G3,width=1)
def vr(x,y0,y1):sd.line((x,y0,x,y1),fill=G3,width=1)
def pill(x,y,w,h,fg=WH):
 sd.line([(x+5,y),(x+w-5,y),(x+w,y+5),(x+w,y+h-5),(x+w-5,y+h),(x+5,y+h),(x,y+h-5),(x,y+5),(x+5,y)],fill=fg,width=1)

def put_png(path,xy,size):
 im=Image.open(ROOT/path).convert('RGBA')
 im=im.resize(size,Image.Resampling.NEAREST)
 sheet.paste(im,xy,im)

pill(12,10,936,62);tx(28,27,'OpenClicky :)',WH,font18)
vr(285,20,62);tx(305,24,'DESIGN SYSTEM   [v1.0]',WH,font12)
tx(305,42,'PIXEL  /  MONOCHROME  /  DESKTOP AI',G2,font10)
for i,label in enumerate(['Style','Icons','UI','Assets']):
 x=[682,753,810,872][i]
 if i==0:sd.rectangle((x-5,27,x+49,51),fill=WH);tx(x+3,32,label,BG,font10)
 else:tx(x,32,label,WH,font10)

# Hero logo / variant tiles / exact palette.
tx(20,88,'01 / OFFICIAL MARK',WH,font12)
put_png('assets/app-icons/png/default-128.png',(35,115),(132,132))
tx(177,150,':)',WH,font18);tx(177,177,'PIXEL MARK',G2,font10);tx(177,196,'64px master',G2,font10)
vr(288,83,258)
tx(303,88,'02 / APP ICONS [4]',WH,font12)
for i,name in enumerate(['default','light','outline','dither']):
 x=303+i*86
 put_png(f'assets/app-icons/png/{name}-128.png',(x,129),(70,70))
 tx(x+2,216,name.upper(),WH,font10)
vr(658,83,258)
tx(673,88,'03 / MONOCHROME',WH,font12)
for i,(hex,color) in enumerate([('#FFFFFF',WH),('#E5E5E5',G1),('#A0A0A0',G2),('#404040',G3),('#000000',BG)]):
 x=681+i*51
 sd.rectangle((x,129,x+41,173),fill=color,outline=G2,width=1)
 tx(x-2,183,hex,WH,font10)
tx(680,210,'0 HUES / 5 TONES',G2,font10)
tx(680,227,'DARK · LIGHT · DITHER',G2,font10)
hr(267)
# System icons
tx(20,280,'04 / SYSTEM ICONS [26]',WH,font12)
for idx,name in enumerate(GLYPHS):
 col=idx%13;rowid=idx//13
 x=24+col*44;y=307+rowid*60
 put_png(f'assets/system-icons/png/32/{name}.png',(x+7,y),(32,32))
 tx(x+6,y+38,['MIC','MUTE','STAR','FOCS','PNTR','LAYR','CHAT','WAVE','SETT','BOLT','TERM','GRID','SLDR','SYNC','OK','EXIT','PLUS','NEXT','EYE','SPKR','PAUS','STOP','REC','SCRN','LOCK','KEYS'][idx],G1,font10)
vr(613,270,426)
tx(629,280,'05 / MICROPHONE STATES',WH,font12)
for idx,name in enumerate(MIC_STATES):
 x=636+idx*78
 put_png(f'assets/microphone-states/png/{name}-64.png',(x,318),(58,58))
 tx(x,386,name.upper(),WH,font10)
hr(427)

tx(20,441,'06 / WALLPAPERS [4 × 4K]',WH,font12)
for idx,name in enumerate(['01-moonrise','02-contour','03-night-city','04-grid-world']):
 x=20+idx*145
 im=Image.open(ROOT/f'assets/wallpapers/1080p/dark/{name}.png').convert('RGB').resize((132,74),Image.Resampling.NEAREST)
 sheet.paste(im,(x,474));sd.rectangle((x,474,x+132,548),outline=G2)
 tx(x,555,name[3:].upper()[:13],G1,font10)
vr(612,428,582)
tx(630,441,'07 / PILL / NOTCH',WH,font12)
pill(635,478,289,54)
tx(650,488,':)',WH,font18)
for i,h in enumerate([8,15,22,9,14,24,12,6,19]):
 sd.rectangle((751+i*9,505-h//2,754+i*9,505+h//2),fill=WH)
tx(865,497,'IDLE',WH,font10)
hr(585)
tx(20,600,'FONT  PRESS START 2P  ·  VT323  ·  IBM PLEX MONO',G1,font10)
tx(20,619,'PLATFORMS  macOS .icns  /  Windows .ico  /  Web PNG + SVG',G2,font10)
tx(694,600,'NO GLOW  ·  NO GRADIENTS',G1,font10)
tx(694,619,'100% BLACK + WHITE',WH,font10)
contact=sheet.resize((1920,1280),Image.Resampling.NEAREST)
contact.save(makepath('preview/contact-sheet.png'),optimize=True)

print('STEP 7: actual-assets specimen sheet assembled')

# --- Practical integration snippets for the recommended Rust / Tauri + React shell.
makepath('examples/AssistantPill.tsx').write_text('''import type { ReactNode } from 'react';

/** Presentational component only: native mic, capture and agents are wired separately. */
export type AssistantState = 'idle' | 'listening' | 'processing' | 'muted';
export type AssistantPillProps = {
  state: AssistantState;
  onPrimaryAction: () => void;
  onOpen: () => void;
  assetBase?: string;
  shortcutLabel?: string;
  children?: ReactNode;
};

const statuses: Record<AssistantState, string> = {
  idle: 'Microphone off',
  listening: 'Microphone listening',
  processing: 'Processing request',
  muted: 'Microphone muted',
};

export function AssistantPill({
  state, onPrimaryAction, onOpen, assetBase = '/openclicky-assets',
  shortcutLabel = 'Configure shortcut', children,
}: AssistantPillProps) {
  return (
    <div className="oc-pill" role="group" aria-label="OpenClicky assistant">
      <button className="oc-pill__mark" type="button" onClick={onOpen} aria-label="Open OpenClicky">
        :)
      </button>
      <button
        className="oc-pill__microphone"
        type="button"
        onClick={onPrimaryAction}
        aria-label={state === 'listening' ? 'Stop listening' : 'Microphone action'}
        disabled={state === 'processing'}
      >
        <img width="24" height="24" style={{ imageRendering: 'pixelated' }}
          src={`${assetBase}/microphone-states/png/${state}-64.png`} alt="" />
      </button>
      <span className="oc-pill__status" role="status" aria-live="polite">{statuses[state]}</span>
      {children}
      <span className="oc-pill__shortcut" aria-label={`Shortcut: ${shortcutLabel}`}>
        {shortcutLabel}
      </span>
    </div>
  );
}
''')
makepath('examples/AssistantPill.css').write_text('''/* Add the tokens/openclicky.css design system before importing this file. */
.oc-pill{display:flex;align-items:center;gap:10px;max-width:420px;min-height:48px;padding:0 10px;color:var(--oc-foreground);background:var(--oc-canvas);border:2px solid var(--oc-border-strong);font:13px var(--oc-font-body)}
.oc-pill__mark{border:0;background:none;color:inherit;font:22px var(--oc-font-ui);min-width:34px;cursor:pointer}
.oc-pill__microphone{display:grid;place-items:center;width:36px;height:36px;padding:0;border:1px solid var(--oc-border);background:transparent;color:inherit;cursor:pointer}
.oc-pill__microphone:disabled{opacity:.55;cursor:not-allowed}
.oc-pill__microphone img{image-rendering:pixelated}
.oc-pill__status{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;flex:1}
.oc-pill__shortcut{font-size:10px;color:var(--oc-muted)}
.oc-pill button:focus-visible{outline:2px dashed var(--oc-foreground);outline-offset:3px}
@media(max-width:400px){.oc-pill__shortcut{display:none}}
''')
makepath('examples/INTEGRATION_GUIDE.md').write_text('''# OpenClicky Pixel UI — Tauri / React integration

This kit implements the **visual layer**, not native permissions, transcription, or AI agents.

## Suggested asset placement

Copy `assets/` into your Tauri frontend's `public/openclicky-assets/`. Put `tokens/openclicky.css` and `examples/AssistantPill.css` into your frontend styles and import them once from your app shell.

```tsx
import { AssistantPill, type AssistantState } from './AssistantPill';
import './openclicky.css';
import './AssistantPill.css';

// `state` must be driven by your actual Rust/native microphone lifecycle.
<AssistantPill
  state={state as AssistantState}
  onPrimaryAction={handleUserApprovedMicAction}
  onOpen={openAssistantPanel}
  shortcutLabel="Ctrl+Space (example)"
/>
```

**Do not** report `listening` until native microphone capture has actually started. Do not declare `muted` merely because the transcription stream is idle. Keep explicit consent and minimize background collection.

### Runtime icon paths

- macOS: `assets/platform/macos/OpenClicky-default.icns` for the app icon.
- Windows: `assets/platform/windows/OpenClicky-default.ico` for the app/installer.
- macOS menubar: use the alpha-mask `tray-template-22.png`/`44.png`, with native template tinting.
- Windows tray: use one of the readable dark/light tray outputs depending on actual background; allow system scaling.
- HTML assistant: SVG glyphs in `system-icons/svg`; no antialiasing on sprite images.

### Theme switching

```ts
document.documentElement.setAttribute('data-oc-theme', 'dark');
// Supported: dark, light, highContrast, dither.
```

For system-driven theme selection, read the OS theme preference and map it to `dark` or `light` without adding unapproved hue accents.

### Design policy

- Hard shadows only; no CSS glow, blurred box shadow, or chromatic backdrop effects.
- Screen reticles should move accurately, not via stepped animation.
- Keep keyboard focus and accessible labels separate from decorative bitmap artwork.
- Sound effects should be opt-in; if enabled, preview at quiet levels and respect reduced motion/audio settings.
''')
makepath('QA_CHECKLIST.md').write_text('''# OpenClicky pixel design system — shipping QA checklist

This is a review checklist for the *app build*; the asset package cannot certify OS behavior by itself.

## File integrity
- [ ] All PNG sizes match suffixes in their file names.
- [ ] ICNS files contain proper multi-size PNG entries; macOS app notarization/integration tested on target OS.
- [ ] ICO files contain true multi-size icon frames; Windows taskbar/installer rendering verified.
- [ ] 16px smile still has clearly separated colon dots and a recognizable parenthesis.
- [ ] Dark/Light/Dither/High Contrast token mappings use only approved grayscale.
- [ ] SVG pixel glyphs are well-formed and visually aligned at 16, 24 and 32px.
- [ ] Wallpaper scenes render at full 4K and 1080p without interpolation blur.

## Application UI
- [ ] Dark, Light, High Contrast and Dither selectors persist correctly.
- [ ] Keyboard and assistive-tech focus are visible throughout the notch/pill and dialogs.
- [ ] System fonts / locally licensed font fallbacks work without network access.
- [ ] 100%, 125%, 150%, 200% Windows display scaling inspected manually.
- [ ] Retina/non-Retina and multiple-monitor layouts checked on macOS.
- [ ] Notch and overlay respect menu bar, taskbar, safe areas, and screen capture boundaries.
- [ ] `prefers-reduced-motion` and native reduced-motion options disable decorative animation.

## Sensitive desktop actions
- [ ] Microphone capture starts only after explicit user action and OS permission.
- [ ] Listening/processing/muted text reflects actual native state.
- [ ] Screen capture is disclosed when active and is not incorrectly shown as continuous.
- [ ] Agent actions are reviewed/authorized as appropriate and are cancellable.
- [ ] Third-party connectors show only real, licensed official provider identities.
- [ ] No false OS-permission dialog screenshots or fake success states.

## Audio & missing media
- [ ] Listen to every synthesized effect on headphones and laptop speakers; tune loudness and transients.
- [ ] Audio is optional, not a sole feedback channel, and respects user preference.
- [ ] Replace illustrative onboarding cards with actual OpenClicky video demonstrations where required.
- [ ] Record real up-to-date app-specific screenshots rather than reproducing copyrighted UI.
- [ ] Obtain proper font licenses if font binaries are bundled in a later app release.
''')
# Fix label spacing in the generated specimen image without altering resource geometry.
# (the produced image is subsequently rendered by the source generator above).
makepath('scripts/generate_assets.py').write_text(Path(__file__).read_text())

print('STEP 8: source generator, React/UI examples, integration guide, QA checklist complete')
