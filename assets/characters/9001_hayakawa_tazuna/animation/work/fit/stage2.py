"""Stage 2: per-frame body + finger fit with fixed camera, temporal warm start and physical priors."""
import json, sys, numpy as np, time
from scipy.optimize import least_squares
import skel, params as P, fastfk, common
from common import load_poses, project, weights_for, IMPORTANCE

MAIN = __name__ == '__main__'
CAMSEL = sys.argv[1] if MAIN and len(sys.argv) > 1 else '2200'
OUT = sys.argv[2] if MAIN and len(sys.argv) > 2 else f'stage2_{CAMSEL}.json'
T0 = float(sys.argv[3]) if MAIN and len(sys.argv) > 3 else 0.0
T1 = float(sys.argv[4]) if MAIN and len(sys.argv) > 4 else 99.0
cand = json.load(open('candidates.json'))[CAMSEL]
CAM = np.array(cand['cam']); common.CY = 724.0
poses = [p for p in load_poses('../ref/animation_reference/qQ40c6kgV8M/poses.jsonl') if T0 <= p['t'] <= T1]

BODY_IDX = [i for i in skel.COCO if i < 91 and i not in (13, 14, 15, 16)]
# hand keypoints used to orient the hand in the body stage (wrist + MCPs)
for base in (91, 112):
    BODY_IDX += list(range(base, base + 21))      # all hand points (fingers use the current finger solution)
HAND_IDX = {'left': [i for i in range(91, 112)], 'right': [i for i in range(112, 133)]}
ROW = {i: fastfk.NAME2ROW[skel.COCO[i]] for i in skel.COCO}

x_idle, off = P.body_init()
# initialise from stage-1 idle solution of the chosen camera
_rv = cand['rotvecs']
for b, n in P.BODY_BLOCKS:
    if n == 3:
        x_idle[off[b]:off[b] + 3] = _rv[b]
# lower arm (flex,twist) from stored rotvec is approximate: recover flex magnitude
for s in ('left', 'right'):
    v = np.array(_rv[f'{s}_lower_arm']); x_idle[off[f'{s}_lower_arm']] = np.linalg.norm(v)

# torso ellipse cross-sections (from mesh measurement): z -> (cy, ax, ay)
def torso_inside(p):
    z = p[2]
    if z < 0.93 or z > 1.36:
        return 0.0
    ax = 0.105 if z < 1.15 else 0.10
    ay = 0.085 if z < 1.2 else 0.09
    cy = 0.012
    d = (p[0] / ax) ** 2 + ((p[1] - cy) / ay) ** 2
    return max(0.0, 1.0 - d)


def body_resid(x, K, W, x_prev, wr_obs, rvh):
    rv = P.body_rotvecs(x); rv.update(rvh)
    A, G = fastfk.fk_all(rv)
    X = A[[ROW[i] for i in BODY_IDX]]
    uv = project(X, CAM)
    w = np.array([W[i] * IMPORTANCE[i] for i in BODY_IDX])
    r = ((uv - K[BODY_IDX]) * w[:, None] / 6.0).ravel()
    pri = [(x - x_prev) * 0.35]                                   # temporal coherence
    for b in ('hips', 'spine', 'chest', 'upper_chest'):
        pri.append((x[off[b]:off[b] + 3] - x_idle[off[b]:off[b] + 3]) * 2.0)
    for b in ('neck', 'head'):
        pri.append((x[off[b]:off[b] + 3]) * 0.6)
    for b in ('left_shoulder', 'right_shoulder'):
        pri.append(x[off[b]:off[b] + 3] * 4.0)
    for s in ('left', 'right'):
        f, tw = x[off[f'{s}_lower_arm']:off[f'{s}_lower_arm'] + 2]
        pri.append([min(0.0, f) * 30.0, max(0.0, f - np.radians(150)) * 30.0, tw * 0.8])
        if not wr_obs[s]:                                        # wrist hidden: relax toward hanging arm
            pri.append([(f - np.radians(20)) * 0.6])
            pri.append(x[off[f'{s}_hand']:off[f'{s}_hand'] + 3] * 0.6)
        pri.append(x[off[f'{s}_hand']:off[f'{s}_hand'] + 3] * 0.25)
    # no interpenetration of hands/fingertips/elbows with torso
    for name in ('left_hand', 'right_hand', 'index_tip_L', 'index_tip_R', 'middle_tip_L', 'middle_tip_R', 'left_lower_arm', 'right_lower_arm'):
        pri.append([torso_inside(A[fastfk.NAME2ROW[name]]) * 25.0])
    return np.concatenate([r] + [np.ravel(p) for p in pri])


HAND_CHAIN = {}
for _s in ('left', 'right'):
    chain = [b for b in skel.ORDER if b.startswith(_s + '_') and any(f in b for f in ('thumb', 'index', 'middle', 'ring', 'little'))]
    HAND_CHAIN[_s] = chain
