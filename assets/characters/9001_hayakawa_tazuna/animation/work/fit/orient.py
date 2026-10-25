"""Hand-orientation refit. The 2D keypoints barely constrain the roll of a hand whose fingers overlap,
so palm normal + finger direction targets (read off the source frames) are imposed per phase.
Only arm/hand parameters move; torso, head and contacts stay as in the contact refit."""
import sys, os, json, numpy as np
SRC, OUT, T0, T1 = sys.argv[1], sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
sys.argv = ['x']
from scipy.optimize import least_squares
import stage3 as S3, stage2 as S2, params as P, fastfk
from common import load_poses, weights_for, project
R_ = fastfk.NAME2ROW

def unit(v):
    v = np.array(v, float); return v / np.linalg.norm(v)

def ortho(n, f):          # make palm normal orthogonal to the finger direction
    f = unit(f); n = unit(n); n = unit(n - np.dot(n, f) * f); return n, f

# torso frame (upper_chest): +X character left, +Y into the body, +Z up
TGT = {
    'idle':  {'right': ortho([0, 0.92, -0.38], [0.85, 0.0, -0.52]),     # backs of both hands toward the camera, fingers angled down-inward
              'left':  ortho([0, 0.92, -0.38], [-0.85, 0.0, -0.52])},
    'chest': {'right': ortho([0.05, 0.97, -0.2], [0.42, 0.12, 0.9]),    # palm flat on the upper chest, fingers toward the left collar
              'left':  ortho([-0.45, 0.85, 0.0], [-0.15, -0.1, -0.98])},  # hanging hand, palm toward the thigh
    'chin':  {'left':  ortho([-0.45, 0.85, 0.0], [-0.15, -0.1, -0.98])},
}
W_ORI = 12.0
W_TEMP = 15.0

def hand_vecs(A, G, s):
    Gt = G[R_['upper_chest']]; h = R_[f'{s}_hand']
    n = Gt.T @ (G[h] @ np.array([0, 0, -1.0]))
    f = Gt.T @ (A[R_[f'{s}_middle_proximal']] - A[h]); f = f / np.linalg.norm(f)
    return n, f

_old_anat = S3.anat_terms
def anat_terms(A, G, t, wr_obs):
    idle, chest, chin = S3.phases(t)
    r = list(_old_anat_hang(A, G, t, wr_obs))
    for ph, w in (('idle', idle), ('chest', chest), ('chin', chin)):
        if w <= 0: continue
        for s, (tn, tf) in TGT[ph].items():
            n, f = hand_vecs(A, G, s)
            if ph == 'chest' and s == 'right':
                ne = tn              # flat on the chest: full normal (fingers must lie in the chest plane, not dig into it)
            else:
                ne = tn - np.dot(tn, f) * f; ne = ne / max(1e-6, np.linalg.norm(ne))   # roll about the finger axis only
            fw = 0.25 if ph == 'chest' and s == 'right' else (0.6 if s == 'left' and ph in ('chest', 'chin') else 0.0)   # unseen hanging hand: keep fingers down
            r += list(w * W_ORI * (n - ne)) + list(w * W_ORI * fw * (f - tf))
    return np.array(r)

def _old_anat_hang(A, G, t, wr_obs):     # keep only the hanging-arm prior of the stage-3 terms
    idle, chest, chin = S3.phases(t)
    r = []; Gt = G[R_['upper_chest']]
    for s in ('left', 'right'):
        if not wr_obs[s] or (s == 'left' and (chest > 0 or chin > 0)):
            sh = A[R_[f'{s}_upper_arm']]; el = A[R_[f'{s}_lower_arm']]; wr = A[R_[f'{s}_hand']]
            du = Gt.T @ ((el - sh) / np.linalg.norm(el - sh)); dl = Gt.T @ ((wr - el) / np.linalg.norm(wr - el))
            r += [max(0.0, du[1] - 0.05) * 80.0, max(0.0, -du[1] - 0.30) * 25.0, max(0.0, dl[1] - 0.05) * 30.0, max(0.0, -dl[1] - 0.45) * 10.0]
    return r
S3.anat_terms = anat_terms
import os
if os.environ.get('TZ_LATE_IDLE'):              # hands are still travelling at 35.0 s: settle the second idle over 35.0-35.3 s
    def phases(t):
        idle = max(S3.ramp(t, 0.0, 12.33), S3.ramp(t, 35.5, 43.0, 0.4))
        return idle, S3.ramp(t, 12.95, 30.12), S3.ramp(t, 30.88, 34.12)
    S3.phases = phases

from scipy.spatial.transform import Rotation as Rot
def seed_chest(x):
    x = x.copy(); rv = P.body_rotvecs(x); A, G = fastfk.fk_all(rv)
    Gt = G[R_['upper_chest']]; tn, tf = TGT['chest']['right']
    fw = Gt @ tf; nw = Gt @ tn
    ex = -fw; ez = -nw; ey = np.cross(ez, ex)
    Gd = np.stack([ex, ey, ez], 1)
    Gp = G[R_['right_lower_arm']]
    L = Gp.T @ Gd
    q = Rot.from_matrix(L).as_quat()            # swing-twist about local X
    tw = np.array([q[0], 0, 0, q[3]]); tw /= np.linalg.norm(tw)
    tau = Rot.from_quat(tw).as_rotvec()[0]
    o = S2.off['right_lower_arm']; x[o + 1] += tau
    x[S2.off['right_hand']:S2.off['right_hand'] + 3] = (Rot.from_rotvec([-tau, 0, 0]) * Rot.from_matrix(L)).as_rotvec()
    return x

