"""Move the hand's roll about the forearm axis into the forearm twist (FK-identical; avoids candy-wrapper wrists)."""
import json, sys, numpy as np
from scipy.spatial.transform import Rotation as Rot
SHARE = 1.0

def swing_twist(q):                      # q = x,y,z,w ; twist about local X
    tw = np.array([q[0], 0, 0, q[3]]); n = np.linalg.norm(tw)
    if n < 1e-9:
        return Rot.from_quat(q), 0.0
    tw /= n; tau = 2 * np.arctan2(tw[0], tw[3])
    tau = (tau + np.pi) % (2 * np.pi) - np.pi
    sw = Rot.from_quat(q) * Rot.from_rotvec([-tau, 0, 0])      # q = swing * twist
    return sw, tau

def fix_frame(rv):
    out = {k: np.asarray(v).tolist() for k, v in rv.items()}; info = {}
    for s in ('left', 'right'):
        q = Rot.from_rotvec(rv[f'{s}_hand']).as_quat()
        sw, tau = swing_twist(q)
        t = SHARE * tau
        out[f'{s}_lower_arm'] = (Rot.from_rotvec(rv[f'{s}_lower_arm']) * Rot.from_rotvec([t, 0, 0])).as_rotvec().tolist()
        out[f'{s}_hand'] = (Rot.from_rotvec([0, 0, 0]) * sw * Rot.from_rotvec([tau - t, 0, 0])).as_rotvec().tolist()
        info[s] = (np.degrees(sw.magnitude()), np.degrees(tau))
    return out, info

if __name__ == '__main__':
    for fn in sys.argv[1:]:
        d = json.load(open(fn)); sw = {'left': [], 'right': []}; tw = {'left': [], 'right': []}
        for f in d['frames']:
            f['rotvecs'], info = fix_frame({k: np.array(v) for k, v in f['rotvecs'].items()})
            for s in info: sw[s].append(info[s][0]); tw[s].append(info[s][1])
        out = fn.replace('.json', '_tw.json'); json.dump(d, open(out, 'w'))
        print(fn, ' '.join(f"{s}: swing med {np.median(sw[s]):.0f} max {np.max(sw[s]):.0f} | twist med {np.median(tw[s]):.0f} range {np.min(tw[s]):.0f}..{np.max(tw[s]):.0f}" for s in sw))


def fix_sequence(frames, share=0.5):
    """Share each hand's roll with the forearm (FK-identical: both rotate about the forearm's rest axis), roll unwrapped over time."""
    import skel
    prev = {'left': None, 'right': None}
    AX = {s: (skel.REST[f'{s}_hand'] - skel.REST[f'{s}_lower_arm']) / np.linalg.norm(skel.REST[f'{s}_hand'] - skel.REST[f'{s}_lower_arm']) for s in prev}
    for f in frames:
        rv = {k: np.asarray(v) for k, v in f['rotvecs'].items()}
        for s in ('left', 'right'):
            d = AX[s]; q = Rot.from_rotvec(rv[f'{s}_hand']).as_quat()
            p = np.dot(q[:3], d) * d; tw = np.array([*p, q[3]]); n = np.linalg.norm(tw)
            tau = 0.0 if n < 1e-9 else 2 * np.arctan2(np.dot(tw[:3] / n, d), tw[3] / n)
            if prev[s] is not None:
                tau += 2 * np.pi * np.round((prev[s] - tau) / (2 * np.pi))
            prev[s] = tau; t = share * tau
            rv[f'{s}_lower_arm'] = (Rot.from_rotvec(rv[f'{s}_lower_arm']) * Rot.from_rotvec(d * t)).as_rotvec()
            rv[f'{s}_hand'] = (Rot.from_rotvec(-d * t) * Rot.from_rotvec(rv[f'{s}_hand'])).as_rotvec()
        f['rotvecs'] = {k: np.asarray(v).tolist() for k, v in rv.items()}
    return frames
