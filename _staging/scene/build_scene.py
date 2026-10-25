# Builds _staging/scene/corridor.glb from the local game PMX (vrm/scene/, not in git).
# The PMX export lost its material table, so each mesh island is given the texture atlas its UVs
# land on (black or transparent areas of an atlas mean "not this one"); everything else uses base00.
# Run headless: blender -b --python _staging/scene/build_scene.py
import bpy, importlib, numpy as np, sys, math
from mathutils import Vector
ROOT = r"C:\Users\admin\dev\tracen-aas-tsr\tracen-aas-tsr"
SRC = ROOT + r"\vrm\scene\pfb_env_cutin1053_00_00_corridor (UmaContainerProp).pmx"
TEX = ROOT + r"\vrm\scene\Texture2D\tex_env_cutin1053_00_00_%s.png"
OUT = sys.argv[sys.argv.index("--")+1] if "--" in sys.argv else ROOT + r"\_staging\scene\corridor.glb"
SCALE = 0.08
pmx = importlib.import_module("bl_ext.user_default.mmd_tools.core.pmx")
m = pmx.load(SRC)
for o in list(bpy.data.objects): bpy.data.objects.remove(o)
names = ("base00", "alpha00", "patramp00")
imgs = {n: bpy.data.images.load(TEX % n) for n in names}
px = {n: np.array(imgs[n].pixels[:], dtype=np.float32).reshape(imgs[n].size[1], imgs[n].size[0], 4) for n in names}
uv = np.array([v.uv for v in m.vertices]); co = np.array([v.co for v in m.vertices])
# islands
par = list(range(len(uv)))
def find(x):
    while par[x] != x: par[x] = par[par[x]]; x = par[x]
    return x
for f in m.faces:
    a, b, c = [find(v) for v in f]; par[b] = a; par[c] = a
isl = {}
for i, f in enumerate(m.faces): isl.setdefault(find(f[0]), []).append(i)
def samp(img, u, v):
    h, w, _ = img.shape
    x = np.clip((u % 1) * w, 0, w - 1).astype(int); y = np.clip((v % 1) * h, 0, h - 1).astype(int)
    return img[h - 1 - y, x]
face_tex = np.zeros(len(m.faces), dtype=int)
counts = [0, 0, 0]
for k, fs in isl.items():
    cu = np.array([uv[list(m.faces[i])].mean(0) for i in fs])
    score = []
    for n in names:
        s = samp(px[n], cu[:, 0], cu[:, 1])
        bad = (s[:, :3].max(1) < 0.03) | (s[:, 3] < 0.5)
        score.append(bad.mean())
    t = 0 if score[0] <= min(score) + 1e-6 else int(np.argmin(score))
    face_tex[fs] = t; counts[t] += len(fs)
print("TEXCOUNT", dict(zip(names, counts)))
me = bpy.data.meshes.new("corridor")
verts = [(c[0] * SCALE, c[2] * SCALE, c[1] * SCALE) for c in co]
faces = [(f[2], f[1], f[0]) for f in m.faces]
me.from_pydata(verts, [], faces)
uvl = me.uv_layers.new(name="UVMap")
loop_v = np.zeros(len(me.loops), dtype=int); me.loops.foreach_get("vertex_index", loop_v)
uvs = uv[loop_v].copy(); uvs[:, 1] = 1 - uvs[:, 1]
uvl.data.foreach_set("uv", uvs.ravel())
# custom normals from pmx
nrm = np.array([v.normal for v in m.vertices])
me.normals_split_custom_set_from_vertices([(n[0], n[2], n[1]) for n in nrm])
for n in names:
    mat = bpy.data.materials.new("corridor_" + n); mat.use_nodes = True
    nt = mat.node_tree; bsdf = next(x for x in nt.nodes if x.type == "BSDF_PRINCIPLED")
    tx = nt.nodes.new("ShaderNodeTexImage"); tx.image = imgs[n]
    nt.links.new(tx.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.85
    if n == "alpha00":
        nt.links.new(tx.outputs["Alpha"], bsdf.inputs["Alpha"])
        try: mat.surface_render_method = "DITHERED"
        except Exception: pass
    me.materials.append(mat)
me.polygons.foreach_set("material_index", face_tex)
me.update()
ob = bpy.data.objects.new("corridor", me); bpy.context.scene.collection.objects.link(ob)
mn = np.array(verts).min(0); mx = np.array(verts).max(0); print("BBOX", mn.round(2), mx.round(2))
if OUT:
    bpy.ops.object.select_all(action="DESELECT"); ob.select_set(True)
    bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", use_selection=True, export_image_format="WEBP", export_image_quality=90, export_yup=True, export_apply=False)
    print("EXPORTED", OUT)
bpy.ops.wm.save_as_mainfile(filepath=ROOT + r"\_staging\scene\corridor_build.blend")
