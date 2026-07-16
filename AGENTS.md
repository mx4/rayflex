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
- **Next-event estimation** is on: at each diffuse bounce a shadow ray is sampled toward a random emitter (`sphere.N`/`triangle.N` with `ke != 0`), so direct light is low-noise even for smallish lights. Emissive **planes** and **meshes** are NOT NEE-sampled (only spheres/triangles) — they still illuminate via brute-force paths but stay noisy; prefer sphere/triangle emitters.
- **Firefly clamp + tone mapping** (both path-tracing only): each path sample's radiance is clamped to `FIREFLY_CLAMP` (render.rs, 6.0) before averaging to trim rare bright speckle; final pixels pass through a highlight-rolloff tone-map (`image.rs`, identity below knee 0.75, soft roll to white above) so emitters/highlights don't hard-clip. Ray-traced (non-PT) scenes are unaffected — tone-map is gated on `path_tracing > 1`. If a scene needs emitters brighter than ~6 to read correctly in *indirect* bounces, raise `FIREFLY_CLAMP`; residual mirror-path speckle is expected (needs MIS to fully fix).

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
- **Next-event estimation (NEE) is on**: every emissive **sphere** and **standalone `triangle.N`** is importance-sampled as a light (each diffuse bounce casts a shadow ray toward a random light), so direct illumination converges fast even for small/dim lights. Emissive **planes** and **meshes** are NOT NEE-sampled (planes are infinite-area; meshes shade as material.0) — they still glow if seen directly or in a mirror, but light the scene only via slow brute-force bounces, so build area lights from spheres/triangles. Caveat: NEE denoises direct light only; diffuse→mirror→light paths (mirror walls, chrome objects) still throw sparse fireflies and want more samples.
- A directly visible emitter with any `ke` channel > 1 clamps to white after gamma — the orb's color shows in its floor glow / reflections, not the orb itself. For a visibly colored emitter keep `ke` ≲ 1.
- Behind-camera trick: a large dim warm sphere (e.g. `ke=(1.1,0.85,0.6)`, r=4) behind the camera gives chrome objects a front sheen without appearing in frame.

### What Actually Looks Good in Path-Tracing Mode

Lessons from composed-scene attempts (what failed and what worked):

- **Open night scenes are less muddy now that NEE is on** — direct light from small emitters resolves cleanly. They're still the hardest case (little bounce light, and any mirrors throw fireflies), so a bright enclosed room is still the safer bet, but "objects on a plane lit by a few small emissive spheres" is now viable at moderate sample counts.
- **Bright enclosed rooms look great.** Cornell-style: closed box, light walls (`kd≈0.75`), one or two saturated accent walls, a large ceiling area light. Lots of bounce light → fast convergence, soft shadows, strong color bleed onto metallic objects. This is the renderer's sweet spot.
- **Build rectangular area lights from 2 emissive triangles** just below the ceiling (planes are infinite — an emissive plane would be the whole ceiling). A ~6x4 panel with `ke≈(15,13,11)` lights a 13-unit room well. Offset the panel behind the hero object so a soft contact shadow falls toward the camera.
- **One mirror wall** (`ks≈0.85`) behind the scene adds a "second room" doubling without chaos. Fully mirrored rooms (infinity-mirror look) turn into unreadable dot-soup unless lights are very sparse and wall `ks` is low (≤0.55) so recursion fades to black — hard to make look good.
- **A gold-mirror hero object** (`ks=(0.97,0.74,0.32)`) in a room with colored walls picks up gorgeous multi-colored reflections. Plain chrome in a dark scene just reflects darkness and reads as a black blob.
- Working example: `scenes/gold-gallery.json` (generated by a Python script; render with `-p 2500 -g --reflection-max-depth 8`).

## Meshes (OBJ)

- **Per-triangle materials** — a mesh shades each triangle with its own material (`Mesh::get_material_id(sub_id)` → `triangles[sub_id].material_id`). An OBJ with a `.mtl` renders multi-material; the `.mtl` materials are appended to the material list after the JSON `material.N` ones. Triangles whose `.mtl` failed to load (missing file) or that have no `usemtl` fall back to `material.0`, so single-material meshes still just need `material.0` defined. NOTE: `.mtl` import forces `ke=0` (emissive not imported) and there's no texture support, so `map_*`/`Ke` lines are dropped. The `obj.N.material` key is still not read by the loader.
- **Transforms** — `obj.N.rotx/roty/rotz` (degrees), `obj.N.scale` (uniform scalar, default 1.0), `obj.N.translate` (`{x,y,z}`, default 0). Applied per vertex as `p' = R(scale · p) + translate` (SRT: scale about origin → rotate about origin → translate into place). Lets you size and position a mesh regardless of its native coordinates, and load the same OBJ multiple times (`obj.0`, `obj.1`, …) at different transforms.
- `obj/teapot.obj` with `rotx=90` (upright, z-up): bbox x[5.51, 9.50], y[-2.71, 3.49], z[-2.49, 0.71]; body center ≈ (7.5, 0.39); spout on the +y side. Put the floor at z=-2.5. 6.3k triangles — fast even in path tracing (hierarchical AABB).
- **Rotation sign follows the standard right-hand/CCW convention** (fixed 2026-07, see Known Bugs): `rotz = t` rotates counter-clockwise viewed from +z, i.e. (x,y) → (x·cos t − y·sin t, x·sin t + y·cos t). Always verify orientation with a cheap render (`-p 32`, 480x300 — sub-second) when placing a new mesh.
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

