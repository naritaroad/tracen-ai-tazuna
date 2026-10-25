"""Re-solve fingers only (body kept) with updated finger priors."""
import sys, os, json, numpy as np
SRC, OUT = sys.argv[1], sys.argv[2]
sys.argv = ['x']
from scipy.optimize import least_squares
import stage3 as S3, stage2 as S2, params as P, fastfk
from common import load_poses, weights_for
d = json.load(open(SRC))
ps = {round(p['t'], 2): p for p in load_poses('../ref/animation_reference/qQ40c6kgV8M/poses.jsonl')}
h_prev = None
for f in d['frames']:
    p = ps[round(f['t'], 2)]; K = p['K']; W = weights_for(p['S'], p['O'], K)
    idle, chest, chin = S3.phases(f['t'])
    if idle > 0:
        W = W.copy(); W[92:112] *= 0.3; W[113:133] *= 0.3
    if os.environ.get('TZ_LEFT_POLICY'):
        import orient_policy; W = orient_policy.left_policy(W, p, f['t'])
    x = np.array(f['body']); rvb = P.body_rotvecs(x); Ab, Gb = fastfk.fk_all(rvb)
    hs = {s: np.array(f['hands'][s]) for s in ('left', 'right')}
    if h_prev is None: h_prev = dict(hs)
    for s in ('left', 'right'):
        wi = S3.R_[f'{s}_hand']
        sol = least_squares(S3.hand_resid3, h_prev[s], args=(s, (Gb[wi], Ab[wi]), K, W, h_prev[s], f['t'], Ab, Gb), method='trf', x_scale='jac', max_nfev=80)
        hs[s] = sol.x
    rv = dict(rvb); rv.update(P.hand_rotvecs('left', hs['left'])); rv.update(P.hand_rotvecs('right', hs['right']))
    f['hands'] = {s: hs[s].tolist() for s in hs}; f['rotvecs'] = {b: np.asarray(v).tolist() for b, v in rv.items()}
    h_prev = hs
json.dump(d, open(OUT, 'w')); print('refingered', SRC, '->', OUT, len(d['frames']))
