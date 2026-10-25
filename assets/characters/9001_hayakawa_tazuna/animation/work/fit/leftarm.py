"""Refit the hanging left arm in chest/chin phases using the faint wrist detections seen through the translucent dialogue panel."""
import sys, json, numpy as np
FILES = sys.argv[1:]
sys.argv = ['x']
from scipy.optimize import least_squares
import stage3 as S3, stage2 as S2, params as P, fastfk, common
from common import load_poses, weights_for, project
ps = {round(p['t'], 2): p for p in load_poses('../ref/animation_reference/qQ40c6kgV8M/poses.jsonl')}
idx = list(range(S2.off['left_upper_arm'], S2.off['left_upper_arm'] + 3)) + list(range(S2.off['left_lower_arm'], S2.off['left_lower_arm'] + 2))
for fn in FILES:
    d = json.load(open(fn)); x_prev = None; moved = []
    for f in d['frames']:
        t = f['t']; idle, chest, chin = S3.phases(t)
        if max(chest, chin) <= 0:
            x_prev = None; continue
        p = ps[round(t, 2)]; K = p['K']; S = p['S']; W = weights_for(S, p['O'], K).copy()
        x0 = np.array(f['body'])
        for j in (9, 91):
            ui = common.UI; inui = ui[0] < K[j, 0] < ui[2] and ui[1] < K[j, 1] < ui[3]
            if inui and S[j] > 0.45:
                W[j] = 0.6 * S[j] * common.IMPORTANCE.get(j, 1.0) if hasattr(common, 'IMPORTANCE') and isinstance(common.IMPORTANCE, dict) else 0.6 * S[j]
            elif W[j] > 0 and K[j, 1] < 1000 and chest > 0:
                W[j] *= 0.2          # isolated detections on the belt line are spurious (hand visibly hangs lower)
        S3.TORSO_REF['x'] = x0
        hs = {s: np.array(f['hands'][s]) for s in ('left', 'right')}
        rvh = {}; rvh.update(P.hand_rotvecs('left', hs['left'])); rvh.update(P.hand_rotvecs('right', hs['right']))
        wr_obs = {'left': False, 'right': True}
        xp = x0.copy() if x_prev is None else x_prev
        def r(z):
            x = x0.copy(); x[idx] = z
            return S3.body_resid3(x, K, W, xp, wr_obs, rvh, t)
        sol = least_squares(r, (xp if x_prev is not None else x0)[idx], method='trf', x_scale='jac', max_nfev=60, ftol=1e-6, xtol=1e-6)
        x = x0.copy(); x[idx] = sol.x
        rv = P.body_rotvecs(x); rv.update(rvh)
        A, G = fastfk.fk_all(rv)
        uv = project(A[fastfk.NAME2ROW['left_hand']], S3.CAM)[0]
        moved.append((t, float(np.linalg.norm(uv - K[9]))))
        f['body'] = x.tolist(); f['rotvecs'] = {b: np.asarray(v).tolist() for b, v in rv.items()}
        x_prev = x
    json.dump(d, open(fn, 'w'))
    m = np.array([e for _, e in moved]) if moved else np.zeros(1)
    print(fn, 'left-arm frames', len(moved), 'L wrist vs det mean %.1f px median %.1f' % (m.mean(), np.median(m)))
