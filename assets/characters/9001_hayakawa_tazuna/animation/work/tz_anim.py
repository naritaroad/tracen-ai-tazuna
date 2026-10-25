"""Blender-side helpers for Tazuna animation fitting (imported from the work folder)."""
import bpy, json, os, math
from mathutils import Matrix, Vector, Quaternion

WORK = r"C:\Users\admin\dev\tracen-aas-tsr\tracen-aas-tsr\assets\characters\9001_hayakawa_tazuna\animation\work"


def arm():
    return bpy.data.objects['Tazuna_Armature']


def human_map():
    hb = arm().data.vrm_addon_extension.vrm1.humanoid.human_bones
    out = {}
    for p in hb.bl_rna.properties:
        v = getattr(hb, p.identifier, None)
        if hasattr(v, 'node') and v.node.bone_name:
            out[p.identifier] = v.node.bone_name
    return out


def reset_pose():
    for pb in arm().pose.bones:
        pb.rotation_mode = 'QUATERNION'; pb.rotation_quaternion = (1, 0, 0, 0); pb.location = (0, 0, 0); pb.scale = (1, 1, 1)


def local_to_basis(bone_name, Lq):
    """world-aligned local rotation (normalized humanoid) -> Blender pose-bone basis quaternion"""
    R = arm().data.bones[bone_name].matrix_local.to_3x3()
    return (R.inverted() @ Lq.to_matrix() @ R).to_quaternion()


def basis_to_local(bone_name, Bq):
    R = arm().data.bones[bone_name].matrix_local.to_3x3()
    return (R @ Bq.to_matrix() @ R.inverted()).to_quaternion()


def rv_to_quat(rv):
    v = Vector(rv); a = v.length
    return Quaternion(v.normalized(), a) if a > 1e-9 else Quaternion()


def set_pose(rotvecs, hips_t=None, H=None):
    H = H or human_map(); a = arm()
    reset_pose()
    for vrm, rv in rotvecs.items():
        if vrm not in H:
            continue
        a.pose.bones[H[vrm]].rotation_quaternion = local_to_basis(H[vrm], rv_to_quat(rv))
    if hips_t is not None:
        pb = a.pose.bones[H['hips']]
        R = a.data.bones[H['hips']].matrix_local.to_3x3()
        pb.location = R.inverted() @ Vector(hips_t)
    bpy.context.view_layer.update()


def set_cam(cam, cy=724.0, name='FitCam', res=(1080, 1448)):
    sc = bpy.context.scene
    c = bpy.data.objects.get(name)
    if not c:
        c = bpy.data.objects.new(name, bpy.data.cameras.new(name))
        coll = bpy.data.collections.get('TestRig') or sc.collection
        coll.objects.link(c)
    C = Vector(cam[:3]); yaw, pitch, roll = cam[3:6]; f = cam[6]
    Rcw = Matrix.Rotation(roll, 3, 'Y') @ Matrix.Rotation(pitch, 3, 'X') @ Matrix.Rotation(yaw, 3, 'Z')
    right, fwd, up = Rcw.col[0], Rcw.col[1], Rcw.col[2]
    M = Matrix((right, up, -fwd)).transposed()
    c.matrix_world = Matrix.Translation(C) @ M.to_4x4()
    c.data.sensor_fit = 'HORIZONTAL'; c.data.sensor_width = 36.0
    c.data.lens = f * 36.0 / res[0]
    c.data.shift_x = 0.0
    c.data.shift_y = (res[1] / 2.0 - cy) / max(res)
    c.data.clip_start = 0.01
    sc.camera = c
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.resolution_percentage = 100
    return c
