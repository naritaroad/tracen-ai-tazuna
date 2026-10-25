import numpy as np, json, common
common.CY = 724.0
import stage1_core as S, fastfk, params as P, skel
from stage3 import surf_y, palm_points, gap
from scipy.optimize import least_squares
def resid(z, f):
    S.FIXF[0] = f
    base = S.resid(z)
    cam = z[:7]; x = z[7:]
    rv = P.body_rotvecs(x); A, G = fastfk.fk_all(rv)
    lp, _ = palm_points(A, G, 'left'); rp, _ = palm_points(A, G, 'right')
    c = [300 * (gap(lp, A, G, 'spine') + 0.004), 300 * (gap(rp, A, G, 'spine') + 0.026)]
    return np.concatenate([base, c])
res = {}
for f in [1224, 1600, 2200, 3000, 4000]:
    c0 = S.cam0.copy(); c0[1] = -f / 1224 * 0.92; c0[6] = f
    z0 = np.concatenate([c0, S.x0])
    sol = least_squares(resid, z0, args=(f,), method='trf', x_scale='jac', max_nfev=3000)
    z = sol.x; cam = z[:7].copy(); cam[6] = f; x = z[7:]
    rv = P.body_rotvecs(x); A, G = fastfk.fk_all(rv)
    X = np.array([A[fastfk.NAME2ROW[skel.COCO[i]]] for i in S.use]); uv = common.project(X, cam)
    err = np.linalg.norm(uv - S.K[S.use], axis=1); body = [j for j, i in enumerate(S.use) if i < 23 and i not in (11, 12)]
    lp, _ = palm_points(A, G, 'left')
    res[str(f)] = dict(cam=cam.tolist(), rotvecs={k: list(map(float, v)) for k, v in rv.items()}, err=float(err[body].mean()), cost=float(sol.cost))
    print(f"f={f} cost={sol.cost:.3f} bodyErr={err[body].mean():.2f}px C={np.round(cam[:3],2)} pitch={np.degrees(cam[4]):.1f} gapL={gap(lp,A,G,'spine')*100:.1f}cm hips={np.round(np.degrees(x[S.off['hips']:S.off['hips']+3]),1)} spine={np.round(np.degrees(x[S.off['spine']:S.off['spine']+3]),1)}", flush=True)
json.dump(res, open('candidates_contact.json', 'w'))
