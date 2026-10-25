import json, numpy as np
from scipy.spatial.transform import Rotation as Rot
import skel

CX, CY = 540.0, 724.0
UI = (59, 991, 1020, 1230)          # dialogue panel (x0,y0,x1,y1) in source px


def load_poses(path):
    rows = [json.loads(l) for l in open(path)]
    out = []
    for r in rows:
        if not r.get('target_present'):
            continue
        out.append(dict(t=r['t'], frame=r['frame'], K=np.array(r['keypoints']), S=np.array(r['scores']),
                        O=np.array(r['observable'], dtype=bool)))
    return out


def cam_matrix(cam):
    """cam = [Cx,Cy,Cz,yaw,pitch,roll,f]; returns (C, Rcw, f)"""
    C = np.array(cam[:3]); Rcw = Rot.from_euler('zxy', cam[3:6]).as_matrix(); return C, Rcw, cam[6]


def project(X, cam):
    C, Rcw, f = cam_matrix(cam)
    d = (np.atleast_2d(X) - C) @ Rcw          # rows: (right, forward, up)
    u = CX + f * d[:, 0] / d[:, 1]
    v = CY - f * d[:, 2] / d[:, 1]
    return np.stack([u, v], 1)


def weights_for(S, O, K):
    w = np.where(O, S, 0.0)
    w = np.where(S < 0.35, 0.0, w)
    inui = (K[:, 0] > UI[0]) & (K[:, 0] < UI[2]) & (K[:, 1] > UI[1]) & (K[:, 1] < UI[3])
    w = np.where(inui, 0.0, w)
    return w


# importance multipliers per COCO index
IMPORTANCE = {}
for i in range(133):
    IMPORTANCE[i] = 1.0
for i in (3, 4):
    IMPORTANCE[i] = 0.15           # ears hidden under hair on anime models
for i in (23 + 4, 23 + 12):
    IMPORTANCE[i] = 0.3            # jaw contour
for i in (23 + 8, 23 + 62):
    IMPORTANCE[i] = 0.6
for i in (11, 12):
    IMPORTANCE[i] = 0.4            # COCO hip landmark differs from joint centre
for i in range(91, 133):
    IMPORTANCE[i] = 0.6            # hand keypoints are noisier on anime hands
