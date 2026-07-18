#!/usr/bin/env python3
"""Generate scenes/toybox.json -- a diffuse-texture showcase.

Three of Keenan Crane's hand-painted CC0 characters -- Spot (cow), Bob
(polka-dot ring), Blub (blowfish) -- from the CMU 3D model repository, each
with its own map_Kd texture (obj/{spot,bob,blub}_texture.png). Chosen over a
single busy/dark model (the backpack) because their bold flat cartoon
textures on simple rounded shapes make UV mapping read instantly -- these
are the canonical texture-mapping demo models. See AGENTS.md's Textures
section for the pipeline (map_Kd load -> UV barycentric interp -> sRGB
sampling), and Meshes for per-triangle materials (each mesh keeps its own
texture via its own one-material .mtl).

Each ships without an .mtl, so obj/{name}.mtl was authored (Kd white,
map_Kd, NO Ks -- a nonzero ks would turn the whole surface into a mirror and
hide the texture, see AGENTS.md) and mtllib/usemtl injected into the OBJ.

Path-traced STUDIO, not a lit box: a soft ceiling area light in a neutral
light-grey enclosure gives even, shadow-soft lighting and clean GI colour
bleed between the toys. The models are pure diffuse, so they converge fast
and completely dodge the mirror/dark-room trap (see suzanne-bust.py). No
checkered floor -- the procedural checker aliases into moire at grazing
angles (that was the backpack asset's floor artefact); a plain diffuse
floor avoids it entirely.

Placed bboxes (after each model's rotx, verified empirically):
  spot rotx=90: x[-0.47,0.47]  y[-1.05,0.67]  z[-0.74,0.95]  (standing cow)
  bob  rotx=0 : x[-0.91,0.94]  y[-0.51,0.46]  z[-0.73,0.73]  (ring on its side)
  blub rotx=90: x[-0.71,0.71]  y[-1.00,1.91]  z[-0.67,1.08]  (fish, side-on)
z-min per model + SCALE sets each translate.z so it rests on the floor.
"""
import json

FLOOR_Z = 0.0
CEIL_Z = 6.0
X0, X1 = -7.5, 7.5
Y0, Y1 = -7.5, 7.5

scene = {}
scene["resolution"] = [960, 720]
# look_at / vfov were solved (not eyeballed) to centre the three toys' screen
# bounding box and leave an even border: all vertices were projected through
# this exact camera, look_at iterated until the bbox centre hit screen (0,0),
# then vfov set so the tighter axis keeps an ~8% margin. Result: L/R = 8.6%,
# T/B = 8.0% margins -- each toy roughly equidistant from its nearest edge.
scene["camera"] = {
    "pos":     {"x": -6.5, "y": -5.6, "z": 3.1},
    "look_at": {"x": 0.198, "y": -0.203, "z": 0.705},
    "up":      {"x": 0, "y": 0, "z": 1},
    "vfov": 38.55,
}

def rgb(r, g, b):
    return {"r": r, "g": g, "b": b}

ZERO = rgb(0.0, 0.0, 0.0)

materials = []
def add_mat(kd=None, ks=None, ke=None):
    materials.append({"kd": kd or ZERO, "ks": ks or ZERO, "ke": ke or ZERO, "shininess": 0})
    return len(materials) - 1

# material.0 is the fallback for any mesh triangle without a usemtl; here it
# doubles as the floor (all model triangles DO have usemtl -> their own
# texture, so they never fall back to it).
MAT_FLOOR = add_mat(kd=rgb(0.42, 0.41, 0.45))   # mid-grey floor (was near-white -> washed out)
MAT_WALL  = add_mat(kd=rgb(0.50, 0.50, 0.54))   # mid-grey walls/ceiling
MAT_LIGHT = add_mat(ke=rgb(7.5, 7.1, 6.5))      # warm-neutral soft ceiling light (dimmed)

