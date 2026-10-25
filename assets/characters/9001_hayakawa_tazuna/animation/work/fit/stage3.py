"""Stage 3: contact-aware refit. Keeps the 2D fit, resolves depth with physical contacts:
idle  -> left palm rests on the lower abdomen, right palm rests on the left hand, fingers relaxed
chest -> right palm on the chest surface
chin  -> right index PIP knuckle under the chin
"""
import json, sys, numpy as np
from scipy.optimize import least_squares
from scipy.interpolate import RegularGridInterpolator
import skel, params as P, fastfk, common
from common import load_poses, project, weights_for, IMPORTANCE
import stage2 as S2            # reuses residual pieces (guarded main below)

T0, T1, OUT = (float(sys.argv[1]), float(sys.argv[2]), sys.argv[3]) if __name__ == '__main__' else (0, 99, None)
CAM = S2.CAM
srf = json.load(open('front_surface.json'))
zg = np.array(srf['z']); xg = np.array(srf['x'])
Y = np.array([[np.nan if v is None else v for v in row] for row in srf['front_y']])
# fill holes by nearest along x
for i in range(Y.shape[0]):
    r = Y[i]; ok = ~np.isnan(r)
    if ok.any():
        Y[i] = np.interp(xg, xg[ok], r[ok])
SURF = RegularGridInterpolator((zg, xg), Y, bounds_error=False, fill_value=None)


def surf_y(p):
    return float(SURF([[np.clip(p[2], zg[0], zg[-1]), np.clip(p[0], xg[0], xg[-1])]])[0])


def ramp(t, a, b, r=0.15):
    """1 inside [a,b], smooth 0..1 over r seconds at both ends"""
    if t < a - r or t > b + r:
        return 0.0
    if t < a:
        x = (t - (a - r)) / r
    elif t > b:
        x = ((b + r) - t) / r
    else:
        return 1.0
    return x * x * (3 - 2 * x)


def phases(t):
    idle = max(ramp(t, 0.0, 12.33), ramp(t, 35.0, 43.0))
    chest = ramp(t, 12.95, 30.12)
    chin = ramp(t, 30.88, 34.12)
    return idle, chest, chin


PALM_OFF = np.array([0, 0, -0.012])            # palm surface below the hand bone line (rest: palm faces -Z)
BACK_OFF = np.array([0, 0, 0.014])
R_ = fastfk.NAME2ROW


def palm_points(A, G, s):
    hi = R_[f'{s}_hand']
    c = (A[hi] + A[R_[f'{s}_index_proximal']] + A[R_[f'{s}_little_proximal']]) / 3.0
    return c + G[hi] @ PALM_OFF, c + G[hi] @ BACK_OFF


CHIN_LOCAL = np.array([0.0, -0.030, 1.376]) - skel.REST['head']


def to_torso(p, A, G, bone):
    """world point -> rest-pose coordinates of the given torso bone (surface moves with the body)"""
    i = R_[bone]
    return G[i].T @ (p - A[i]) + skel.REST[bone]


def gap(p, A, G, bone):
    q = to_torso(p, A, G, bone)
    return q[1] - surf_y(q)


def contact_terms(A, G, t):
    idle, chest, chin = phases(t)
    r = []
    lp, lb = palm_points(A, G, 'left'); rp, rb = palm_points(A, G, 'right')
    if idle > 0:
        r.append(idle * 300 * (gap(lp, A, G, 'spine') + 0.004))          # left palm on the lower abdomen
        r.append(idle * 300 * (gap(rp, A, G, 'spine') + 0.026))          # right palm resting on the left hand
    if chest > 0:
        r.append(chest * 300 * (gap(rp, A, G, 'upper_chest') + 0.006))
    if chin > 0:
        hi = R_['head']
        target = A[hi] + G[hi] @ CHIN_LOCAL
        pip = A[R_['right_index_intermediate']]
        r += list(chin * 120 * (pip - target))
    return np.array(r) if r else np.zeros(0)


