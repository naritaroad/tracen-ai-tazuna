import numpy as np
from scipy.spatial.transform import Rotation as Rot
import skel

FING = ['index', 'middle', 'ring', 'little']
BODY_BLOCKS = [('hips', 3), ('spine', 3), ('chest', 3), ('upper_chest', 3), ('neck', 3), ('head', 3),
               ('left_shoulder', 3), ('right_shoulder', 3), ('left_upper_arm', 3), ('right_upper_arm', 3),
               ('left_lower_arm', 2), ('right_lower_arm', 2), ('left_hand', 3), ('right_hand', 3)]
NB = sum(n for _, n in BODY_BLOCKS)


def hand_blocks(s):
    b = [(f'{s}_thumb_metacarpal', 3), (f'{s}_thumb_proximal', 1), (f'{s}_thumb_distal', 1)]
    for f in FING:
        b += [(f'{s}_{f}_proximal', 2), (f'{s}_{f}_intermediate', 1), (f'{s}_{f}_distal', 1)]
    return b
HAND_BLOCKS = {s: hand_blocks(s) for s in ('left', 'right')}
NH = sum(n for _, n in HAND_BLOCKS['left'])


def _dir(b):
    """rest direction of a finger segment"""
    nxt = {'proximal': 'intermediate', 'intermediate': 'distal', 'metacarpal': 'proximal'}
    s, f, seg = b.split('_')
    if f == 'thumb':
        nxt = {'metacarpal': 'proximal', 'proximal': 'distal'}
    if seg == 'distal':
        S = 'L' if s == 'left' else 'R'
        tip = skel.PTS[f'{f}_tip_{S}'][1]
        d = tip - skel.REST[b]
    else:
        d = skel.REST[f'{s}_{f}_{nxt[seg]}'] - skel.REST[b]
    return d / np.linalg.norm(d)

FLEX_AXIS = {}
SPREAD_AXIS = {}
for s in ('left', 'right'):
    for b, n in HAND_BLOCKS[s]:
        d = _dir(b)
        a = np.cross(d, [0, 0, -1.0]); FLEX_AXIS[b] = a / np.linalg.norm(a)   # +angle curls toward palm (palm faces -Z in rest)
        SPREAD_AXIS[b] = np.array([0, 0, 1.0])
# elbow: + flex = forearm toward front (-Y)
ELBOW_SIGN = {'left': -1.0, 'right': 1.0}


def body_rotvecs(x):
    rv = {}; i = 0
    for b, n in BODY_BLOCKS:
        v = x[i:i + n]; i += n
        if b.endswith('lower_arm'):
            side = b.split('_')[0]
            flex, twist = v
            R = Rot.from_rotvec([0, 0, ELBOW_SIGN[side] * flex]) * Rot.from_rotvec([twist, 0, 0])
            rv[b] = R.as_rotvec()
        else:
            rv[b] = np.array(v)
    # legs stay vertical in world: counter-rotate against hips
    inv = Rot.from_rotvec(rv['hips']).inv().as_rotvec()
    rv['left_upper_leg'] = inv; rv['right_upper_leg'] = inv
    return rv


def hand_rotvecs(s, h):
    rv = {}; i = 0
    for b, n in HAND_BLOCKS[s]:
        v = h[i:i + n]; i += n
        if n == 3:
            rv[b] = np.array(v)
        elif n == 2:
            R = Rot.from_rotvec(SPREAD_AXIS[b] * v[1]) * Rot.from_rotvec(FLEX_AXIS[b] * v[0])
            rv[b] = R.as_rotvec()
        else:
            rv[b] = FLEX_AXIS[b] * v[0]
    return rv


def body_init():
    x = np.zeros(NB); i = 0; off = {}
    for b, n in BODY_BLOCKS:
        off[b] = i; i += n
    x[off['left_upper_arm']:off['left_upper_arm'] + 3] = [0, np.radians(65), 0]
    x[off['right_upper_arm']:off['right_upper_arm'] + 3] = [0, -np.radians(65), 0]
    x[off['left_lower_arm']] = np.radians(70); x[off['right_lower_arm']] = np.radians(70)
    return x, off


def hand_init():
    h = np.zeros(NH); i = 0
    for b, n in HAND_BLOCKS['left']:
        if n == 2 or n == 1:
            h[i] = np.radians(15)
        i += n
    return h
