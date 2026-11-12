# Builds the Warren rabbit: a rigged, animated NZ White, and exports it as glTF for the 3D hutch.
#
#   blender --background --python web/blender/rabbit.py -- web/src/assets/rabbit.glb [preview-prefix]
#
# Body: a Skin-modifier skeleton (spine, legs, ears) subdivided, then "sculpted" with smooth bumps
# (haunches, cheeks, brow, eye sockets). One mesh, skinned to an armature by automatic weights.
# Animations (in place; the app moves the rabbit across the floor): Hop, Idle, Look, SitUp.
# Faces +X, feet on z = 0, about 1 unit nose to tail. Inner ears and nose are vertex-painted pink
# (COLOR_0); the app adds fur shells on top.
import math
import sys

import bpy
from mathutils import Euler, Vector

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
OUT = argv[0] if argv else "rabbit.glb"
PREVIEW = argv[1] if len(argv) > 1 else None

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.fps = 30


def select_only(ob):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob


# ---------------------------------------------------------------- skeleton (proportions from reference photos)
# A crouched adult: the rump is the highest, roundest part; the back falls to a short thick neck; the
# head is a wedge with a tapered muzzle, carried below the rump. Slender front legs drop straight from
# a deep chest; long hind feet lie flat; a small tail turns up. name: (position, radius_xy, radius_z)
P = {
    "tail": ((-0.455, 0.0, 0.43), 0.045, 0.06),
    "rump": ((-0.24, 0.0, 0.37), 0.21, 0.215),
    "loin": ((-0.10, 0.0, 0.385), 0.185, 0.19),
    "mid": ((0.03, 0.0, 0.35), 0.16, 0.165),
    "chest": ((0.15, 0.0, 0.29), 0.14, 0.15),
    "neck": ((0.23, 0.0, 0.355), 0.11, 0.115),
    "head": ((0.31, 0.0, 0.435), 0.128, 0.13),
    "face": ((0.40, 0.0, 0.418), 0.098, 0.102),
    "muzzle": ((0.465, 0.0, 0.39), 0.06, 0.064),
    "nose": ((0.505, 0.0, 0.376), 0.03, 0.03),
}
for s, n in ((1, "L"), (-1, "R")):
    P[f"hip.{n}"] = ((-0.22, 0.10 * s, 0.30), 0.15, 0.16)
    P[f"knee.{n}"] = ((-0.10, 0.13 * s, 0.17), 0.10, 0.10)
    P[f"hock.{n}"] = ((-0.34, 0.13 * s, 0.045), 0.045, 0.05)
    P[f"toe.{n}"] = ((-0.02, 0.115 * s, 0.025), 0.033, 0.028)
    P[f"shoulder.{n}"] = ((0.14, 0.075 * s, 0.23), 0.065, 0.07)
    P[f"elbow.{n}"] = ((0.12, 0.08 * s, 0.13), 0.045, 0.05)
    P[f"paw.{n}"] = ((0.17, 0.075 * s, 0.025), 0.034, 0.03)
    P[f"earbase.{n}"] = ((0.29, 0.045 * s, 0.50), 0.0, 0.0)
    P[f"earmid.{n}"] = ((0.265, 0.075 * s, 0.70), 0.0, 0.0)
    P[f"eartip.{n}"] = ((0.235, 0.095 * s, 0.88), 0.0, 0.0)
EDGES = [("tail", "rump"), ("rump", "loin"), ("loin", "mid"), ("mid", "chest"), ("chest", "neck"), ("neck", "head"), ("head", "face"), ("face", "muzzle"), ("muzzle", "nose")]
for n in "LR":
    EDGES += [
        ("rump", f"hip.{n}"), (f"hip.{n}", f"knee.{n}"), (f"knee.{n}", f"hock.{n}"), (f"hock.{n}", f"toe.{n}"),
        ("chest", f"shoulder.{n}"), (f"shoulder.{n}", f"elbow.{n}"), (f"elbow.{n}", f"paw.{n}"),
    ]


def ellipsoid(center, rx, ry, rz):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=14, radius=1.0, location=center)
    o = bpy.context.view_layer.objects.active
    o.scale = (rx, ry, rz)
    return o


# ---------------------------------------------------------------- body: a dense chain of ellipsoids
# along every edge (radii blended smoothly), fused by a voxel remesh and smoothed into one surface
pieces = []
for k, (co, r, rz) in P.items():
    if not k.startswith("ear"):
        pieces.append(ellipsoid(co, r, r, rz))