def left_policy(W, p, t):
    """The hanging left hand sits on the dialogue-panel edge: its points blink in and out of the mask.
    Use one consistent rule instead: fingers ignored, wrist kept (chin pose, hand at the hip) at reduced weight."""
    idle, chest, chin = S3.phases(t)
    if chest > 0 or chin > 0:
        W = W.copy(); W[92:112] = 0.0
        if chin > 0 and p['S'][9] > 0.45:
            W[9] = W[91] = 0.5 * p['S'][9]
        elif chest > 0:
            W[9] = W[91] = 0.0
    return W

def seed_needed(x):
    rv = P.body_rotvecs(x); A, G = fastfk.fk_all(rv); n, f = hand_vecs(A, G, 'right')
    return float(np.dot(n, TGT['chest']['right'][0])) < 0.8

idx = []
for b in ('left_upper_arm', 'right_upper_arm', 'left_lower_arm', 'right_lower_arm', 'left_hand', 'right_hand'):
    n = dict(P.BODY_BLOCKS)[b]; idx += list(range(S2.off[b], S2.off[b] + n))
idx = np.array(idx)
ps = {round(p['t'], 2): p for p in load_poses('../ref/animation_reference/qQ40c6kgV8M/poses.jsonl')}
d = json.load(open(SRC)); frames = [f for f in d['frames'] if T0 <= f['t'] < T1]
x_prev = None; out = []
if os.environ.get('TZ_INIT'):                    # continue the chain from another solution (file@t)
    _fn, _t = os.environ['TZ_INIT'].split('@')
    x_prev = np.array([f for f in json.load(open(_fn))['frames'] if abs(f['t'] - float(_t)) < 0.02][0]['body'])
SEED = {}
if os.environ.get('TZ_SEED_FILE'):               # extra start: an already converged idle chain
    SEED = {round(f['t'], 2): np.array(f['body']) for f in json.load(open(os.environ['TZ_SEED_FILE']))['frames']}
for k, f in enumerate(frames):
    t = f['t']; p = ps[round(t, 2)]; K = p['K']; W = weights_for(p['S'], p['O'], K)
    idle, chest, chin = S3.phases(t)
    if idle > 0:
        W = W.copy(); W[92:112] *= 0.3; W[113:133] *= 0.3
    W = left_policy(W, p, t)
    x0 = np.array(f['body']); S3.TORSO_REF['x'] = x0
    hs = {s: np.array(f['hands'][s]) for s in ('left', 'right')}
    rvh = {}; rvh.update(P.hand_rotvecs('left', hs['left'])); rvh.update(P.hand_rotvecs('right', hs['right']))
    wr_obs = {'left': bool(W[9] > 0 or W[91] > 0), 'right': bool(W[10] > 0 or W[112] > 0)}
    base = x0.copy() if x_prev is None else x0.copy()
    if x_prev is not None: base[idx] = x_prev[idx]
    xp = base.copy()
    def res(z):
        x = x0.copy(); x[idx] = z
        r = S3.body_resid3(x, K, W, xp, wr_obs, rvh, t)
        if x_prev is not None:                  # forearm twist / hand roll are interchangeable: keep the split continuous
            r = np.concatenate([r, (4.0 + (W_TEMP - 4.0) * max(idle, chest, chin) ** 2) * (z - x_prev[idx])])   # loose in transitions
        return r
    best = None
    starts = [base[idx]] + ([x0[idx]] if x_prev is not None else [])
    if chest > 0.5 and (x_prev is None or seed_needed(x_prev)):   # seed only when the chain has lost the flat-hand pose
        starts.append(seed_chest(base)[idx])
    if t >= 35.0 and round(t, 2) in SEED:
        starts.append(SEED[round(t, 2)][idx])
    for z0 in starts:
        sol = least_squares(res, z0, method='trf', x_scale='jac', max_nfev=80, ftol=1e-6, xtol=1e-6)
        if best is None or sol.cost < best.cost: best = sol
    x = x0.copy(); x[idx] = best.x
    rv = P.body_rotvecs(x); rv.update(rvh); A, G = fastfk.fk_all(rv)
    uv = project(A[[S2.ROW[i] for i in S2.BODY_IDX]], S3.CAM); wb = np.array([W[i] for i in S2.BODY_IDX])
    e = np.linalg.norm(uv - K[S2.BODY_IDX], axis=1); berr = float((e * (wb > 0)).sum() / max(1, (wb > 0).sum()))
    f['body'] = x.tolist(); f['rotvecs'] = {b: np.asarray(v).tolist() for b, v in rv.items()}; f['err'] = berr
    x_prev = x; out.append(f)
    if k % 20 == 0:
        nR, fR = hand_vecs(A, G, 'right'); nL, fL = hand_vecs(A, G, 'left')
        print(f"{k}/{len(frames)} t={t:.1f} err={berr:.1f} Rn={nR.round(2)} Rf={fR.round(2)} Ln={nL.round(2)}", flush=True)
json.dump(dict(cam=d['cam'], cy=d.get('cy', 724.0), frames=out), open(OUT, 'w'))
print('saved', OUT, len(out), 'mean err', np.mean([o['err'] for o in out]))
