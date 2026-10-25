"""Reprojection error of the smoothed 30 fps animation vs. the 10 Hz 2D keypoints (per segment, per joint group)."""
import json, sys, numpy as np
from scipy.spatial.transform import Rotation as Rot
import skel, fastfk, common
from common import load_poses, project, weights_for

src = sys.argv[1] if len(sys.argv) > 1 else 'body30.json'
d = json.load(open(src)); cam = np.array(json.load(open('candidates.json'))['2200']['cam']); common.CY = 724.0
t30 = np.array(d['t']); Q = {b: np.array(v) for b, v in d['quats'].items()}
poses = load_poses('../ref/animation_reference/qQ40c6kgV8M/poses.jsonl')
GROUPS = {'head/face': [0, 1, 2, 23 + 8, 23 + 30], 'shoulders': [5, 6], 'elbows': [7, 8], 'wrists': [9, 10],
          'R fingers': list(range(113, 133)), 'L fingers': list(range(92, 112))}
SEGS = [('idle', 0.3, 12.3), ('hand→chest', 12.3, 13.2), ('chest hold', 13.2, 30.2), ('chest→chin', 30.2, 30.9),
        ('chin hold', 30.9, 34.2), ('chin→idle', 34.2, 35.1), ('idle 2', 35.1, 42.1)]
rows = []
for p in poses:
    if p['t'] < t30[0] or p['t'] > t30[-1]:
        continue
    i = int(round((p['t'] - t30[0]) * 30))
    rv = {b: Rot.from_quat(Q[b][i]).as_rotvec() for b in Q}
    A, G = fastfk.fk_all(rv)
    W = weights_for(p['S'], p['O'], p['K'])
    err = {}
    for g, idx in GROUPS.items():
        e = [np.linalg.norm(project(A[fastfk.NAME2ROW[skel.COCO[j]]], cam)[0] - p['K'][j]) for j in idx if W[j] > 0 and j in skel.COCO]
        err[g] = np.mean(e) if e else np.nan
    rows.append((p['t'], err))
print(f"{'segment':12s} " + ' '.join(f'{g:>10s}' for g in GROUPS))
for name, a, b in SEGS:
    sel = [e for t, e in rows if a <= t < b]
    print(f'{name:12s} ' + ' '.join(f'{np.nanmean([e[g] for e in sel]):10.1f}' for g in GROUPS))
allv = [np.nanmean([e[g] for g in ('head/face', 'shoulders', 'elbows', 'wrists')]) for t, e in rows]
print('overall body mean %.2f px, p95 %.2f px (image 1080x1448)' % (np.nanmean(allv), np.nanpercentile(allv, 95)))
json.dump([dict(t=t, **{k: (None if np.isnan(v) else float(v)) for k, v in e.items()}) for t, e in rows], open(src.replace('.json', '_proj_err.json'), 'w'))