for a_, b_ in EDGES:
    (pa, ar, az), (pb, br, bz) = P[a_], P[b_]
    pa, pb = Vector(pa), Vector(pb)
    steps = max(3, int((pb - pa).length / 0.02))
    for i in range(1, steps):
        t = i / steps
        u = t * t * (3 - 2 * t)
        r = ar + (br - ar) * u
        pieces.append(ellipsoid(pa.lerp(pb, t), r, r, az + (bz - az) * u))
bpy.ops.object.select_all(action="DESELECT")
for o in pieces:
    o.select_set(True)
bpy.context.view_layer.objects.active = pieces[0]
bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
bpy.ops.object.join()
body = bpy.context.view_layer.objects.active
body.name = "Rabbit"
rem = body.modifiers.new("Remesh", "REMESH")
rem.mode = "VOXEL"
rem.voxel_size = 0.006
bpy.ops.object.modifier_apply(modifier="Remesh")
lap = body.modifiers.new("Lap", "LAPLACIANSMOOTH")
lap.lambda_factor = 1.0
lap.iterations = 18
bpy.ops.object.modifier_apply(modifier="Lap")


def bump(center, radius, amount):
    c = Vector(center)
    body.data.update()
    for v in body.data.vertices:
        d = (v.co - c).length
        if d < radius * 2.5:
            v.co += v.normal * (math.exp(-((d / radius) ** 2)) * amount)


for s in (1, -1):
    bump((-0.20, 0.17 * s, 0.30), 0.10, 0.02)  # round haunches
    bump((0.415, 0.07 * s, 0.38), 0.05, 0.014)  # cheeks
    bump((0.396, 0.093 * s, 0.466), 0.03, -0.012)  # eye sockets
    bump((0.36, 0.06 * s, 0.50), 0.04, 0.008)  # brow
bump((0.0, 0.0, 0.19), 0.11, -0.02)  # tuck the belly
bump((0.20, 0.0, 0.20), 0.07, 0.014)  # chest ruff
bump((0.495, 0.0, 0.352), 0.022, -0.006)  # split lip
sm = body.modifiers.new("Smooth", "CORRECTIVE_SMOOTH")
sm.iterations = 6
bpy.ops.object.modifier_apply(modifier="Smooth")
dec = body.modifiers.new("Dec", "DECIMATE")
dec.ratio = 0.12
bpy.ops.object.modifier_apply(modifier="Dec")
bpy.ops.object.shade_smooth()

# ---------------------------------------------------------------- colour: NZ White with soft occlusion
# Base white with a cream back and brighter chest and belly; crevices darkened by ray-cast ambient
# occlusion baked into the vertex colours (the app adds a fur texture on top).
from mathutils.bvhtree import BVHTree

body.data.update()
bvh = BVHTree.FromObject(body, bpy.context.evaluated_depsgraph_get())
dirs = []
golden = math.pi * (3 - math.sqrt(5))
for i in range(24):
    zc = 1 - (i + 0.5) / 24
    r = math.sqrt(1 - zc * zc)
    dirs.append(Vector((math.cos(golden * i) * r, math.sin(golden * i) * r, zc)))


def occlusion(co, nrm):
    hits = 0
    total = 0
    tangent = nrm.orthogonal().normalized()
    bit = nrm.cross(tangent)
    for d in dirs:
        w = tangent * d.x + bit * d.y + nrm * d.z
        total += 1
        if bvh.ray_cast(co + nrm * 0.004, w, 0.12)[0] is not None:
            hits += 1
    return hits / total


col = body.data.color_attributes.new("Col", "FLOAT_COLOR", "POINT")
for v in body.data.vertices:
    up = v.normal.z
    base = Vector((0.955, 0.94, 0.915))
    if up > 0.4:  # back: a touch creamier
        base = base.lerp(Vector((0.93, 0.905, 0.865)), min(1, (up - 0.4) / 0.6) * 0.6)
    if up < -0.2 or (v.co.x > 0.12 and v.co.z < 0.3):  # belly and chest: brightest
        base = base.lerp(Vector((1.0, 0.99, 0.975)), 0.6)
    ao = occlusion(v.co, v.normal)
    shade = 1.0 - 0.55 * ao
    c = base * shade
    if (v.co - Vector((0.51, 0.0, 0.378))).length < 0.022:
        c = Vector((0.88, 0.55, 0.6)) * shade  # nose
    col.data[v.index].color = (c.x, c.y, c.z, 1.0)


