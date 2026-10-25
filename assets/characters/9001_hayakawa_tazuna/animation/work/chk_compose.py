"""usage: chk_compose.py <render_dir> <out.png> <x0> <y0> <x1> <y1> t1 t2 ...  (source px box)"""
import sys, subprocess, os
from PIL import Image
rd, out = sys.argv[1], sys.argv[2]; box = tuple(int(v) for v in sys.argv[3:7]); ts = sys.argv[7:]
root = os.path.abspath(os.path.join(os.path.dirname(__file__), '../../../../..'))
src = os.path.join(root, 'example_pose/9001_Hayakawa Tazuna/exsample_source_vedio/qQ40c6kgV8M.mp4')
rows = []
for t in ts:
    tmp = f'/tmp/src_{t}.png'
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-ss', t, '-i', src, '-frames:v', '1', tmp], check=True)
    s = Image.open(tmp).convert('RGBA'); r = Image.open(os.path.join(rd, f'pose_{t}.png')).convert('RGBA').resize(s.size)
    bg = Image.new('RGBA', s.size, (200, 205, 210, 255)); bg.alpha_composite(r)
    ov = s.copy(); rr = r.copy(); rr.putalpha(rr.split()[3].point(lambda v: int(v * 0.55))); ov.alpha_composite(rr)
    rows.append([x.crop(box).convert('RGB') for x in (s, bg, ov)])
w, h = rows[0][0].size; o = Image.new('RGB', (w * 3, h * len(rows)))
for j, row in enumerate(rows):
    for i, im in enumerate(row): o.paste(im, (i * w, j * h))
o.save(out); print('wrote', out, o.size)