- ~~**Rotation matrices apply transposed**~~ — FIXED 2026-07. `Vec3::multiply` (vec3.rs) indexed `mat[i + j*3]` (the transpose of the matrix as written), so `rotx/roty/rotz` rotated by −angle vs. their standard CCW definitions. Fixed to `mat[i*3 + j]`. Every scene rotation angle was negated in the same change (cow, teapot, trolley: `rotx -90 → 90`; buddha: `rotz 90 → -90`) to keep every render pixel-identical — see `git log -- src/vec3.rs` for that commit.
- ~~**`Vec3::gen_rnd_sphere` is not uniform**~~ — FIXED 2026-07. Now samples components in `[-1,1]` so the `n > 1` rejection actually fires, giving directions uniform on the unit sphere (was: cube `[-0.5,0.5]³` normalized, biased toward cube diagonals).
- ~~**Path-tracer diffuse is not Lambertian**~~ — FIXED 2026-07. `trace_ray_path` now scatters cosine-weighted around the surface normal (`hit_normal + gen_rnd_sphere`, with a degenerate-direction guard) instead of around the mirror-reflection direction. Diffuse shading is now view-independent.
- ~~**Per-triangle mesh materials are ignored**~~ — FIXED 2026-07. `Mesh::get_material_id(sub_id)` now returns the hit triangle's material; `load_mesh` range-checks tobj's per-face ids against the count of successfully-loaded `.mtl` materials (missing-mtl meshes fall back to `material.0` instead of indexing out of bounds). The `obj.N.material` key is still not read.
- **Scene loader silently drops keys after a numbering gap** — `material.N`/`sphere.N`/… loading stops at the first missing index with no warning.
- **`report_progress` divides by zero** for renders smaller than 128 total pixels (`denom / 128 == 0`).
- **`generate_scene` writes a `num_planes` key** that nothing reads.
- Fixed 2026-07: the UI reset `path_level` to 1 every frame while the path-tracing checkbox was off, so re-enabling it silently rendered 1 sample/pixel.

## Improvement Backlog

Renderer quality (highest visual payoff first):
1. ~~**Next-event estimation**~~ — DONE 2026-07. `trace_ray_path` importance-samples emissive spheres/triangles per diffuse bounce (`direct_light` + `NeeLight` in render.rs; light set built in scene.rs). Direct lighting converges ~10x faster.
2. **Multiple importance sampling (MIS)** — the natural follow-up to NEE. Kills the diffuse→mirror→light fireflies that NEE alone leaves, and handles small bright lights + glossy surfaces robustly. Also: sphere lights currently use uniform-area sampling (half the samples face away) — cone sampling toward the visible cap would cut their variance.
3. ~~**Tone mapping**~~ + ~~firefly clamp~~ — DONE 2026-07. Highlight-rolloff tone-map (image.rs, PT-only) + per-sample `FIREFLY_CLAMP` (render.rs). Follow-up still open: **exposure control** (a scene/CLI multiplier before tone-map) so brightness isn't purely emitter-driven.
4. **Dielectrics/refraction** (glass spheres) — big showcase win; the material model currently has no transmission.
5. Mixed materials: probabilistic kd/ks choice plus a roughness parameter (glossy, not just perfect mirror).
6. Russian-roulette path termination instead of the hard depth cap.

Geometry/performance:
- Top-level BVH over scene objects — `find_closest_hit` linearly scans every object; many-sphere scenes pay per ray.
- Smooth (interpolated vertex) normals for meshes — the teapot renders visibly faceted.
- ~~OBJ translation + scale~~ — DONE 2026-07 (`obj.N.scale`, `obj.N.translate`; see Meshes section). Remaining blocker to *nicely* rendering an existing OBJ model: smooth vertex normals (meshes render faceted).
- ~~Per-triangle materials at shading time~~ — DONE 2026-07 (see Known Bugs).

Workflow/UI:
- Expose `reflection_max_depth` in the UI (hardcoded to 5 there; mirror-heavy path-traced scenes need 8+).
- Progressive preview: accumulate samples and refresh the texture, instead of one fixed-sample pass.
- Discover the scene dropdown from `scenes/*.json` instead of a hardcoded list in app.rs.
- Loader: strict serde structs with arrays instead of numbered keys; warn on unknown/gap keys.
- Environment map (HDRI) background for path tracing instead of returning black on miss.

## Prompt for Augmenting This File

If you discover something not covered here (new object types, material properties, lighting models, export formats, performance tuning flags, test commands, common pitfalls), add a new section to this file. Keep sections short and example-driven. Prefer concrete values and commands over abstract descriptions. If a section grows beyond ~30 lines, split it into a subsection.