# ---------------------------------------------------------------- ears: long, broad, flat, spoon-shaped
def ear(side):
    n = "L" if side > 0 else "R"
    base, tip = Vector(P[f"earbase.{n}"][0]), Vector(P[f"eartip.{n}"][0])
    length = (tip - base).length
    bpy.ops.mesh.primitive_uv_sphere_add(segments=28, ring_count=18, radius=1.0)
    ob = bpy.context.view_layer.objects.active
    me = ob.data
    for v in me.vertices:
        x, y, z = v.co
        t = (z + 1.0) * 0.5  # 0 at the base, 1 at the tip
        width = 0.062 * (0.45 + 0.85 * math.sin(math.pi * min(t * 0.98, 1.0)) ** 0.75) * (1 - 0.25 * t)
        thick = 0.011 * (1.15 - 0.5 * t)
        cup = 0.012 * math.sin(math.pi * t) * (1 - y * y)  # the front face is hollowed
        v.co = Vector((x * thick - (cup if x > 0 else cup * 0.6), y * width, t * length))
    me.update()
    # colour: pink inside (front face), white outside
    ec = me.color_attributes.new("Col", "FLOAT_COLOR", "POINT")
    for v in me.vertices:
        t = v.co.z / length
        inner = v.normal.x > 0.25 and 0.08 < t < 0.94 and abs(v.co.y) < 0.75 * 0.062 * (0.45 + 0.85 * math.sin(math.pi * t) ** 0.75)
        c = (0.9, 0.62, 0.66) if inner else (0.95, 0.935, 0.91)
        ec.data[v.index].color = (*c, 1.0)
    # stand it on its base, the hollow facing forward and a little outward
    d = (tip - base).normalized()
    q = d.to_track_quat("Z", "X")
    ob.rotation_euler = q.to_euler()
    ob.rotation_euler.rotate(Euler((0, 0, math.radians(-28 * side))))
    ob.location = base
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bpy.ops.object.shade_smooth()
    return ob


ears = [ear(1), ear(-1)]
bpy.ops.object.select_all(action="DESELECT")
for o in ears:
    o.select_set(True)
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.join()

# UVs (the app maps its fur texture with them)
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.02)
bpy.ops.object.mode_set(mode="OBJECT")

fur = bpy.data.materials.new("Fur")
fur.use_nodes = True
bsdf = fur.node_tree.nodes["Principled BSDF"]
vc = fur.node_tree.nodes.new("ShaderNodeVertexColor")
vc.layer_name = "Col"
fur.node_tree.links.new(vc.outputs["Color"], bsdf.inputs["Base Color"])
bsdf.inputs["Roughness"].default_value = 0.9
bsdf.inputs["Sheen Weight"].default_value = 1.0
bsdf.inputs["Subsurface Weight"].default_value = 0.15
body.data.materials.clear()
body.data.materials.append(fur)

# ---------------------------------------------------------------- eyes: small, dark, glossy, on the side of the head
eye_mat = bpy.data.materials.new("Eye")
eye_mat.use_nodes = True
eb = eye_mat.node_tree.nodes["Principled BSDF"]
eb.inputs["Base Color"].default_value = (0.05, 0.006, 0.012, 1)
eb.inputs["Roughness"].default_value = 0.04
eb.inputs["Coat Weight"].default_value = 1.0
eb.inputs["Coat Roughness"].default_value = 0.0
parts = []
for s in (1, -1):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=20, radius=0.029, location=(0.398, 0.089 * s, 0.466))
    e = bpy.context.view_layer.objects.active
    e.scale = (1.0, 0.75, 1.0)  # a rabbit's eye bulges sideways, set a little into the socket
    e.rotation_euler = (0, 0, math.radians(30 * s))
    e.data.materials.append(eye_mat)
    bpy.ops.object.shade_smooth()
    parts.append(e)
bpy.ops.object.select_all(action="DESELECT")
for e in parts:
    e.select_set(True)
bpy.context.view_layer.objects.active = parts[0]
bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
bpy.ops.object.join()
eye_ob = bpy.context.view_layer.objects.active
eye_ob.name = "Eyes"

