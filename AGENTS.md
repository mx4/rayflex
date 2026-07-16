# Rayflex — Ray/Path Tracer

## Quick Reference

```bash
cargo build --release
./target/release/rayflex -l scenes/<scene>.json -x 900 -y 600 --img-file out.png --reflection-max-depth 10
```

CLI flags: `-l` scene file, `-x`/`-y` resolution, `--img-file` output, `--reflection-max-depth N`, `-g` gamma correction, `-a` adaptive sampling, `-p 0` disable path tracing, `-p N` set N samples/pixel for path tracing, `-u` open UI.

## Coordinate System

- **Z is up** (ground plane at `z=0`, normal `(0,0,1)`)
- **Y axis**: letters/objects arranged along Y (positive Y = top of word, negative Y = bottom)
- **X axis**: depth/distance from camera (camera at negative X looks toward positive X)
- Camera `up` vector is always `(0, 0, 1)`

## Scene JSON Format

Top-level keys (order doesn't matter):

| Key pattern | Description |
|---|---|
| `resolution` | `[width, height]` array |
| `camera` | `pos`, `look_at`, `up`, `vfov` |
| `material.N` | `kd` (diffuse RGB), `ks` (specular RGB), `ke` (emissive RGB), `shininess` |
| `sphere.N` | `center` `{x,y,z}`, `radius`, `material_id` |
| `plane.N` | `point` `{x,y,z}`, `normal` `{x,y,z}`, `material_id` |
| `triangle.N` | Three vertices, `material_id` |
| `obj.N.path` | OBJ mesh file path, optional `rotx`/`roty`/`rotz` in degrees |
| `spot-light.N` | `pos`, `rgb`, `intensity` |
| `vec-light.N` | `dir`, `rgb`, `intensity` |
| `ambient` | `rgb`, `intensity` |

## Letter Structure (rayflex.json)

The word "rayflex" is built from spheres. Materials 1–7 map to letters in descending Y order:

| Material | Letter | Y range (approx) |
|----------|--------|-------------------|
| 1 | R | 2.30 → 2.78 |
| 2 | A | 1.48 → 1.96 |
| 3 | Y | 0.66 → 1.14 |
| 4 | F | -0.16 → 0.32 |
| 5 | L | -0.98 → -0.50 |
| 6 | E | -1.80 → -1.32 |
| 7 | X | -2.78 → -2.14 |

Each letter spans Z from ~0.32 to ~1.28 (height). Sphere radius = 0.105. Ground plane at z=0 reflects everything.

## Camera Tips

- **Centering**: `look_at` must be at the visual center of the bounding box of all objects + reflections. For word + ground reflections, this is approximately `(0, ~0.7, ~0.2)` — not at the geometric center of the word alone.
- **To verify centering**: project all 8 bounding-box corners into NDC and check that the bbox center is near (0,0).
- **Perspective ratio**: for R to appear Nx larger than X, solve `dist_X / dist_R = N` given camera position.
- **Tilt**: `atan2(-(cam_z - look_z), sqrt(dx²+dy²))` gives downward angle in degrees.
- **FOV check**: with `vfov=60` and aspect 1.5, half-angles are h=48.7° v=30°. All object extremes must stay under these limits.

## Path Tracing Mode

Path tracing (`-p N` with N > 1) uses Monte Carlo sampling instead of direct illumination. Key differences:

- **Ignores `spot-light` and `vec-light`** — only `ke` (emissive) materials act as light sources
- **Diffuse materials**: `ks` should be zero (path tracer does its own scattering via hemisphere sampling)
- **Emissive materials**: `ke` > 0 on an object makes it glow. The color + intensity acts as the light source. Typical values: `{r: 10-20, g: 10-20, b: 10-20}` for bright area lights
- **Ambient light**: set to zero intensity (path tracing doesn't use it)
- **Higher `-p`** = more samples per pixel = less noise but slower. Start with `-p 100` for testing, use `-p 400-1000` for final renders
- **Keep reflection depth** (`--reflection-max-depth 6-8`) to cap bounce count
- **Emissive spheres** make good area lights. Place them behind the camera or outside the FOV to avoid seeing them in the frame

Example:
```bash
./target/release/rayflex -l scenes/rayflex-pt.json -x 900 -y 600 --img-file out.png -p 400 --reflection-max-depth 8
```

Speed calibration (M-series MacBook, scene with a 6.3k-triangle mesh + ~55 spheres + plane + sky dome): 480x300 `-p 32` ≈ 1 s; 600x375 `-p 256` ≈ 10 s. Iterate composition at low res/samples, save `-p 2000+` for the final frame. Always pass `-g` in path-tracing mode.

### Path-Tracing Material Semantics (trace_ray_path)

Materials are **mutually exclusive** in path-tracing mode — each surface is exactly one of:

- **Emitter** (`ke` ≠ 0): path terminates and returns `ke` directly; `kd`/`ks` ignored.
- **Mirror** (`ks` ≠ 0): perfect specular reflection tinted by `ks`; `kd` ignored. Tinted mirrors work great: gold `ks=(0.95,0.72,0.30)`, silver `(0.88,0.88,0.90)`, copper `(0.92,0.55,0.38)`, chrome `(0.86,0.88,0.91)`.
- **Diffuse** (`ks` = 0): scattered bounce weighted by `kd`.

Other path-tracing facts:

- **Rays that miss everything return black** — there is no sky. Enclose the scene, or add a giant emissive "sky dome" sphere (e.g. radius 90 centered on the scene, `ke=(0.03,0.045,0.085)` for faint night-blue ambient). Sphere intersection takes the far root from inside, so a dome works.
- `checkered` is **ignored** in path-tracing mode (only applies to the ray tracer).
- No next-event estimation (brute-force): prefer **large** area lights (radius 3–5, `ke` 10–15) over small bright ones, or the image stays noisy.
- A directly visible emitter with any `ke` channel > 1 clamps to white after gamma — the orb's color shows in its floor glow / reflections, not the orb itself. For a visibly colored emitter keep `ke` ≲ 1.
- Behind-camera trick: a large dim warm sphere (e.g. `ke=(1.1,0.85,0.6)`, r=4) behind the camera gives chrome objects a front sheen without appearing in frame.

### What Actually Looks Good in Path-Tracing Mode

Lessons from composed-scene attempts (what failed and what worked):

- **Open night scenes look muddy.** Dark diffuse floor + black sky + a few emitters = grainy, dim, gray. Without next-event estimation, diffuse surfaces in dim scenes stay noisy even at 2000+ samples. Avoid "objects on an infinite plane at night".
- **Bright enclosed rooms look great.** Cornell-style: closed box, light walls (`kd≈0.75`), one or two saturated accent walls, a large ceiling area light. Lots of bounce light → fast convergence, soft shadows, strong color bleed onto metallic objects. This is the renderer's sweet spot.
- **Build rectangular area lights from 2 emissive triangles** just below the ceiling (planes are infinite — an emissive plane would be the whole ceiling). A ~6x4 panel with `ke≈(15,13,11)` lights a 13-unit room well. Offset the panel behind the hero object so a soft contact shadow falls toward the camera.
- **One mirror wall** (`ks≈0.85`) behind the scene adds a "second room" doubling without chaos. Fully mirrored rooms (infinity-mirror look) turn into unreadable dot-soup unless lights are very sparse and wall `ks` is low (≤0.55) so recursion fades to black — hard to make look good.
- **A gold-mirror hero object** (`ks=(0.97,0.74,0.32)`) in a room with colored walls picks up gorgeous multi-colored reflections. Plain chrome in a dark scene just reflects darkness and reads as a black blob.
- Working example: `scenes/gold-gallery.json` (generated by a Python script; render with `-p 2500 -g --reflection-max-depth 8`).

## Meshes (OBJ)

- **A mesh always shades with `material.0`** — `Mesh::get_material_id()` returns the mesh-level id (hardcoded 0 in `load_mesh`). Per-triangle `.mtl` materials are loaded into the material list but ignored at shading time. The `obj.N.material` key seen in some scenes is not read by the loader. So: design the scene with material.0 = the mesh's material.
- **Rotation only** — no translation or scale for OBJs. Build the scene around the mesh's native position.
- `obj/teapot.obj` with `rotx=-90` (upright, z-up): bbox x[5.51, 9.50], y[-2.71, 3.49], z[-2.49, 0.71]; body center ≈ (7.5, 0.39); spout on the +y side. Put the floor at z=-2.5. 6.3k triangles — fast even in path tracing (hierarchical AABB).
- **rotz sign is inverted vs. the usual CCW convention** for scene placement: `rotz = t` moves points by (x,y) → (x·cos t + y·sin t, −x·sin t + y·cos t), i.e. clockwise viewed from +z. Verified empirically (teapot center (7.5, 0.39) with rotz=18 lands at ≈(7.25, −1.95)). Always verify orientation with a cheap render (`-p 32`, 480x300 — sub-second).
- Rotations apply in order rotx → roty → rotz, each about the **origin**, so rotating an off-origin mesh also moves it.

## Scene JSON Loader Quirks

- `material.N` / `sphere.N` / `plane.N` / `triangle.N` / `obj.N.*` must be numbered **contiguously from 0** — the loader stops at the first missing index and silently ignores the rest.
- Generating scene JSON from a small Python script is much easier to iterate on than hand-editing (rings of spheres, palettes, recomputing centers after rotation).

## Testing & Validation

When adjusting camera or scene parameters, always:
1. Render to a temp file: `--img-file /tmp/test.png`
2. Verify render logs show expected camera direction vectors
3. Check `max_h`/`max_v` angles computed from bounding box projection stay within FOV limits
4. If changing centering, recompute NDC bbox center after every look_at adjustment

## Known Bugs (verified in code, 2026-07)

- **Rotation matrices apply transposed** — `Vec3::multiply` (vec3.rs) indexes `mat[i + j*3]`, i.e. multiplies by the transpose of the matrix as written, so `rotx/roty/rotz` rotate by −angle vs. their standard CCW definitions. This is why `obj.N.rotz = t` turns meshes clockwise (see Meshes section). Fixing it flips every scene that uses rotations.
- **`Vec3::gen_rnd_sphere` is not uniform** — it samples a cube `[-0.5,0.5]³` and normalizes. The `n <= 1.0` rejection never fires (max cube norm ≈ 0.866), so directions are biased toward the cube diagonals. Should sample components in `[-1,1]` and reject `n > 1`.
- **Path-tracer diffuse is not Lambertian** — `trace_ray_path` scatters around the *mirror-reflection* direction (`reflect_dir + gen_rnd_sphere`, "fuzzy metal" style), not cosine-weighted around the surface normal, so diffuse shading is view-dependent and directionally biased.
- **Per-triangle mesh materials are ignored** — `load_mesh` parses `.mtl` materials into the material list and assigns per-triangle ids, but shading uses `Mesh::get_material_id()` which returns the mesh-level id, hardcoded to 0 (`Mesh::new(triangles, 0)`). The `obj.N.material` key found in older scene files is never read by the loader either.
- **Scene loader silently drops keys after a numbering gap** — `material.N`/`sphere.N`/… loading stops at the first missing index with no warning.
- **`report_progress` divides by zero** for renders smaller than 128 total pixels (`denom / 128 == 0`).
- **`generate_scene` writes a `num_planes` key** that nothing reads.
- Fixed 2026-07: the UI reset `path_level` to 1 every frame while the path-tracing checkbox was off, so re-enabling it silently rendered 1 sample/pixel.

## Improvement Backlog

Renderer quality (highest visual payoff first):
1. **Next-event estimation** (direct light sampling toward emitters) in `trace_ray_path` — the single biggest noise reduction; would make small/dim lights usable.
2. **Cosine-weighted hemisphere sampling** for diffuse (fixes the Lambertian bug above).
3. **Tone mapping** (Reinhard or ACES) + exposure control instead of hard clamp — lets bright emitters roll off instead of clipping to white.
4. **Dielectrics/refraction** (glass spheres) — big showcase win; the material model currently has no transmission.
5. Mixed materials: probabilistic kd/ks choice plus a roughness parameter (glossy, not just perfect mirror).
6. Russian-roulette path termination instead of the hard depth cap.

Geometry/performance:
- Top-level BVH over scene objects — `find_closest_hit` linearly scans every object; many-sphere scenes pay per ray.
- Smooth (interpolated vertex) normals for meshes — the teapot renders visibly faceted.
- OBJ translation + scale (rotation-only today forces scenes to be built around the mesh's native coordinates).
- Per-triangle materials at shading time (see bug above).

Workflow/UI:
- Expose `reflection_max_depth` in the UI (hardcoded to 5 there; mirror-heavy path-traced scenes need 8+).
- Progressive preview: accumulate samples and refresh the texture, instead of one fixed-sample pass.
- Discover the scene dropdown from `scenes/*.json` instead of a hardcoded list in app.rs.
- Loader: strict serde structs with arrays instead of numbered keys; warn on unknown/gap keys.
- Environment map (HDRI) background for path tracing instead of returning black on miss.

## Prompt for Augmenting This File

If you discover something not covered here (new object types, material properties, lighting models, export formats, performance tuning flags, test commands, common pitfalls), add a new section to this file. Keep sections short and example-driven. Prefer concrete values and commands over abstract descriptions. If a section grows beyond ~30 lines, split it into a subsection.