TIP_PTS = {s_: [k for k in skel.PTS if k.endswith('_' + ('L' if s_ == 'left' else 'R')) and 'tip' in k] for s_ in ('left', 'right')}


def hand_fk(s, h, Gw, Pw):
    rv = P.hand_rotvecs(s, h)
    G = {f'{s}_hand': Gw}; Pp = {f'{s}_hand': Pw}
    for b in HAND_CHAIN[s]:
        p = skel.PARENT[b]
        G[b] = G[p] @ fastfk.rodrigues(np.asarray(rv[b])); Pp[b] = Pp[p] + G[p] @ (skel.REST[b] - skel.REST[p])
    for k in TIP_PTS[s]:
        bb, rp = skel.PTS[k]; Pp[k] = Pp[bb] + G[bb] @ (rp - skel.REST[bb])
    return Pp


def hand_resid(h, s, wrist, K, W, h_prev):
    Gw, Pw = wrist
    Pp = hand_fk(s, h, Gw, Pw)
    idx = HAND_IDX[s]
    X = np.array([Pp[skel.COCO[i]] for i in idx])
    uv = project(X, CAM)
    w = np.array([W[i] for i in idx])
    r = ((uv - K[idx]) * w[:, None] / 4.0).ravel()
    pri = [(h - h_prev) * 1.0, (h - P.hand_init()) * 0.25]
    # flex limits: no hyper-extension beyond -10deg, no more than 100deg
    j = 0; lim = []
    for b, n in P.HAND_BLOCKS[s]:
        if n in (1, 2):
            lim += [min(0.0, h[j] + np.radians(10)) * 20.0, max(0.0, h[j] - np.radians(100)) * 20.0]
            if n == 2:
                lim += [h[j + 1] * 2.0]
        j += n
    return np.concatenate([r] + pri + [np.array(lim)])



if MAIN:
    out = []
    x_prev = x_idle.copy(); h_prev = {s: P.hand_init() for s in ('left', 'right')}
    if len(sys.argv) > 5:                      # warm start from a previous fit frame: file.json@time
        fp, tt = sys.argv[5].split('@'); tt = float(tt)
        fr = min(json.load(open(fp))['frames'], key=lambda f: abs(f['t'] - tt))
        x_prev = np.array(fr['body']); h_prev = {s: np.array(fr['hands'][s]) for s in ('left', 'right')}
        print('warm start from', fp, fr['t'])
    t_start = time.time()
    for k, p in enumerate(poses):
        K = p['K']; W = weights_for(p['S'], p['O'], K)
        wr_obs = {'left': bool(W[9] > 0 or W[91] > 0), 'right': bool(W[10] > 0 or W[112] > 0)}
        hs = dict(h_prev)
        x = x_prev
        for it in range(1):                       # body, then fingers
            rvh = {}; rvh.update(P.hand_rotvecs('left', hs['left'])); rvh.update(P.hand_rotvecs('right', hs['right']))
            sol = least_squares(body_resid, x, args=(K, W, x_prev, wr_obs, rvh), method='trf', x_scale='jac', max_nfev=60, ftol=1e-6, xtol=1e-6)
            x = sol.x
            rvb = P.body_rotvecs(x)
            Ab, Gb = fastfk.fk_all(rvb)
            for s in ('left', 'right'):
                if W[HAND_IDX[s]].sum() > 3:
                    wi = fastfk.NAME2ROW[f'{s}_hand']
                    wrist = (Gb[wi], Ab[wi])
                    hsol = least_squares(hand_resid, hs[s], args=(s, wrist, K, W, h_prev[s]), method='trf', x_scale='jac', max_nfev=60)
                    hs[s] = hsol.x
                else:
                    hs[s] = h_prev[s] * 0.9 + P.hand_init() * 0.1
        rv = dict(rvb); rv.update(P.hand_rotvecs('left', hs['left'])); rv.update(P.hand_rotvecs('right', hs['right']))
        A, G = fastfk.fk_all(rv)
        uvb = project(A[[ROW[i] for i in BODY_IDX]], CAM)
        wb = np.array([W[i] for i in BODY_IDX])
        e = np.linalg.norm(uvb - K[BODY_IDX], axis=1)
        berr = float((e * (wb > 0)).sum() / max(1, (wb > 0).sum()))
        out.append(dict(t=p['t'], frame=p['frame'], body=x.tolist(), hands={s: hs[s].tolist() for s in hs},
                        rotvecs={b: np.asarray(v).tolist() for b, v in rv.items()}, err=berr, wr_obs=wr_obs))
        x_prev = x; h_prev = hs
        if k % 20 == 0:
            print(f"{k}/{len(poses)} t={p['t']:.1f} err={berr:.2f}px nfev={sol.nfev} elapsed={time.time()-t_start:.0f}s", flush=True)
    json.dump(dict(cam=CAM.tolist(), cy=724.0, frames=out), open(OUT, 'w'))
    print('saved', OUT, len(out), 'mean err', np.mean([o['err'] for o in out]))
