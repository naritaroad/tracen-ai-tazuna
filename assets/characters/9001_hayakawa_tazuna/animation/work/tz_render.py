"""Render an action through the fitted source camera into a PNG sequence (chunked, spring bones simulated)."""
import bpy, os, json
import tz_anim as T


def setup(action_name, face_action_name=None, out_dir=None, scale=50):
    sc = bpy.context.scene; a = T.arm()
    a.animation_data_create(); a.animation_data.action = bpy.data.actions[action_name]
    act = a.animation_data.action
    if hasattr(act, 'slots') and act.slots:
        a.animation_data.action_slot = act.slots[0]
    if face_action_name:
        a.data.animation_data_create(); fa = bpy.data.actions[face_action_name]
        a.data.animation_data.action = fa
        if hasattr(fa, 'slots') and fa.slots:
            a.data.animation_data.action_slot = fa.slots[0]
    elif a.data.animation_data:
        a.data.animation_data.action = None
    cam = json.load(open(os.path.join(T.WORK, 'candidates.json')))['2200']['cam']
    T.set_cam(cam)
    sc.render.resolution_percentage = scale
    sc.render.engine = 'BLENDER_EEVEE'
    sc.render.film_transparent = True
    sc.render.image_settings.file_format = 'PNG'; sc.render.image_settings.color_mode = 'RGBA'
    sb = a.data.vrm_addon_extension.spring_bone1
    sb.enable_animation = True
    os.makedirs(out_dir, exist_ok=True)
    return sc


def render_range(out_dir, f0, f1):
    sc = bpy.context.scene; a = T.arm()
    if f0 == sc.frame_start:
        bpy.ops.vrm.reset_spring_bone1_animation_state(armature_object_name=a.name)
        for f in range(max(1, f0 - 45), f0):            # pre-roll so hair settles before the first frame
            sc.frame_set(f0)
    for f in range(f0, f1 + 1):
        sc.frame_set(f)
        sc.render.filepath = os.path.join(out_dir, f'{f - sc.frame_start + 1:04d}.png')
        bpy.ops.render.render(write_still=True)
