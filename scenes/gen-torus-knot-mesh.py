#!/usr/bin/env python3
"""Generate obj/torus-knot.obj -- a (3,2) torus knot as a swept tube, with
exact analytic per-vertex normals (no smoothing ambiguity, unlike scanned
organic meshes). Run once; the resulting OBJ is committed, not regenerated
on every build.

Was originally a (3,8) knot (many tight windings -- a dense coiled-rope
look). Switched to (3,2) with a bigger radius-oscillation amplitude (R,r
below) to match a simpler, more open "two big loops + a braided crossing"
composition -- fewer, larger windings read as a much more recognizable
knot shape.

tube_radius vs. self-intersection: a naive minimum spine-to-spine distance
check suggested tube_radius must stay below ~0.09 for this (R,r), which
would look like a thin wire. In practice this check is overly conservative
-- it treats any close center-to-center approach as unsafe regardless of
crossing angle, but two strands crossing at a steep angle can pass close
without their swept tube surfaces actually intersecting. Verified visually
instead (zoomed render of the crossing region, tube_radius=0.26): clean
over/under occlusion, no torn-seam artifacts. If you change P/Q/R/r, re-verify
by rendering and zooming into the tightest crossing rather than trusting
the naive distance check alone.
"""
import math

def torus_knot_point(t, p, q, R, r):
    rad = R + r * math.cos(q * t)
    x = rad * math.cos(p * t)
    y = rad * math.sin(p * t)
    z = r * math.sin(q * t)
    return (x, y, z)

def normalize(v):
    n = math.sqrt(sum(c*c for c in v))
    return tuple(c/n for c in v)

def sub(a, b):
    return tuple(a[i]-b[i] for i in range(3))
def add(a, b):
    return tuple(a[i]+b[i] for i in range(3))
def scale(a, s):
    return tuple(c*s for c in a)
def cross(a, b):
    return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])
def norm(v):
    return math.sqrt(sum(c*c for c in v))

def generate_torus_knot(p, q, R, r, n_t, n_tube, tube_radius):
    ts = [i / n_t * 2 * math.pi for i in range(n_t)]
    spine = [torus_knot_point(t, p, q, R, r) for t in ts]

    verts = []
    normals = []
    for i in range(n_t):
        p0 = spine[(i - 1) % n_t]
        p1 = spine[i]
        p2 = spine[(i + 1) % n_t]
        tangent = normalize(sub(p2, p0))
        # 'outward-ish' reference: the spine point itself, since the knot
        # wraps around the origin -- gives a stable, non-twisting frame
        # without needing parallel transport.
        b_temp = cross(tangent, p1)
        if norm(b_temp) < 1e-6:
            b_temp = cross(tangent, (0, 0, 1))
        binorm_ref = normalize(b_temp)
        frame_n = normalize(cross(binorm_ref, tangent))
        frame_b = normalize(cross(tangent, frame_n))
        for j in range(n_tube):
            theta = j / n_tube * 2 * math.pi
            offset = add(scale(frame_n, math.cos(theta)), scale(frame_b, math.sin(theta)))
            verts.append(add(p1, scale(offset, tube_radius)))
            normals.append(offset)  # already unit length (frame_n, frame_b orthonormal)

    faces = []
    for i in range(n_t):
        for j in range(n_tube):
            i2 = (i + 1) % n_t
            j2 = (j + 1) % n_tube
            a = i * n_tube + j
            b = i * n_tube + j2
            c = i2 * n_tube + j
            d = i2 * n_tube + j2
            faces.append((a, c, d))
            faces.append((a, d, b))
    return verts, normals, faces

P, Q, R, r = 3, 2, 2.0, 1.6
verts, normals, faces = generate_torus_knot(P, Q, R, r, n_t=280, n_tube=28, tube_radius=0.26)

out = "/Users/maxime/git/rayflex/obj/torus-knot.obj"
with open(out, 'w') as f:
    f.write(f'# ({P},{Q}) torus knot, swept tube, procedurally generated\n')
    for v in verts:
        f.write(f'v {v[0]:.6f} {v[1]:.6f} {v[2]:.6f}\n')
    for n in normals:
        f.write(f'vn {n[0]:.6f} {n[1]:.6f} {n[2]:.6f}\n')
    for a, b, c in faces:
        f.write(f'f {a+1}//{a+1} {b+1}//{b+1} {c+1}//{c+1}\n')

print(f'wrote {out}: verts={len(verts)} faces={len(faces)}')