TORSO_W = 150.0
TORSO_REF = {}


def body_resid3(x, K, W, x_prev, wr_obs, rvh, t):
    base = S2.body_resid(x, K, W, x_prev, wr_obs, rvh)
    tor = []
    for b in ('hips', 'spine', 'chest', 'upper_chest'):     # keep an upright torso: depth is resolved by the arms
        tor.append((x[S2.off[b]:S2.off[b] + 3] - TORSO_REF['x'][S2.off[b]:S2.off[b] + 3]) * TORSO_W)
    base = np.concatenate([base] + tor)
    rv = P.body_rotvecs(x); rv.update(rvh)
    A, G = fastfk.fk_all(rv)
    return np.concatenate([base, contact_terms(A, G, t), anat_terms(A, G, t, wr_obs)])


def anat_terms(A, G, t, wr_obs):
    idle, chest, chin = phases(t)
    r = []
    up = R_['upper_chest']; Gt = G[up]
    def palm_n(s):
        return Gt.T @ (G[R_[f'{s}_hand']] @ np.array([0, 0, -1.0]))      # palm normal in torso frame
    # palms face the body while touching it (+Y = into the body)
    if idle > 0:
        for s in ('left', 'right'):
            n = palm_n(s); r += list(idle * 6.0 * (n - np.array([0, 0.85, 0.53])) * np.array([1, 0, 1]))
    if chest > 0:
        n = palm_n('right'); r += list(chest * 6.0 * (n - np.array([-0.2, 0.95, 0.2])) * np.array([1, 0, 1]))
    # an unseen arm hangs naturally: no large fore/aft swing of the upper arm
    for s in ('left', 'right'):
        if not wr_obs[s] or (s == 'left' and (chest > 0 or chin > 0)):
            sh = A[R_[f'{s}_upper_arm']]; el = A[R_[f'{s}_lower_arm']]; wr = A[R_[f'{s}_hand']]
            du = Gt.T @ ((el - sh) / np.linalg.norm(el - sh)); dl = Gt.T @ ((wr - el) / np.linalg.norm(wr - el))
            r += [max(0.0, du[1] - 0.05) * 80.0, max(0.0, -du[1] - 0.30) * 25.0, max(0.0, dl[1] - 0.05) * 30.0, max(0.0, -dl[1] - 0.45) * 10.0]
    return np.array(r)


RELAX = None


def hand_resid3(h, s, wrist, K, W, h_prev, t, Abody, Gbody):
    base = S2.hand_resid(h, s, wrist, K, W, h_prev)
    idle, chest, chin = phases(t)
    extra = []
    if idle > 0:                                    # overlapped fingers: detector unreliable -> relaxed straight fingers
        extra.append(idle * 8.0 * (h - RELAX))
    if chest > 0:                                   # hand laid on the chest: gently curved fingers
        extra.append(chest * 2.0 * (h - RELAX_CHEST))
    if (chest > 0 or chin > 0) and s == 'left':     # hanging left hand: relaxed
        extra.append(max(chest, chin) * 2.0 * (h - RELAX_CHEST))
    if chin > 0 and s == 'right':
        Pp = S2.hand_fk(s, h, *wrist)
        hi = R_['head']; target = Abody[hi] + Gbody[hi] @ CHIN_LOCAL
        extra.append(chin * 120 * (Pp['right_index_intermediate'] - target))
    return np.concatenate([base] + [np.ravel(e) for e in extra])


RELAX = np.zeros(P.NH); RELAX_CHEST = np.zeros(P.NH)
_i = 0
for b, n in P.HAND_BLOCKS['left']:
    if n in (1, 2):
        RELAX[_i] = np.radians(12 if 'thumb' not in b else 8)
        RELAX_CHEST[_i] = np.radians(16 if 'thumb' not in b else 8)
    _i += n