# ---------------------------------------------------------------- armature
arm_data = bpy.data.armatures.new("RabbitRig")
arm = bpy.data.objects.new("RabbitRig", arm_data)
scene.collection.objects.link(arm)
select_only(arm)
bpy.ops.object.mode_set(mode="EDIT")
ebs = arm_data.edit_bones


def bone(name, head, tail, parent=None, up=(0, 0, 1)):
    b = ebs.new(name)
    b.head = Vector(head)
    b.tail = Vector(tail)
    b.align_roll(Vector(up))
    if parent:
        b.parent = ebs[parent]
    return b


def at(k, dz=0.0):
    x, y, z = P[k][0]
    return (x, y, z + dz)


bone("root", at("rump"), at("mid"))
bone("spine", at("mid"), at("chest"), "root")
bone("neck", at("chest"), at("neck"), "spine")
bone("head", at("neck"), at("muzzle"), "neck")
bone("tail", at("rump", 0.04), at("tail"), "root")
for n in "LR":
    bone(f"ear1.{n}", at(f"earbase.{n}"), at(f"earmid.{n}"), "head", up=(1, 0, 0))
    bone(f"ear2.{n}", at(f"earmid.{n}"), at(f"eartip.{n}"), f"ear1.{n}", up=(1, 0, 0))
    bone(f"thigh.{n}", at(f"hip.{n}"), at(f"knee.{n}"), "root", up=(1, 0, 0))
    bone(f"shin.{n}", at(f"knee.{n}"), at(f"hock.{n}"), f"thigh.{n}", up=(1, 0, 0))
    bone(f"foot.{n}", at(f"hock.{n}"), at(f"toe.{n}"), f"shin.{n}", up=(0, 0, 1))
    bone(f"arm.{n}", at(f"shoulder.{n}"), at(f"elbow.{n}"), "spine", up=(1, 0, 0))
    bone(f"fore.{n}", at(f"elbow.{n}"), at(f"paw.{n}"), f"arm.{n}", up=(1, 0, 0))
bpy.ops.object.mode_set(mode="OBJECT")

# skin the body to the rig
bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
arm.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.object.parent_set(type="ARMATURE_AUTO")
# the eyes ride on the head bone (parent_set keeps them where they are)
bpy.ops.object.select_all(action="DESELECT")
eye_ob.select_set(True)
arm.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.object.mode_set(mode="POSE")
arm_data.bones.active = arm_data.bones["head"]
bpy.ops.object.parent_set(type="BONE", keep_transform=True)
bpy.ops.object.mode_set(mode="OBJECT")

# ---------------------------------------------------------------- animations
for pbn in arm.pose.bones:
    pbn.rotation_mode = "XYZ"


def key(frame, p):
    for bn in arm.pose.bones:
        bn.rotation_euler = Euler([math.radians(a) for a in p.get(bn.name, (0, 0, 0))], "XYZ")
        bn.keyframe_insert("rotation_euler", frame=frame, group=bn.name)
        if bn.name == "root":
            bn.location = Vector(p.get("loc", (0, 0, 0)))
            bn.keyframe_insert("location", frame=frame, group=bn.name)


def pose(root=(0, 0, 0), loc=(0, 0, 0), spine=0, neck=(0, 0, 0), head=(0, 0, 0), thigh=0, shin=0, foot=0, arm_=0, fore=0, tail=0, a1=0, a2=0, spread=0):
    p = {"root": root, "loc": loc, "spine": (spine, 0, 0), "neck": neck if isinstance(neck, tuple) else (neck, 0, 0), "head": head if isinstance(head, tuple) else (head, 0, 0), "tail": (tail, 0, 0)}
    for s, n in ((1, "L"), (-1, "R")):
        p[f"thigh.{n}"] = (thigh, 0, 0)
        p[f"shin.{n}"] = (shin, 0, 0)
        p[f"foot.{n}"] = (foot, 0, 0)
        p[f"arm.{n}"] = (arm_, 0, 0)
        p[f"fore.{n}"] = (fore, 0, 0)
        p[f"ear1.{n}"] = (a1, 0, spread * s)
        p[f"ear2.{n}"] = (a2, 0, 0)
    return p


arm.animation_data_create()


def action(name, frames):
    act = bpy.data.actions.new(name)
    act.use_fake_user = True
    arm.animation_data.action = act
    for f, p in frames:
        key(f, p)
    return act