planes = [
    ({"x": 0, "y": 0, "z": FLOOR_Z}, {"x": 0, "y": 0, "z": 1}, MAT_FLOOR),
    ({"x": 0, "y": 0, "z": CEIL_Z},  {"x": 0, "y": 0, "z": -1}, MAT_WALL),
    ({"x": X0, "y": 0, "z": 0}, {"x": 1, "y": 0, "z": 0}, MAT_WALL),
    ({"x": X1, "y": 0, "z": 0}, {"x": -1, "y": 0, "z": 0}, MAT_WALL),
    ({"x": 0, "y": Y0, "z": 0}, {"x": 0, "y": 1, "z": 0}, MAT_WALL),
    ({"x": 0, "y": Y1, "z": 0}, {"x": 0, "y": -1, "z": 0}, MAT_WALL),
]

# big soft ceiling light panel (2 triangles). Winding is now irrelevant
# (NEE is two-sided, see AGENTS.md) but kept facing DOWN for intent.
LX0, LX1 = -3.0, 3.0
LY0, LY1 = -3.0, 3.0
LZ = CEIL_Z - 0.05
p00 = {"x": LX0, "y": LY0, "z": LZ}
p10 = {"x": LX1, "y": LY0, "z": LZ}
p11 = {"x": LX1, "y": LY1, "z": LZ}
p01 = {"x": LX0, "y": LY1, "z": LZ}
triangles = [
    ([p00, p11, p10], MAT_LIGHT),
    ([p00, p01, p11], MAT_LIGHT),
]

for i, m in enumerate(materials):
    scene[f"material.{i}"] = m
for i, (pt, n, mat) in enumerate(planes):
    scene[f"plane.{i}"] = {"point": pt, "normal": n, "material_id": mat}
for i, (pts, mat) in enumerate(triangles):
    scene[f"triangle.{i}"] = {"points": pts, "material_id": mat}

# The floor-rest height is computed from each model's actual geometry (its
# min z after rotx, times scale) rather than hardcoded -- hardcoding it made
# a model float or sink the moment its rotx changed. rotz is about z, so it
# does not affect z-min.
import math

def rotx_zmin(name, rx_deg):
    r = math.radians(rx_deg)
    c, s = math.cos(r), math.sin(r)
    zmin = None
    with open(f"/Users/maxime/git/rayflex/obj/{name}.obj") as f:
        for ln in f:
            if ln.startswith("v "):
                _, y, z = (float(v) for v in ln.split()[1:4])
                zr = y * s + z * c
                zmin = zr if zmin is None else min(zmin, zr)
    return zmin

# name, rotx, rotz, scale, floor (x, y). All models are Y-up natively, so
# rotx=90 stands each upright (cow on legs, duck sitting, fish on its belly).
# rotz aims each toy's face toward the viewer / inward. Spot is the hero:
# centre and largest; Bob (screen-left) and Blub (screen-right) flank it,
# well clear of the cow's silhouette and pulled in from the frame edges.
# Camera sits at (-x,-y) looking toward (+x,+y), so screen-right is the
# (+x,-y) direction: to keep the fish BESIDE the cow (not occluded behind
# it) it must sit forward (smaller y), not "back-right" (+y), which tucks it
# behind the centre.
models = [
    ("spot", 90, 90, 1.9, ( 0.3,  0.6)),   # cow, hero, centre, facing camera
    ("bob",  90, 90, 1.35, (-2.9, -1.2)),  # duck, front-left, facing camera
    ("blub", 90, -88, 1.35, ( 0.75, -2.25)),  # fish, right-forward to balance the duck, angled toward camera
]
for i, (name, rx, rz, sc, (px, py)) in enumerate(models):
    zmin = rotx_zmin(name, rx)
    scene[f"obj.{i}.path"] = f"obj/{name}.obj"
    scene[f"obj.{i}.rotx"] = rx
    scene[f"obj.{i}.rotz"] = rz
    scene[f"obj.{i}.scale"] = sc
    # rotz is about z, so it doesn't change z-min; rest the model on the floor.
    scene[f"obj.{i}.translate"] = {
        "x": px,
        "y": py,
        "z": FLOOR_Z - zmin * sc + 0.01,
    }

out = "/Users/maxime/git/rayflex/scenes/toybox.json"
with open(out, "w") as f:
    json.dump(scene, f, indent=2)
print(f"wrote {out}: {len(materials)} materials, {len(triangles)} light tris, {len(models)} models")
