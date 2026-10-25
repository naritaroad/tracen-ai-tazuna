"""Humanoid FK model of Tazuna in VRM-normalized (world-aligned) rotation space.

Blender world axes: X = character's left, Y = back (character faces -Y), Z = up.
Each humanoid bone has a local rotation L (axis-angle, world-aligned rest frame).
Global G = G_parent @ L ; head position = parent_head + G_parent @ (rest_head - rest_parent_head).
"""
import json, numpy as np
from scipy.spatial.transform import Rotation as Rot

SK = json.load(open(__file__.replace('skel.py', 'skeleton.json')))
HUM = SK['human']            # vrm name -> blender bone
BONES = SK['bones']
B2H = {v: k for k, v in HUM.items()}

def humanoid_parent(vrm):
    b = BONES[HUM[vrm]]['parent']
    while b is not None and b not in B2H:
        b = BONES[b]['parent']
    return B2H[b] if b else None

FINGERS = ['thumb', 'index', 'middle', 'ring', 'little']
ORDER = ['hips', 'spine', 'chest', 'upper_chest', 'neck', 'head', 'left_eye', 'right_eye']
for s in ('left', 'right'):
    ORDER += [f'{s}_upper_leg', f'{s}_lower_leg', f'{s}_foot', f'{s}_shoulder', f'{s}_upper_arm', f'{s}_lower_arm', f'{s}_hand']
    ORDER += [f'{s}_thumb_metacarpal', f'{s}_thumb_proximal', f'{s}_thumb_distal']
    for f in FINGERS[1:]:
        ORDER += [f'{s}_{f}_proximal', f'{s}_{f}_intermediate', f'{s}_{f}_distal']
PARENT = {b: humanoid_parent(b) for b in ORDER}
REST = {b: np.array(BONES[HUM[b]]['head']) for b in ORDER}
IDX = {b: i for i, b in enumerate(ORDER)}

# extra points attached to bones: name -> (bone, rest position)
import os
SHOULDER_DX = float(os.environ.get('TZ_SH_DX', '0.0'))
SHOULDER_DZ = float(os.environ.get('TZ_SH_DZ', '0.0'))
PTS = {}
L = SK['landmarks']; T = SK['tips']
PTS['nose'] = ('head', L['nose_tip']); PTS['chin'] = ('head', L['chin_tip']); PTS['mouth'] = ('head', L['mouth_center'])
PTS['jaw_L'] = ('head', L['jaw_L']); PTS['jaw_R'] = ('head', L['jaw_R']); PTS['ear_L'] = ('head', L['ear_L']); PTS['ear_R'] = ('head', L['ear_R'])
PTS['eye_L'] = ('head', BONES[HUM['left_eye']]['head']); PTS['eye_R'] = ('head', BONES[HUM['right_eye']]['head'])
for s, S in (('left', 'L'), ('right', 'R')):
    for f, F in zip(FINGERS, ['Thumb', 'Index', 'Middle', 'Ring', 'Pinky']):
        PTS[f'{f}_tip_{S}'] = (f'{s}_thumb_distal' if f == 'thumb' else f'{s}_{f}_distal', T[f'{F}_tip_{S}'])
# COCO 'shoulder' on anime renders sits on the top-outer shoulder silhouette, not the joint centre
for s_, S_, sx in (('left', 'L', 1), ('right', 'R', -1)):
    h = np.array(BONES[HUM[f'{s_}_upper_arm']]['head'])
    PTS[f'shoulder_kp_{S_}'] = (f'{s_}_upper_arm', h + np.array([SHOULDER_DX * sx, 0.0, SHOULDER_DZ]))
PTS = {k: (b, np.array(p)) for k, (b, p) in PTS.items()}


def fk(rotvecs, hips_t=np.zeros(3)):
    """rotvecs: dict bone -> rotvec (world-aligned local). returns heads, globals(dict of 3x3), points."""
    G = {}; P = {}
    for b in ORDER:
        rv = rotvecs.get(b)
        Lm = Rot.from_rotvec(rv).as_matrix() if rv is not None else np.eye(3)
        p = PARENT[b]
        if p is None:
            G[b] = Lm; P[b] = REST[b] + hips_t
        else:
            G[b] = G[p] @ Lm; P[b] = P[p] + G[p] @ (REST[b] - REST[p])
    pts = {k: P[b] + G[b] @ (rp - REST[b]) for k, (b, rp) in PTS.items()}
    return P, G, pts


# COCO-WholeBody index -> model point name
COCO = {0: 'nose', 1: 'eye_L', 2: 'eye_R', 3: 'ear_L', 4: 'ear_R',
        5: 'shoulder_kp_L', 6: 'shoulder_kp_R', 7: 'left_lower_arm', 8: 'right_lower_arm',
        9: 'left_hand', 10: 'right_hand', 11: 'left_upper_leg', 12: 'right_upper_leg',
        13: 'left_lower_leg', 14: 'right_lower_leg', 15: 'left_foot', 16: 'right_foot',
        23 + 8: 'chin', 23 + 4: 'jaw_R', 23 + 12: 'jaw_L', 23 + 30: 'nose', 23 + 62: 'mouth'}
# hands: 91 left wrist, 112 right wrist
for base, s, S in ((91, 'left', 'L'), (112, 'right', 'R')):
    COCO[base] = f'{s}_hand'
    COCO[base + 1] = f'{s}_thumb_metacarpal'; COCO[base + 2] = f'{s}_thumb_proximal'; COCO[base + 3] = f'{s}_thumb_distal'; COCO[base + 4] = f'thumb_tip_{S}'
    for i, f in enumerate(FINGERS[1:]):
        o = base + 5 + i * 4
        COCO[o] = f'{s}_{f}_proximal'; COCO[o + 1] = f'{s}_{f}_intermediate'; COCO[o + 2] = f'{s}_{f}_distal'; COCO[o + 3] = f'{f}_tip_{S}'


def point(name, P, pts):
    return P[name] if name in P else pts[name]
