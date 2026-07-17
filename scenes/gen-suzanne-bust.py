#!/usr/bin/env python3
"""Generate scenes/suzanne-bust.json -- a polished bronze Suzanne bust in a
dark gallery, path-traced. Mirror material deliberately chosen: specular
highlights are far more sensitive to normal smoothness than diffuse shading,
so this is also the strongest visual showcase of smooth vertex normals.

obj/suzanne.obj native bbox: x[-1.367,1.367] y[-0.984,0.984] z[-0.852,0.852].
With rotx=90, rotz=270 (front-facing, verified empirically):
  world X in [-0.852, 0.852]   (nose axis)
  world Y in [-1.367, 1.367]   (ear to ear)
  world Z in [-0.984, 0.984]   (chin to crown)
"""
import json

SCALE = 1.35
HALF_Z = 0.984 * SCALE  # chin-to-crown half-extent after scale
FLOOR_Z = 0.0
TRANSLATE_Z = FLOOR_Z + HALF_Z + 0.01  # neck stump rests just above the floor

CEIL_Z = 5.5
X0, X1 = -7.5, 7.5
Y0, Y1 = -7.5, 7.5

scene = {}
scene["resolution"] = [960, 960]
scene["camera"] = {
    "pos":     {"x": -6.2, "y": -3.9, "z": TRANSLATE_Z + 0.65},
    "look_at": {"x": 0.0, "y": 0.0, "z": TRANSLATE_Z + 0.05},
    "up":      {"x": 0, "y": 0, "z": 1},
    "vfov": 34.0,
}

def rgb(r, g, b):
    return {"r": r, "g": g, "b": b}

ZERO = rgb(0.0, 0.0, 0.0)

materials = []
def add_mat(kd=None, ks=None, ke=None):
    materials.append({"kd": kd or ZERO, "ks": ks or ZERO, "ke": ke or ZERO, "shininess": 0})
    return len(materials) - 1

# Only the bust is a mirror. Mirrors get no NEE (specular is a delta -- a
# reflected ray must geometrically hit an emitter to pick up any light), so
# the room around it must be diffuse with real albedo for rays that bounce
# off the bust to reliably terminate on a lit surface (same recipe as
# gold-gallery's diffuse floor/walls around its mirror teapot). An
# all-mirror room (bust + floor) with near-black walls was tried first and
# rendered pure black: ~100% of paths exhausted reflection_max_depth
# without ever finding light.
MAT_BRONZE = add_mat(ks=rgb(0.80, 0.50, 0.28))   # 0: Suzanne (meshes use material.0)
MAT_WALL   = add_mat(kd=rgb(0.24, 0.22, 0.26))   # dark diffuse gallery walls
MAT_FLOOR  = add_mat(kd=rgb(0.28, 0.255, 0.28))  # dark diffuse stone floor
MAT_LIGHT  = add_mat(ke=rgb(26.0, 21.0, 16.0))   # warm soft key light
MAT_RIM    = add_mat(ke=rgb(2.0, 2.8, 4.0))      # small cool rim accent, behind the bust

planes = [
    ({"x": 0, "y": 0, "z": FLOOR_Z}, {"x": 0, "y": 0, "z": 1}, MAT_FLOOR),
    ({"x": 0, "y": 0, "z": CEIL_Z},  {"x": 0, "y": 0, "z": -1}, MAT_WALL),
    ({"x": X0, "y": 0, "z": 0}, {"x": 1, "y": 0, "z": 0}, MAT_WALL),
    ({"x": X1, "y": 0, "z": 0}, {"x": -1, "y": 0, "z": 0}, MAT_WALL),
    ({"x": 0, "y": Y0, "z": 0}, {"x": 0, "y": 1, "z": 0}, MAT_WALL),
    ({"x": 0, "y": Y1, "z": 0}, {"x": 0, "y": -1, "z": 0}, MAT_WALL),
]

# soft key light panel above-front of the bust (2 triangles)
LX0, LX1 = -1.6, 0.9
LY0, LY1 = -2.0, -0.2
LZ = CEIL_Z - 0.05
p00 = {"x": LX0, "y": LY0, "z": LZ}
p10 = {"x": LX1, "y": LY0, "z": LZ}
p11 = {"x": LX1, "y": LY1, "z": LZ}
p01 = {"x": LX0, "y": LY1, "z": LZ}
triangles = [
    ([p00, p10, p11], MAT_LIGHT),
    ([p00, p11, p01], MAT_LIGHT),
]

spheres = []
def add_sphere(x, y, z, r, mat):
    spheres.append({"center": {"x": x, "y": y, "z": z}, "radius": r, "material_id": mat})

# small cool rim light on the far side of the bust from the camera (camera
# is at -X,-Y; this sits at +X,+Y beyond the origin), so it's mostly
# occluded by the head and only grazes the silhouette edge as a rim glow.
add_sphere(2.4, 1.6, TRANSLATE_Z + 0.25, 0.55, MAT_RIM)

for i, m in enumerate(materials):
    scene[f"material.{i}"] = m
for i, (pt, n, mat) in enumerate(planes):
    scene[f"plane.{i}"] = {"point": pt, "normal": n, "material_id": mat}
for i, (pts, mat) in enumerate(triangles):
    scene[f"triangle.{i}"] = {"points": pts, "material_id": mat}
for i, s in enumerate(spheres):
    scene[f"sphere.{i}"] = s
scene["obj.0.path"] = "obj/suzanne.obj"
scene["obj.0.rotx"] = 90
scene["obj.0.rotz"] = 270
scene["obj.0.scale"] = SCALE
scene["obj.0.translate"] = {"x": 0.0, "y": 0.0, "z": TRANSLATE_Z}

out = "/Users/maxime/git/rayflex/scenes/suzanne-bust.json"
with open(out, "w") as f:
    json.dump(scene, f, indent=2)
print(f"wrote {out}: {len(materials)} materials, {len(spheres)} spheres, {len(triangles)} triangles")
