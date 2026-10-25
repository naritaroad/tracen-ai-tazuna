"""Blender-side builder: body/face tracks -> Actions, clip cutting, loop blending, VRMA export."""
import bpy, json, os, math
from mathutils import Quaternion, Vector
import tz_anim as T

FPS = 30


def _fcurves(action):
    """Blender 5 layered actions: return (and create) the channelbag fcurves collection for the action's slot."""
    try:
        return action.fcurves
    except AttributeError:
        pass
    if not action.layers:
        layer = action.layers.new('Layer'); layer.strips.new(type='KEYFRAME')
    strip = action.layers[0].strips[0]
    slot = action.slots[0] if action.slots else None
    if slot is None:
        return None
    cb = strip.channelbag(slot, ensure=True)
    return cb.fcurves


def new_action(name, owner, id_type):
    old = bpy.data.actions.get(name)
    if old:
        bpy.data.actions.remove(old)
    act = bpy.data.actions.new(name)
    act.use_fake_user = True
    owner.animation_data_create()
    if hasattr(act, 'slots'):
        slot = act.slots.new(id_type=id_type, name=owner.name)
        owner.animation_data.action = act
        owner.animation_data.action_slot = slot
    else:
        owner.animation_data.action = act
    return act


def bake_quats(fc_coll, data_path_base, frames, quats, group=None):
    """frames: list of frame numbers; quats: list of (w,x,y,z)"""
    curves = []
    for i in range(4):
        fc = fc_coll.new(data_path=data_path_base, index=i, group_name=group or '')
        fc.keyframe_points.add(len(frames))
        co = []
        for f, q in zip(frames, quats):
            co += [f, q[i]]
        fc.keyframe_points.foreach_set('co', co)
        for kp in fc.keyframe_points:
            kp.interpolation = 'BEZIER'; kp.handle_left_type = kp.handle_right_type = 'AUTO_CLAMPED'
        fc.update(); curves.append(fc)
    return curves


def build_body_action(name, body30, t_from=None, t_to=None, frame_offset=1):
    a = T.arm(); H = T.human_map()
    act = new_action(name, a, 'OBJECT')
    fcs = _fcurves(act)
    ts = body30['t']
    idx = [i for i, t in enumerate(ts) if (t_from is None or t >= t_from - 1e-6) and (t_to is None or t <= t_to + 1e-6)]
    t0 = ts[idx[0]]
    frames = [round((ts[i] - t0) * FPS) + frame_offset for i in idx]
    for vrm, qs in body30['quats'].items():
        if vrm not in H:
            continue
        bn = H[vrm]
        bq = []
        prev = None
        for i in idx:
            x, y, z, w = qs[i]
            q = T.local_to_basis(bn, Quaternion((w, x, y, z)))
            if prev is not None and prev.dot(q) < 0:
                q = -q
            prev = q; bq.append((q.w, q.x, q.y, q.z))
        a.pose.bones[bn].rotation_mode = 'QUATERNION'
        bake_quats(fcs, f'pose.bones["{bn}"].rotation_quaternion', frames, bq, group=bn)
    return act, frames[0], frames[-1]


def expression_path(name):
    ex = T.arm().data.vrm_addon_extension.vrm1.expressions
    d = ex.all_name_to_expression_dict()
    return d[name].path_from_id('preview')


def build_face_action(name, tracks, fps, t_from, t_to, frame_offset=1, step=1):
    a = T.arm()
    act = new_action(name, a.data, 'ARMATURE')
    fcs = _fcurves(act)
    n = len(next(iter(tracks.values())))
    i0 = max(0, round(t_from * fps)); i1 = min(n - 1, round(t_to * fps))
    for ename, vals in tracks.items():
        path = 'vrm_addon_extension.vrm1.expressions.' + expression_path(ename).split('vrm1.expressions.', 1)[-1] \
            if 'vrm1.expressions' in expression_path(ename) else expression_path(ename)
        fc = fcs.new(data_path=path, index=0, group_name='expressions')
        idx = list(range(i0, i1 + 1, step))
        fc.keyframe_points.add(len(idx))
        co = []
        for i in idx:
            co += [round((i - i0) * FPS / fps) + frame_offset, float(vals[i])]
        fc.keyframe_points.foreach_set('co', co)
        for kp in fc.keyframe_points:
            kp.interpolation = 'LINEAR'
        fc.update()
    return act


def set_range(f0, f1):
    sc = bpy.context.scene
    sc.render.fps = FPS; sc.render.fps_base = 1.0
    sc.frame_start = f0; sc.frame_end = f1


def export_vrma(path):
    a = T.arm()
    bpy.context.view_layer.objects.active = a
    for o in bpy.context.selected_objects:
        o.select_set(False)
    a.select_set(True)
    if bpy.context.mode != 'OBJECT':
        bpy.ops.object.mode_set(mode='OBJECT')
    bpy.ops.object.mode_set(mode='POSE')
    r = bpy.ops.export_scene.vrma(filepath=path, armature_object_name=a.name)
    bpy.ops.object.mode_set(mode='OBJECT')
    return r
