#!/usr/bin/env python3
"""Generate scenes/gold-gallery.json — bright gallery room, path-traced.

Cornell-style: warm white room, crimson left wall, deep-blue right wall,
mirror back wall (one clean infinity doubling), rectangular ceiling area
light built from two emissive triangles, gold teapot centerpiece with a
chrome sphere and diffuse companions. Bright diffuse room + big area light
= fast convergence, soft shadows, strong color bleed.

Teapot (obj/teapot.obj, rotx=90): x[5.5,9.5] y[-2.7,3.5] z[-2.49,0.71],
body center (7.5, 0.39). Floor at z=-2.5.
"""
import json

TEA_CX, TEA_CY = 7.5, 0.39
FLOOR_Z = -2.5
CEIL_Z = 5.0
X_BACK = 12.5          # mirror wall behind teapot
X_FRONT = -6.5         # behind camera
Y_LEFT = 7.5           # +y crimson
Y_RIGHT = -6.7         # -y blue

scene = {}
scene["resolution"] = [1200, 750]
scene["camera"] = {
    "pos":     {"x": -3.4, "y": -1.7, "z": 0.55},
    "look_at": {"x": TEA_CX, "y": TEA_CY, "z": -0.5},
    "up":      {"x": 0, "y": 0, "z": 1},
    "vfov": 46.0,
}

def rgb(r, g, b):
    return {"r": r, "g": g, "b": b}

ZERO = rgb(0.0, 0.0, 0.0)

materials = []
def add_mat(kd=None, ks=None, ke=None):
    materials.append({"kd": kd or ZERO, "ks": ks or ZERO, "ke": ke or ZERO, "shininess": 0})
    return len(materials) - 1

MAT_TEAPOT = add_mat(ks=rgb(0.97, 0.74, 0.32))    # 0: polished gold (meshes use material.0)
MAT_WHITE  = add_mat(kd=rgb(0.78, 0.76, 0.72))    # warm white walls/ceiling
MAT_FLOOR  = add_mat(kd=rgb(0.62, 0.60, 0.57))    # warm gray floor
MAT_RED    = add_mat(kd=rgb(0.62, 0.07, 0.09))    # crimson wall
MAT_BLUE   = add_mat(kd=rgb(0.06, 0.24, 0.55))    # deep blue wall
MAT_MIRROR = add_mat(ks=rgb(0.85, 0.86, 0.88))    # back mirror wall
MAT_LIGHT  = add_mat(ke=rgb(15.0, 13.2, 10.8))    # warm ceiling panel
MAT_CHROME = add_mat(ks=rgb(0.90, 0.91, 0.93))
MAT_IVORY  = add_mat(kd=rgb(0.90, 0.86, 0.76))
MAT_CORAL  = add_mat(kd=rgb(0.95, 0.33, 0.22))
MAT_TEAL   = add_mat(kd=rgb(0.05, 0.55, 0.52))

planes = [
    ({"x": 0, "y": 0, "z": FLOOR_Z}, {"x": 0, "y": 0, "z": 1}, MAT_FLOOR),
    ({"x": 0, "y": 0, "z": CEIL_Z},  {"x": 0, "y": 0, "z": -1}, MAT_WHITE),
    ({"x": X_BACK, "y": 0, "z": 0},  {"x": -1, "y": 0, "z": 0}, MAT_MIRROR),
    ({"x": X_FRONT, "y": 0, "z": 0}, {"x": 1, "y": 0, "z": 0}, MAT_WHITE),
    ({"x": 0, "y": Y_LEFT, "z": 0},  {"x": 0, "y": -1, "z": 0}, MAT_RED),
    ({"x": 0, "y": Y_RIGHT, "z": 0}, {"x": 0, "y": 1, "z": 0}, MAT_BLUE),
]

# ceiling light panel: 2 triangles, 6.5 x 5, slightly camera-side of teapot
LX0, LX1 = TEA_CX - 2.4, TEA_CX + 3.6
LY0, LY1 = TEA_CY - 2.2, TEA_CY + 2.2
LZ = CEIL_Z - 0.05
p00 = {"x": LX0, "y": LY0, "z": LZ}
p10 = {"x": LX1, "y": LY0, "z": LZ}
p11 = {"x": LX1, "y": LY1, "z": LZ}
p01 = {"x": LX0, "y": LY1, "z": LZ}
# Winding matters, and it LOSES LIGHT -- it is not merely a noise issue.
# The panel's geometric normal (edge1 x edge2) must point DOWN into the
# room. The original winding ([p00,p10,p11] / [p00,p11,p01]) gave normal
# z = +26.40, straight up into the ceiling, which cut BOTH paths to it:
#   - NEE: direct_light gates on cos_l = light_normal . (-wi) > 0, so every
#     sample toward the panel was rejected.
#   - BSDF: a diffuse continuation ray that lands on the panel returns zero,
#     because emission is suppressed for anything registered as an NEE light
#     (the anti-double-counting rule in trace_ray_path).
# So NO diffuse surface ever received direct light from this scene's only
# lamp. The room looked plausible anyway only because a *specular* bounce
# passes count_emission=true -- i.e. it was lit entirely by light routed
# through the mirror wall / chrome sphere / gold teapot. Fixing the winding
# measured +73% mean brightness and -44% noise at equal spp.
# Verified by computing the normal, not by eye.
triangles = [
    ([p00, p11, p10], MAT_LIGHT),
    ([p00, p01, p11], MAT_LIGHT),
]

spheres = []
def add_sphere(x, y, z, r, mat):
    spheres.append({
        "center": {"x": round(x, 4), "y": round(y, 4), "z": round(z, 4)},
        "radius": round(r, 4),
        "material_id": mat,
    })

# companions on the floor (rest on z=-2.5)
add_sphere(TEA_CX + 1.4, TEA_CY + 4.4, FLOOR_Z + 1.15, 1.15, MAT_CHROME)   # chrome, back-left
add_sphere(TEA_CX + 2.2, TEA_CY - 3.6, FLOOR_Z + 0.85, 0.85, MAT_TEAL)     # teal, back-right
add_sphere(TEA_CX - 3.6, TEA_CY - 2.7, FLOOR_Z + 0.6, 0.6, MAT_CORAL)      # coral, front-right
add_sphere(TEA_CX - 4.2, TEA_CY + 2.3, FLOOR_Z + 0.5, 0.5, MAT_IVORY)      # ivory, front-left
add_sphere(TEA_CX - 3.0, TEA_CY - 3.9, FLOOR_Z + 0.35, 0.35, MAT_WHITE)    # small white pebble

for i, m in enumerate(materials):
    scene[f"material.{i}"] = m
for i, (pt, n, mat) in enumerate(planes):
    scene[f"plane.{i}"] = {"point": pt, "normal": n, "material_id": mat}
for i, (pts, mat) in enumerate(triangles):
    scene[f"triangle.{i}"] = {"points": pts, "material_id": mat}
for i, s in enumerate(spheres):
    scene[f"sphere.{i}"] = s
scene["obj.0.path"] = "obj/teapot.obj"
scene["obj.0.rotx"] = 90

out = "/Users/maxime/git/rayflex/scenes/gold-gallery.json"
with open(out, "w") as f:
    json.dump(scene, f, indent=2)
print(f"wrote {out}: {len(materials)} materials, {len(spheres)} spheres, {len(triangles)} triangles")
