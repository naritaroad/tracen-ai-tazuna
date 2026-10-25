"""Apply game clips (converted by uma2blend.py) to Tazuna_Armature: single frames or baked actions."""
import bpy, json, os
from mathutils import Quaternion, Vector
import tz_anim as T
GAME = os.path.join(T.WORK, 'game')
def load(name):
    return json.load(open(os.path.join(GAME, name + '.json')))
def apply_frame(d, f):
    a = T.arm(); fr = d['frames'][f]
    for pb in a.pose.bones:
        pb.rotation_mode = 'QUATERNION'
        pb.rotation_quaternion = Quaternion(fr['rot'][pb.name]) if pb.name in fr['rot'] else Quaternion()
        pb.location = Vector(fr['hip_loc']) if pb.name == 'Hip' else Vector()
    bpy.context.view_layer.update()
def build_action(d, act_name=None, frame_offset=1):
    import tz_build as B
    a = T.arm(); act_name = act_name or ('G_' + d['name'])
    if act_name in bpy.data.actions: bpy.data.actions.remove(bpy.data.actions[act_name])
    act = B.new_action(act_name, a, 'OBJECT'); fcs = B._fcurves(act)
    bones = sorted(d['frames'][0]['rot'])
    curves = {}
    for bn in bones:
        for i in range(4):
            fc = fcs.new(f'pose.bones["{bn}"].rotation_quaternion', index=i, action_group=bn); curves[(bn, i)] = fc
    loc = [fcs.new('pose.bones["Hip"].location', index=i, action_group='Hip') for i in range(3)]
    n = len(d['frames'])
    for (bn, i), fc in curves.items():
        fc.keyframe_points.add(n)
        prev = None
        for f, fr in enumerate(d['frames']):
            q = Quaternion(fr['rot'][bn])
            if prev is not None and prev.dot(q) < 0: q = -q
            prev = q
            fc.keyframe_points[f].co = (f + frame_offset, q[i]); fc.keyframe_points[f].interpolation = 'LINEAR'
    for i, fc in enumerate(loc):
        fc.keyframe_points.add(n)
        for f, fr in enumerate(d['frames']):
            fc.keyframe_points[f].co = (f + frame_offset, fr['hip_loc'][i]); fc.keyframe_points[f].interpolation = 'LINEAR'
    act.use_fake_user = True
    return act


def export_list(items, out_dir):
    """items: list of (clip_json_name, out_file_stem). Builds each action and exports a VRMA."""
    import tz_build as B
    a = T.arm(); os.makedirs(out_dir, exist_ok=True); done = []
    if a.data.animation_data: a.data.animation_data.action = None
    for jn, stem in items:
        d = load(jn); act = build_action(d)
        a.animation_data_create(); a.animation_data.action = act
        if hasattr(act, 'slots') and act.slots: a.animation_data.action_slot = act.slots[0]
        B.set_range(1, len(d['frames']))
        path = os.path.join(out_dir, stem + '.vrma'); B.export_vrma(path)
        done.append((stem, len(d['frames']), os.path.getsize(path)))
    return done