# Root bone axes: Y along the spine (forward), Z up. loc is in those axes, so (0, 0, h) lifts.
REST = pose()
CROUCH = pose(loc=(0, 0, -0.03), spine=6, thigh=-18, shin=14, foot=4, arm_=-6, fore=6, a1=4)
LAUNCH = pose(root=(8, 0, 0), loc=(0, 0, 0.05), spine=-6, thigh=48, shin=-30, foot=-24, arm_=-38, fore=-16, a1=14, a2=8, tail=10)
AIR = pose(loc=(0, 0, 0.17), spine=-8, thigh=36, shin=-20, foot=-30, arm_=-44, fore=-10, a1=30, a2=18, tail=12)
FRONT = pose(root=(-12, 0, 0), loc=(0, 0, 0.06), spine=4, thigh=10, foot=-10, arm_=-6, a1=10, a2=26, tail=4)
HIND = pose(root=(-4, 0, 0), loc=(0, 0, -0.02), spine=12, thigh=-30, shin=20, foot=8, arm_=4, fore=4, a1=-4, a2=-8)
action("Hop", [(0, CROUCH), (4, LAUNCH), (8, AIR), (12, FRONT), (16, HIND), (20, CROUCH)])
action("Idle", [(0, REST), (20, pose(loc=(0, 0, 0.004), head=-2)), (40, REST), (46, pose(head=-4, a1=-6, spread=4)), (52, REST), (70, pose(loc=(0, 0, 0.004), head=2)), (90, REST)])
action("Look", [(0, REST), (12, pose(head=(-4, 0, 32), a1=4)), (34, pose(head=(-6, 0, 34), a1=8, spread=6)), (48, pose(head=(0, 0, -24))), (64, REST)])
UP = dict(root=(36, 0, 0), loc=(0, 0, -0.01), spine=10, neck=-24, head=-16, thigh=-52, shin=22, foot=-6, arm_=40, fore=50, a1=-10, a2=-6, spread=6)
action("SitUp", [(0, REST), (10, pose(**UP)), (40, pose(**{**UP, "root": (38, 0, 0), "spread": 8})), (54, REST)])
arm.animation_data.action = bpy.data.actions["Idle"]

# ---------------------------------------------------------------- export
bpy.ops.object.select_all(action="SELECT")
bpy.ops.export_scene.gltf(
    filepath=OUT,
    export_format="GLB",
    use_selection=True,
    export_yup=True,
    export_skins=True,
    export_animations=True,
    export_animation_mode="ACTIONS",
    export_vertex_color="ACTIVE",
    export_materials="EXPORT",
)
print("exported", OUT)

# ---------------------------------------------------------------- preview stills of each pose
if PREVIEW:
    world = bpy.data.worlds.new("World")
    scene.world = world
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.45, 0.47, 0.5, 1)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.45
    bpy.ops.mesh.primitive_plane_add(size=8)
    floor = bpy.context.view_layer.objects.active
    fm = bpy.data.materials.new("Floor")
    fm.use_nodes = True
    fm.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.3, 0.32, 0.35, 1)
    floor.data.materials.append(fm)
    for nm, loc, rot, en in (("Key", (1.6, -1.8, 2.2), (45, 0, 40), 200), ("Rim", (-1.8, 1.4, 1.5), (60, 0, -135), 160)):
        L = bpy.data.lights.new(nm, "AREA")
        L.energy = en
        L.size = 2.0
        lo = bpy.data.objects.new(nm, L)
        lo.location = loc
        lo.rotation_euler = [math.radians(a) for a in rot]
        scene.collection.objects.link(lo)
    cam = bpy.data.cameras.new("Cam")
    cam.lens = 50
    co = bpy.data.objects.new("Cam", cam)
    co.location = (1.25, -1.7, 0.62)
    scene.collection.objects.link(co)
    co.rotation_euler = (Vector((0.02, 0, 0.38)) - co.location).to_track_quat("-Z", "Y").to_euler()
    scene.camera = co
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 24
    scene.cycles.device = "CPU"
    scene.render.resolution_x = 560
    scene.render.resolution_y = 440
    for act_name, frame in (("Idle", 0), ("Hop", 4), ("Hop", 8), ("Hop", 16), ("SitUp", 30), ("Look", 20)):
        arm.animation_data.action = bpy.data.actions[act_name]
        scene.frame_set(frame)
        scene.render.filepath = f"{PREVIEW}-{act_name}-{frame}.png"
        bpy.ops.render.render(write_still=True)
        print("rendered", scene.render.filepath)
