#!/usr/bin/env python3
"""Generate scenes/torus-knot.json -- a floating diffuse (3,2) torus knot
above a dark floor, path-traced, lit by two lights (upper-left, upper-right
from the camera) for cross-shadowing. The knot is a swept tube (see
scenes/gen-torus-knot-mesh.py; the mesh itself is generated once and
committed as an OBJ, not regenerated here) with exact per-vertex normals --
every point on a tube has a mathematically well-defined outward radial
normal, so this is an unambiguous, clean showcase of smooth shading (unlike
organic scans where "how much smoothing" is a modeling choice).

Was originally a (3,8) knot (dense coiled-rope look); switched to (3,2)
with a bigger R/r ratio for a simpler, more recognizable "two big open
loops + a braided crossing" composition. Camera angle was found by an
azimuth/elevation sweep around the knot (az=120 deg) picked as the best
match to a reference "torus knot" image -- not an exact replica (this knot
reads a bit more diagonal than the reference's cleaner side-by-side
loops), but the same recognizable two-loop-plus-crossing gestalt.

No walls: this ray/path tracer has no sky -- a ray that hits nothing
returns pure black (see AGENTS.md). A closed room exists only to keep that
void from showing around the subject; dropping it entirely gives a clean
floating-in-black-void look instead, which suits a knot (no natural "room"
context) better than a lit box. Only the floor stays, so there's still a
grounding shadow. Two emissive SPHERES (NEE-compatible -- see AGENTS.md
Path Tracing notes) stand in for the walls' fill light, positioned via the
camera's own right/up basis so "upper-left/upper-right" means relative to
what the camera actually sees, not arbitrary world coordinates.

obj/torus-knot.obj bbox: x in [-3.86,3.86], y in [-3.18,3.18], z in
[-1.86,1.86] (flat, wide knot -- the tube's spine lies mostly in the XY
plane). Bounding radius (corner-to-center) ~4.7; camera distance must clear
half_extent*margin/tan(vfov/2) or the knot crops at the frame edges (first
pass at dist=11 with vfov=32 was too tight -- required ~16-17 for this
bounding radius and margin).
"""
import json
import math

FLOOR_Z = 0.0
KNOT_Z = 2.4  # floating height of the knot's center above the floor

VFOV = 32.0
_az = math.radians(120.0)
_elev = 8.0  # higher = looking down more onto the knot
_dist = 17.0  # see bbox note above: must clear ~16-17 to avoid cropping
_cam_x = _dist * math.cos(_az)
_cam_y = _dist * math.sin(_az)
_cam_z = KNOT_Z + _elev
_look_at = (0.0, 0.0, KNOT_Z)

scene = {}
scene["resolution"] = [960, 960]
scene["camera"] = {
    "pos":     {"x": _cam_x, "y": _cam_y, "z": _cam_z},
    "look_at": {"x": _look_at[0], "y": _look_at[1], "z": _look_at[2]},
    "up":      {"x": 0, "y": 0, "z": 1},
    "vfov": VFOV,
}

def rgb(r, g, b):
    return {"r": r, "g": g, "b": b}

def norm3(v):
    n = math.sqrt(sum(c * c for c in v))
    return tuple(c / n for c in v)

def cross3(a, b):
    return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])

def add3(a, b):
    return tuple(a[i] + b[i] for i in range(3))

def scale3(a, s):
    return tuple(c * s for c in a)

ZERO = rgb(0.0, 0.0, 0.0)

materials = []
def add_mat(kd=None, ks=None, ke=None):
    materials.append({"kd": kd or ZERO, "ks": ks or ZERO, "ke": ke or ZERO, "shininess": 0})
    return len(materials) - 1

