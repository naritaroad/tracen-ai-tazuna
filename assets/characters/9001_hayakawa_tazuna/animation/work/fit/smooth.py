"""Merge chunk fits, adaptive temporal smoothing in quaternion space, resample to 30 fps."""
import json, sys, numpy as np
from scipy.spatial.transform import Rotation as Rot
from scipy.ndimage import gaussian_filter1d
from scipy.signal import savgol_filter
from scipy.interpolate import CubicSpline

FPS = 30
import os
PFX = os.environ.get('TZ_FIT', 'O_')          # O_ = hand-orientation refit of the contact solution
J = {k: json.load(open(f'fit{PFX}{k}.json'))['frames'] for k in ('A', 'R1', 'R2', 'R3')}
frames = ([f for f in J['A'] if f['t'] < 12.26] + [f for f in J['R1'] if 12.26 <= f['t'] < 20.9] +
          [f for f in J['R2'] if 20.9 <= f['t'] < 30.6])
if os.path.exists(f'fit{PFX}R3b.json'):         # re-solved chin + return-to-idle range
    R3b = json.load(open(f'fit{PFX}R3b.json'))['frames']; t_end = max(f['t'] for f in R3b) + 0.01
    frames += [f for f in R3b if f['t'] >= 30.6] + [f for f in J['R3'] if f['t'] > t_end]
else:
    frames += [f for f in J['R3'] if f['t'] >= 30.6]
frames.sort(key=lambda f: f['t'])
import twistfix
frames = twistfix.fix_sequence(frames, share=0.5)    # half of the hand roll goes to the forearm (no candy-wrapper wrist)
t = np.array([f['t'] for f in frames])
bones = sorted(frames[0]['rotvecs'].keys())
print('samples', len(t), t[0], t[-1], 'bones', len(bones))


def quat_series(b):
    q = Rot.from_rotvec(np.array([f['rotvecs'][b] for f in frames])).as_quat()   # x,y,z,w
    for i in range(1, len(q)):
        if np.dot(q[i], q[i - 1]) < 0:
            q[i] = -q[i]
    return q


# global motion speed (deg/s) of the arms, used to drive adaptive smoothing
def ang_speed(q):
    d = np.abs(np.sum(q[1:] * q[:-1], 1)).clip(0, 1)
    s = np.degrees(2 * np.arccos(d)) * 10.0
    return np.concatenate([[s[0]], s])

speed = np.zeros(len(t))
for b in ('right_upper_arm', 'right_lower_arm', 'right_hand'):
    speed = np.maximum(speed, ang_speed(quat_series(b)))
speed = gaussian_filter1d(speed, 1.0)
# transitions identified frame-accurately from the source (60 fps review); everything else is a hold
TRANS = [(12.25, 13.10), (30.05, 30.95), (34.05, 35.20)]
w_fast = np.zeros(len(t))
for a, b2 in TRANS:
    for i, ti in enumerate(t):
        if a - 0.2 <= ti <= b2 + 0.2:
            x = 1.0 if a <= ti <= b2 else 1.0 - min(abs(ti - a), abs(ti - b2)) / 0.2
            w_fast[i] = max(w_fast[i], x * x * (3 - 2 * x))
t30 = np.arange(round(t[0] * FPS), round(t[-1] * FPS) + 1) / FPS
out = {}
for b in bones:
    q = quat_series(b)
    finger = any(k in b for k in ('thumb', 'index', 'middle', 'ring', 'little'))
    heavy_sigma = 5.0 if finger else 4.0                      # holds: remove 10 Hz fit jitter (sigma 0.4-0.5 s)
    if b in ('head', 'neck', 'upper_chest', 'chest', 'spine', 'hips'):
        heavy_sigma = 2.0                                    # keep nods / tilts / breathing readable
    qh = q.copy()                                             # heavy smoothing per hold segment (no leak across transitions)
    core = np.zeros(len(t), bool)
    for a, b2 in TRANS:
        core |= (t >= a) & (t <= b2)
    i = 0
    while i < len(t):
        j = i
        while j < len(t) and core[j] == core[i]:
            j += 1
        if not core[i]:
            qh[i:j] = gaussian_filter1d(q[i:j], heavy_sigma, axis=0, mode='nearest')
        i = j
    ql = savgol_filter(q, 5, 2, axis=0, mode='interp')
    qs = qh * (1 - w_fast[:, None]) + ql * w_fast[:, None]
    qs /= np.linalg.norm(qs, axis=1, keepdims=True)
    cs = CubicSpline(t, qs, axis=0)
    q30 = cs(t30); q30 /= np.linalg.norm(q30, axis=1, keepdims=True)
    out[b] = q30.tolist()
json.dump(dict(fps=FPS, t=t30.tolist(), quats=out, speed=speed.tolist(), t10=t.tolist(), w_fast=w_fast.tolist()), open('body30.json', 'w'))
print('frames30', len(t30), 'transition samples', int((w_fast > 0.5).sum()))
for a, b2 in [(0, 12.3), (12.3, 13.2), (13.2, 30.2), (30.2, 30.9), (30.9, 34.2), (34.2, 35.1), (35.1, 42.2)]:
    s = (t >= a) & (t < b2)
    print(f'{a:5.1f}-{b2:5.1f} mean speed {speed[s].mean():6.1f} deg/s  max {speed[s].max():6.1f}  w_fast {w_fast[s].mean():.2f}')