if __name__ == '__main__':
    src = {}
    for k in ('A1', 'A2', 'B1', 'B2'):
        for f in json.load(open(f'fit{k}.json'))['frames']:
            src.setdefault(round(f['t'], 2), []).append(f)
    poses = [p for p in load_poses('../ref/animation_reference/qQ40c6kgV8M/poses.jsonl') if T0 <= p['t'] <= T1]
    out = []; x_prev = None; h_prev = None
    for k, p in enumerate(poses):
        cand = src.get(round(p['t'], 2))
        if not cand:
            continue
        f0 = cand[0]
        if x_prev is None:
            x = np.array(f0['body']); hs = {s: np.array(f0['hands'][s]) for s in ('left', 'right')}
            x_prev = x.copy(); h_prev = dict(hs)
        else:                                   # continue from the previous contact solution (stable convergence)
            x = x_prev.copy(); hs = dict(h_prev)
        K = p['K']; W = weights_for(p['S'], p['O'], K)
        TORSO_REF['x'] = np.array(f0['body'])          # torso follows the 2D-only solution of this frame
        idle, chest, chin = phases(p['t'])
        if idle > 0:      # down-weight overlapped finger detections in the body stage
            W = W.copy(); W[92:112] *= 0.3; W[113:133] *= 0.3
        wr_obs = {'left': bool(W[9] > 0 or W[91] > 0), 'right': bool(W[10] > 0 or W[112] > 0)}
        rvh = {}; rvh.update(P.hand_rotvecs('left', hs['left'])); rvh.update(P.hand_rotvecs('right', hs['right']))
        sol = least_squares(body_resid3, x, args=(K, W, x_prev, wr_obs, rvh, p['t']), method='trf', x_scale='jac', max_nfev=70, ftol=1e-6, xtol=1e-6)
        x = sol.x
        rvb = P.body_rotvecs(x); Ab, Gb = fastfk.fk_all(rvb)
        for s in ('left', 'right'):
            wi = R_[f'{s}_hand']
            hsol = least_squares(hand_resid3, hs[s], args=(s, (Gb[wi], Ab[wi]), K, W, h_prev[s], p['t'], Ab, Gb), method='trf', x_scale='jac', max_nfev=60)
            hs[s] = hsol.x
        rv = dict(rvb); rv.update(P.hand_rotvecs('left', hs['left'])); rv.update(P.hand_rotvecs('right', hs['right']))
        A, G = fastfk.fk_all(rv)
        uv = project(A[[S2.ROW[i] for i in S2.BODY_IDX]], CAM)
        wb = np.array([W[i] for i in S2.BODY_IDX]); e = np.linalg.norm(uv - K[S2.BODY_IDX], axis=1)
        berr = float((e * (wb > 0)).sum() / max(1, (wb > 0).sum()))
        lp, _ = palm_points(A, G, 'left'); rp, _ = palm_points(A, G, 'right')
        gl = gap(lp, A, G, 'spine'); gr = gap(rp, A, G, 'spine' if p['t'] < 12.6 or p['t'] > 34.6 else 'upper_chest')
        out.append(dict(t=p['t'], frame=p['frame'], body=x.tolist(), hands={s: hs[s].tolist() for s in hs},
                        rotvecs={b: np.asarray(v).tolist() for b, v in rv.items()}, err=berr,
                        gapL=float(gl), gapR=float(gr)))
        x_prev = x; h_prev = dict(hs)
        if k % 25 == 0:
            print(f"{k}/{len(poses)} t={p['t']:.1f} err={berr:.2f}px gapL={out[-1]['gapL']*100:.1f}cm gapR={out[-1]['gapR']*100:.1f}cm", flush=True)
    json.dump(dict(cam=CAM.tolist(), cy=724.0, frames=out), open(OUT, 'w'))
    print('saved', OUT, len(out), 'mean err', np.mean([o['err'] for o in out]))