# Diffuse jewel-tone knot, not mirror: a mirror this size/distance in a dark
# scene only catches thin specular streaks off small lights (tried first on
# an earlier scene, came out nearly black -- see gems-and-pearls lesson). A
# broad Lambertian gradient reads the tube's shape -- and its smooth
# normals -- far more clearly, and doesn't fight for light the way an
# all-mirror object does (see suzanne-bust.py for that failure mode too).
MAT_METAL = add_mat(kd=rgb(0.14, 0.72, 0.55))    # 0: knot -- emerald (brighter albedo)
MAT_FLOOR = add_mat(kd=rgb(0.34, 0.33, 0.36))    # mid-gray floor: catches the shadow AND bounces fill
MAT_LIGHT_R = add_mat(ke=rgb(60.0, 51.0, 39.0))  # warm key, camera's upper-right
MAT_LIGHT_L = add_mat(ke=rgb(36.0, 42.0, 57.0))  # cool fill, camera's upper-left
# NO sky dome: an enclosing emissive sphere contributes *nothing* under NEE
# (verified: dome-only scene renders pure black even at ke=0.5). It gets
# registered as an NEE light, but NEE's one-sided cos_l check rejects it
# (its normals point outward, we're inside), AND the diffuse continuation
# ray suppresses its emission precisely because it IS an NEE light -- so
# both paths to it are cut. See AGENTS.md Known Bugs. A big dim emissive
# sphere placed normally (not enclosing) works fine for fill instead.

planes = [
    ({"x": 0, "y": 0, "z": FLOOR_Z}, {"x": 0, "y": 0, "z": 1}, MAT_FLOOR),
]

# Camera-relative basis, so "upper-left"/"upper-right" mean what the viewer
# actually sees, not arbitrary world-space offsets.
forward = norm3((_look_at[0]-_cam_x, _look_at[1]-_cam_y, _look_at[2]-_cam_z))
cam_right = norm3(cross3(forward, (0.0, 0.0, 1.0)))
cam_up = cross3(cam_right, forward)  # already unit: forward and cam_right are orthonormal

knot_center = (0.0, 0.0, KNOT_Z)
# UP_OFFSET must clear the top of the view frustum or the light spheres
# render as blown-out white discs in the frame's top corners (they're
# emitters seen directly -- verified at UP_OFFSET=5.5, which put them just
# below the camera). ke below is scaled up to compensate for the extra
# distance's inverse-square falloff.
R_OFFSET = 7.0    # sideways spread
UP_OFFSET = 11.0  # height above the knot -- keeps the spheres out of frame
FWD_OFFSET = 2.0  # slightly toward the camera from the knot's center

def light_pos(side):
    base = add3(knot_center, scale3(forward, -FWD_OFFSET))
    base = add3(base, scale3(cam_up, UP_OFFSET))
    return add3(base, scale3(cam_right, side * R_OFFSET))

spheres = []
def add_sphere(pos, radius, mat):
    spheres.append({"center": {"x": pos[0], "y": pos[1], "z": pos[2]}, "radius": radius, "material_id": mat})

# Bigger radius = softer shadow edges and more total flux (a sphere light's
# power scales with its area), which is most of the exposure win here.
add_sphere(light_pos(+1.0), 2.2, MAT_LIGHT_R)  # camera's upper-right
add_sphere(light_pos(-1.0), 2.2, MAT_LIGHT_L)  # camera's upper-left

for i, m in enumerate(materials):
    scene[f"material.{i}"] = m
for i, (pt, n, mat) in enumerate(planes):
    scene[f"plane.{i}"] = {"point": pt, "normal": n, "material_id": mat}
for i, s in enumerate(spheres):
    scene[f"sphere.{i}"] = s

scene["obj.0.path"] = "obj/torus-knot.obj"
scene["obj.0.translate"] = {"x": 0.0, "y": 0.0, "z": KNOT_Z}

out = "/Users/maxime/git/rayflex/scenes/torus-knot.json"
with open(out, "w") as f:
    json.dump(scene, f, indent=2)
print(f"wrote {out}: {len(materials)} materials, {len(spheres)} spheres (2 lights)")
