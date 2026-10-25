"""Render given poses through the fitted source camera and compose crops: source | render | overlay."""
import bpy, os, json, subprocess
import tz_anim as T

def render_poses(pose_file, out_dir, scale=100):
    sc = bpy.context.scene; a = T.arm()
    if a.animation_data: a.animation_data.action = None
    if a.data.animation_data: a.data.animation_data.action = None
    a.data.vrm_addon_extension.spring_bone1.enable_animation = False
    cam = json.load(open(os.path.join(T.WORK, 'candidates.json')))['2200']['cam']; T.set_cam(cam)
    sc.render.resolution_x = 1080; sc.render.resolution_y = 1448; sc.render.resolution_percentage = scale
    sc.render.film_transparent = True; sc.render.engine = 'BLENDER_EEVEE'
    os.makedirs(out_dir, exist_ok=True); poses = json.load(open(pose_file)); names = []
    for t, rv in poses.items():
        T.reset_pose(); T.set_pose(rv); bpy.context.view_layer.update()
        sc.render.filepath = os.path.join(out_dir, f'pose_{t}.png'); bpy.ops.render.render(write_still=True); names.append(t)
    return names
