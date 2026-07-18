#!/usr/bin/env python3
"""Generate scenes/backpack.json -- a textured OBJ+MTL showcase.

obj/backpack.obj is the "Survival Guitar Backpack" (Berk Gedik), via the
LearnOpenGL tutorial repo, with its own backpack.mtl + diffuse.jpg (a real
map_Kd diffuse texture, not a hand-authored one) -- see AGENTS.md's Textures
section for the pipeline this exercises (UV load -> barycentric
interpolation -> sRGB-linearized sampling).

obj/backpack.mtl is patched from the upstream Ks 0.5 0.5 0.5 to 0 0 0: a
nonzero ks makes the WHOLE material a mirror in path-tracing mode (kd and
any texture are ignored outright -- see AGENTS.md, materials are diffuse XOR
mirror, never both) and a 50%-strength mirror blend even in ray-tracing
mode, which visibly washes the diffuse texture out against a plain sky.
Verified directly: rendering with the original Ks 0.5 produced a nearly
textureless grey/white model; zeroing it revealed the full diffuse texture.

Simple ray-traced scene (not path-traced): this is a single-mesh texture
demo, not a lighting showcase, so it follows the teapot/cow/trolley family
(spot lights + checkered floor) rather than the NEE-lit gallery family
(gold-gallery/suzanne-bust/torus-knot) -- faster to render and sidesteps
that whole family's mirror/dark-room failure mode entirely (moot here since
ks is zeroed anyway, but there's no reason to pay path-tracing's cost for a
scene with no interesting light transport).

obj/backpack.obj bbox: x[-1.92,1.82] y[-0.79,2.68] z[-1.74,2.88] in its own
frame. After obj.0.rotx=90 (Y-up -> Z-up, verified empirically -- world Z
range becomes the original Y range exactly), world Z spans [-1.74,2.88];
translate.z=1.761 puts its bottom just above the floor at z=0.
"""
import json

FLOOR_Z = 0.0
BOTTOM_Z_LOCAL = -1.741018  # backpack's world-Z-min after rotx=90, pre-translate
TRANSLATE_Z = FLOOR_Z - BOTTOM_Z_LOCAL + 0.02  # small margin above the floor

scene = {}
scene["resolution"] = [960, 960]  # matches the other 1:1 assets (see xtask/src/main.rs)
scene["camera"] = {
    "pos":     {"x": -7.4, "y": -6.3, "z": 3.3},
    "look_at": {"x": -0.05, "y": 0.95, "z": 1.7},
    "up":      {"x": 0, "y": 0, "z": 1},
    "vfov": 32.0,
}

def rgb(r, g, b):
    return {"r": r, "g": g, "b": b}

scene["material.0"] = {  # checkered floor
    "kd": rgb(0.7, 0.7, 0.72),
    "ks": rgb(0.0, 0.0, 0.0),
    "ke": rgb(0.0, 0.0, 0.0),
    "shininess": 0,
    "checkered": True,
}
scene["plane.0"] = {
    "point": {"x": 0, "y": 0, "z": FLOOR_Z},
    "normal": {"x": 0, "y": 0, "z": 1},
    "material_id": 0,
}
# Intensities tuned by rendering exactly the way xtask ships this asset
# (ray-traced scenes render with gamma OFF -- see xtask/src/main.rs's
# "match the original assets" comment), not against a -g preview: a -g test
# render looks fine at much lower intensities, then comes out dark once
# rendered for real without gamma. The backpack's dark leather/canvas
# texture also just absorbs more light than the flatter, paler kd colors
# teapot.json/cow.json use, and needs correspondingly more incident light.
scene["spot-light.0"] = {
    "intensity": 34,
    "pos": {"x": -5, "y": -5, "z": 6},
    "rgb": rgb(1.0, 1.0, 1.0),
}
scene["spot-light.1"] = {  # cool fill from the opposite side
    "intensity": 16,
    "pos": {"x": 4, "y": 3, "z": 4},
    "rgb": rgb(0.7, 0.75, 0.9),
}
scene["ambient"] = {"intensity": 0.5, "rgb": rgb(1.0, 1.0, 1.0)}

scene["obj.0.path"] = "obj/backpack.obj"
scene["obj.0.rotx"] = 90
scene["obj.0.translate"] = {"x": 0.0, "y": 0.0, "z": TRANSLATE_Z}

out = "/Users/maxime/git/rayflex/scenes/backpack.json"
with open(out, "w") as f:
    json.dump(scene, f, indent=2)
print(f"wrote {out}")
