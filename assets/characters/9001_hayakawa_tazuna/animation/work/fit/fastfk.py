"""Fast numpy FK equivalent to skel.fk (Rodrigues, no scipy objects)."""
import numpy as np
import skel

ORDER = skel.ORDER
PIDX = [skel.ORDER.index(skel.PARENT[b]) if skel.PARENT[b] else -1 for b in ORDER]
REST = np.array([skel.REST[b] for b in ORDER])
OFFS = np.array([REST[i] - (REST[p] if p >= 0 else 0) for i, p in enumerate(PIDX)])
PT_NAMES = list(skel.PTS.keys())
PT_BONE = np.array([ORDER.index(skel.PTS[k][0]) for k in PT_NAMES])
PT_OFF = np.array([skel.PTS[k][1] - skel.REST[skel.PTS[k][0]] for k in PT_NAMES])
NAME2ROW = {b: i for i, b in enumerate(ORDER)}
NAME2ROW.update({k: len(ORDER) + i for i, k in enumerate(PT_NAMES)})


def rodrigues(v):
    th = np.sqrt(v @ v)
    if th < 1e-12:
        return np.eye(3)
    k = v / th; K = np.array([[0, -k[2], k[1]], [k[2], 0, -k[0]], [-k[1], k[0], 0]])
    return np.eye(3) + np.sin(th) * K + (1 - np.cos(th)) * (K @ K)


def fk_all(rotvecs, hips_t=None):
    """returns array (len(ORDER)+len(PTS), 3) of world positions and list of global rotations"""
    n = len(ORDER)
    G = [None] * n; P = np.zeros((n, 3))
    for i, b in enumerate(ORDER):
        rv = rotvecs.get(b)
        L = rodrigues(np.asarray(rv)) if rv is not None else np.eye(3)
        p = PIDX[i]
        if p < 0:
            G[i] = L; P[i] = REST[i] + (hips_t if hips_t is not None else 0)
        else:
            G[i] = G[p] @ L; P[i] = P[p] + G[p] @ OFFS[i]
    pts = np.array([P[PT_BONE[j]] + G[PT_BONE[j]] @ PT_OFF[j] for j in range(len(PT_NAMES))])
    return np.vstack([P, pts]), G
