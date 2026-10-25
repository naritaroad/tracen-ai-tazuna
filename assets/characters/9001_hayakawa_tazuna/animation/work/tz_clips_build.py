"""Build all clip Actions in Blender from clips.json (+ face tracks) and export VRMA files + manifest."""
import bpy, json, os
from mathutils import Quaternion
import tz_anim as T
import tz_build as B

WORK = T.WORK
ANIM = os.path.dirname(WORK)                     # .../animation
VRMA_DIR = os.path.join(ANIM, 'vrma')


def build_clip_action(cid, clip):
    a = T.arm(); H = T.human_map()
    act = B.new_action(f'TZ_{cid}', a, 'OBJECT')
    fcs = B._fcurves(act)
    n = clip['frames']; frames = list(range(1, n + 1))
    for vrm, qs in clip['quats'].items():
        if vrm not in H:
            continue
        bn = H[vrm]; prev = None; bq = []
        for x, y, z, w in qs:
            q = T.local_to_basis(bn, Quaternion((w, x, y, z)))
            if prev is not None and prev.dot(q) < 0:
                q = -q
            prev = q; bq.append((q.w, q.x, q.y, q.z))
        a.pose.bones[bn].rotation_mode = 'QUATERNION'
        B.bake_quats(fcs, f'pose.bones["{bn}"].rotation_quaternion', frames, bq, group=bn)
    if clip['loop']:
        for fc in fcs:
            for kp in fc.keyframe_points:
                kp.interpolation = 'LINEAR'          # dense keys: linear keeps the loop seam exact
    return act, n


def assign(owner, act):
    owner.animation_data_create()
    owner.animation_data.action = act
    if hasattr(act, 'slots') and act.slots:
        owner.animation_data.action_slot = act.slots[0]


def build_all(export=True):
    clips = json.load(open(os.path.join(WORK, 'clips.json'), encoding='utf-8'))
    face = json.load(open(os.path.join(WORK, 'face_tracks.json'), encoding='utf-8'))
    os.makedirs(VRMA_DIR, exist_ok=True)
    a = T.arm()
    manifest = dict(version=1, fps=clips['fps'], model='../vrm/HayakawaTazuna.vrm', clips=[])
    for cid, c in clips['clips'].items():
        act, n = build_clip_action(cid, c)
        face_act = None
        if c.get('face'):
            t0 = c['source']['start']
            face_act = B.build_face_action(f'TZ_{cid}_face', face['tracks'], face['fps'], t0, t0 + (n - 1) / clips['fps'])
        assign(a, act)
        if face_act is not None:
            assign(a.data, face_act)
        elif a.data.animation_data:
            a.data.animation_data.action = None
        B.set_range(1, n)
        rel = f'vrma/{cid}.vrma'
        if export:
            B.export_vrma(os.path.join(ANIM, rel))
        entry = {k: v for k, v in c.items() if k not in ('quats', 'frames')}
        entry.update(id=cid, file=rel, frames=n, duration=round((n - 1) / clips['fps'], 4))
        manifest['clips'].append(entry)
        print(cid, n, 'frames', 'face' if face_act else '')
    json.dump(manifest, open(os.path.join(ANIM, 'animations.json'), 'w', encoding='utf-8'), ensure_ascii=False, indent=1)
    # leave the idle loop active for editing
    assign(a, bpy.data.actions['TZ_idle_polite'])
    if a.data.animation_data:
        a.data.animation_data.action = None
    B.set_range(1, clips['clips']['idle_polite']['frames'])
    return manifest
