"""Stage 1: fit camera + idle body pose to the median keypoints of the idle segment."""
import json, numpy as np, sys
from scipy.optimize import least_squares
from scipy.spatial.transform import Rotation as Rot
import skel, params as P
from common import load_poses, project, weights_for, IMPORTANCE

poses = load_poses('../ref/animation_reference/qQ40c6kgV8M/poses.jsonl')
idle = [p for p in poses if 1.0 <= p['t'] <= 12.2]
K = np.median(np.stack([p['K'] for p in idle]), 0)
S = np.median(np.stack([p['S'] for p in idle]), 0)
O = np.mean(np.stack([p['O'] for p in idle]), 0) > 0.5
W = weights_for(S, O, K)
use = [i for i in skel.COCO if i < 91 and W[i] > 0 and i not in (13, 14, 15, 16)]   # legs truncated by frame: ankle/knee detections unreliable
print('idle frames', len(idle), 'body/face points used', len(use))

x0, off = P.body_init()
cam0 = np.array([0.0, -3.0, 1.25, 0.0, 0.0, 0.0, 2800.0])


def resid(z):
    cam = z[:7]; x = z[7:]
    rv = P.body_rotvecs(x)
    Pj, G, pts = skel.fk(rv)
    X = np.array([skel.point(skel.COCO[i], Pj, pts) for i in use])
    uv = project(X, cam)
    w = np.array([W[i] * IMPORTANCE[i] for i in use])
    r = ((uv - K[use]) * w[:, None] / 8.0).ravel()
    # priors: small rotations for torso/neck/shoulders, rotations bounded
    pri = []
    for b, n in P.BODY_BLOCKS:
        v = x[off[b]:off[b] + n]
        if b in ('spine', 'chest', 'upper_chest', 'neck', 'head', 'hips'):
            pri.append(v * 3.0)
        elif b.endswith('shoulder'):
            pri.append(v * 6.0)
        elif b.endswith('hand'):
            pri.append(v * 1.0)
        elif b.endswith('lower_arm'):
            pri.append([min(0, v[0]) * 30.0, v[1] * 1.5])   # no hyperextension
        else:
            pri.append(v * 0.3)
    # hands must be in front of the torso (y < torso front) – depth prior
    for s in ('left', 'right'):
        hy = Pj[f'{s}_hand'][1]
        pri.append([max(0.0, hy - (-0.06)) * 40.0])
    cam_pri = [(cam[6] - 2800) / 2000.0, cam[5] * 5.0]          # weak focal prior, roll ~ 0
    return np.concatenate([r, np.concatenate([np.ravel(p) for p in pri]), cam_pri])


z0 = np.concatenate([cam0, x0])
sol = least_squares(resid, z0, method='trf', x_scale='jac', max_nfev=4000)
z = sol.x; cam = z[:7]; x = z[7:]
rv = P.body_rotvecs(x); Pj, G, pts = skel.fk(rv)
X = np.array([skel.point(skel.COCO[i], Pj, pts) for i in use]); uv = project(X, cam)
err = np.linalg.norm(uv - K[use], axis=1)
print('cost', sol.cost, 'status', sol.status, 'nfev', sol.nfev)
print('camera C=%s yaw=%.2f pitch=%.2f roll=%.2f f=%.0f' % (np.round(cam[:3], 3), *np.degrees(cam[3:6]), cam[6]))
for i, e in zip(use, err):
    print(f'  {i:3d} {skel.COCO[i]:18s} err {e:6.1f}px  w {W[i]:.2f}')
print('mean body err', err[[j for j, i in enumerate(use) if i < 23]].mean())
for b, n in P.BODY_BLOCKS:
    print(b, np.round(np.degrees(x[off[b]:off[b] + n]), 1))
print('hand pos L', np.round(Pj['left_hand'], 3), 'R', np.round(Pj['right_hand'], 3), 'elbowL', np.round(Pj['left_lower_arm'], 3))
json.dump(dict(cam=cam.tolist(), idle_body=x.tolist()), open('stage1.json', 'w'))
