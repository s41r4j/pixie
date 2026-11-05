#!/usr/bin/env python3
"""Run post-build asset verification. No native OS application functionality is tested."""
from pathlib import Path
from PIL import Image
import io, re, struct, csv, json, wave, hashlib, xml.etree.ElementTree as ET, subprocess, tempfile
root=Path(__file__).resolve().parent.parent
errors=[]
def check(condition,msg):
 if not condition:errors.append(msg)
for variant in ['default','light','outline','dither']:
 for n in [16,24,32,48,64,128,256,512,1024]:
  p=root/f'assets/app-icons/png/{variant}-{n}.png'
  check(p.exists(),str(p))
  if p.exists():
   with Image.open(p) as im:
    check(im.size==(n,n),f'wrong app icon size: {p}')
    pix=im.convert('RGB').getdata()
    check(all(r==g==b for r,g,b in pix),f'chromatic pixel in {p}')
 for plat,suf in [('macos','.icns'),('windows','.ico')]:
  p=root/f'assets/platform/{plat}/OpenClicky-{variant}{suf}'
  check(p.exists(),f'missing {p}')
  if p.exists():
   try:
    im=Image.open(p);check(im.format==('ICNS' if plat=='macos' else 'ICO'),f'bad {p}')
   except Exception as e:errors.append(f'bad icon {p}: {e}')
for k in ['mic','mic-off','sparkle','focus','cursor','layers','chat','waveform','settings','bolt','terminal','grid','sliders','sync','check','close','plus','arrow','eye','speaker','pause','stop','record','screen','lock','keyboard']:
 p=root/f'assets/system-icons/svg/{k}.svg';check(p.exists(),str(p))
 for size in [16,24,32,48,64,128]:
  p=root/f'assets/system-icons/png/{size}/{k}.png'
  check(p.exists(),str(p))
  if p.exists():
   with Image.open(p) as im:check(im.size==(size,size),f'bad glyph size {p}')
for k in ['idle','listening','processing','muted']:
 for size in [32,64,128,256]:
  p=root/f'assets/microphone-states/png/{k}-{size}.png';check(p.exists(),str(p))
  if p.exists():
   with Image.open(p) as im:check(im.size==(size,size),f'bad mic state size {p}')
for k in ['01-moonrise','02-contour','03-night-city','04-grid-world']:
 for theme in ['dark','light']:
  for resolution,dim in [('4k',(3840,2160)),('1080p',(1920,1080))]:
   p=root/f'assets/wallpapers/{resolution}/{theme}/{k}.png';check(p.exists(),str(p))
   if p.exists():
    with Image.open(p) as im:check(im.size==dim,f'bad wallpaper size {p}')
for i in range(1,6):
 p=root/f'assets/onboarding/onboarding-{i:02d}.png';check(p.exists(),str(p))
 if p.exists():
  with Image.open(p) as im:check(im.size==(1280,720),f'bad onboarding size {p}')
for p in root.rglob('*.svg'):
 try:ET.parse(p)
 except Exception as e:errors.append(f'bad SVG {p}: {e}')
check(not any(p.suffix in ['.ttf','.otf','.woff','.woff2'] for p in root.rglob('*') if p.is_file()),'font binary unexpectedly bundled')
with (root/'tokens/tokens.json').open() as f:tok=json.load(f)
check(tok['identity']['mark']==':)','bad brand mark')
check(set(tok['palettes'])=={'dark','light','highContrast','dither'},'missing themes')
for palette in tok['palettes'].values():
 for name,val in palette.items():
  if re.fullmatch(r'#[0-9a-fA-F]{6}',str(val)):
   check(val[1:3].lower()==val[3:5].lower()==val[5:7].lower(),f'nonmono token {name}: {val}')
with (root/'assets/audio/audio-slots.csv').open() as f: audio_rows=list(csv.DictReader(f))
check(len(audio_rows)==82,'audio file count mismatch')
for row in audio_rows:
 p=root/'assets/audio'/row['slot'];check(p.exists(),f'missing sound: {p}')
 if p.suffix=='.wav' and p.exists():
  with wave.open(str(p)) as w:
   check(w.getframerate()==44100 and w.getnchannels()==1 and w.getsampwidth()==2,f'bad wave format: {p}')
   actual=1000*w.getnframes()/w.getframerate()
   check(abs(actual-int(row['duration_ms']))<1.5,f'duration mismatch: {p} {actual}')
for p in root.rglob('*.svg'):
 check('xmlns="http://www.w3.org/2000/svg"' in p.read_text(),f'bad XML namespace {p}')
with (root/'REPLACEMENT_MAP.csv').open() as f:
 rows=list(csv.DictReader(f))
 for row in rows:
  if row['openclicky_asset']:
   check((root/row['openclicky_asset']).exists(),f'missing replacement {row["original_slot"]} -> {row["openclicky_asset"]}')
check(len(rows)>150,f'incomplete mapping ({len(rows)} rows)')
# Parse all local JS in standalone showroom (not a network/browser runtime test).
html=(root/'preview/index.html').read_text()
script=re.findall(r'<script>(.*?)</script>',html,flags=re.S)
check(len(script)==1,'missing local preview script')
if script:
 with tempfile.NamedTemporaryFile(mode='w',suffix='.js',delete=False) as f:
  f.write(script[0]);js_path=f.name
 result=subprocess.run(['node','--check',js_path],capture_output=True,text=True)
 check(result.returncode==0,f'JS parse failure: {result.stderr}')
 Path(js_path).unlink()
check((root/'preview/contact-sheet.png').exists(),'missing actual asset contact sheet')
if errors:
 print('FAIL: '+str(len(errors))+' checks:')
 print('\n'.join('- '+e for e in errors[:80]))
 raise SystemExit(2)
print(f'PASS — 4 complete icon families; 26 glyphs; 4 microphone states; 16 wallpaper renders; 5 onboarding cards; 82 new audio files; {len(rows)} replacement map entries; SVG/XML, JSON, JS parses OK; no font binaries.')
