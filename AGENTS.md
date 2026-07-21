# Rayflex — Ray/Path Tracer

## Quick Reference

```bash
cargo build --release
./target/release/rayflex -l scenes/<scene>.json -x 900 -y 600 --img-file out.png --reflection-max-depth 10
```

CLI flags: `-l` scene file, `-x`/`-y` resolution, `--img-file` output, `--reflection-max-depth N`, `--seed N` deterministic path tracing (reproducible renders), `-g` gamma correction, `-a` adaptive sampling, `-p 0` disable path tracing, `-p N` set N samples/pixel for path tracing, `-u` open UI.

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
| `material.N` | `kd` (diffuse RGB), `ks` (specular RGB), `ke` (emissive RGB), `kt` (transmission RGB), `ior` (index of refraction), `shininess`. Meshes can also get a `map_Kd` diffuse texture, but only from a `.mtl` file (JSON materials have no way to reference an image) — see Meshes → Textures |
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
- **Dielectric** (`kt` ≠ 0): transparent/refractive (glass, water). Snell's law + Fresnel (Schlick approximation). In PT mode the bounce is a probabilistic reflect-or-refract choice (estimator weights cancel, so plain tints are unbiased); in Whitted mode a deterministic Fresnel-weighted blend of both rays. `kt` tints transmitted light, `ks` tints the Fresnel reflection. `ior` defaults to 1.0 (air); use 1.33 for water, 1.5 for glass. Total internal reflection handled (falls back to a pure reflection bounce). Helpers on `Material`: `is_emitter`/`is_dielectric`/`is_mirror`/`is_diffuse`, with precedence **emitter > dielectric > mirror > diffuse** when several fields are nonzero. Working example: `scenes/glass-ball.json`. v1 limitations: **no Beer-Lambert absorption** (`kt` tints once per refraction event regardless of thickness), **no dispersion** (one IOR shared by RGB), **no nested dielectrics** (no medium stack on the ray — a glass sphere in air is fine, a glass sphere underwater is not). Caustics emerge naturally in PT mode (light → glass → diffuse floor) but converge slowly — the NEE shadow ray treats the glass as an opaque occluder, so the floor under the glass is in shadow and the caustic forms only via brute-force diffuse→dielectric→emitter paths. Expect `-p 1000+` for a clean caustic (400x400 at `-p 400` already shows a clear ring).
- **Mirror** (`ks` ≠ 0): perfect specular reflection tinted by `ks`; `kd` ignored. Tinted mirrors work great: gold `ks=(0.95,0.72,0.30)`, silver `(0.88,0.88,0.90)`, copper `(0.92,0.55,0.38)`, chrome `(0.86,0.88,0.91)`.
- **Diffuse** (`ks` = 0): scattered bounce weighted by `kd`.

Other path-tracing facts:

- **Rays that miss everything return black** — there is no sky. Enclose the scene, or add a giant emissive "sky dome" sphere (e.g. radius 50–90 centered on the scene, `ke=(0.03,0.045,0.085)` for faint night-blue ambient). A dome works as both backdrop *and* uniform ambient fill: sphere intersection takes the far root from inside, and NEE orients a sphere light's normal toward the receiver so an enclosing dome lights correctly (a diffuse surface fully enclosed by the dome converges to `kd * le`). Domes were silently broken between the NEE and the 2026-07 fix — see Known Bugs.
- `checkered` is **ignored** in path-tracing mode (only applies to the ray tracer).
- **Next-event estimation (NEE) is on**: every emissive **sphere** and **standalone `triangle.N`** is importance-sampled as a light (each diffuse bounce casts a shadow ray toward a random light), so direct illumination converges fast even for small/dim lights. Emissive **planes** and **meshes** are NOT NEE-sampled (planes are infinite-area; meshes shade as material.0) — they still glow if seen directly or in a mirror, but light the scene only via slow brute-force bounces, so build area lights from spheres/triangles. Caveat: NEE denoises direct light only; diffuse→mirror→light paths (mirror walls, chrome objects) still throw sparse fireflies and want more samples.
- ~~**A `triangle.N` light's winding must face the room**~~ — NO LONGER TRUE (fixed 2026-07). **Emitter winding is now irrelevant**: `NeeLight::sample` orients a triangle light's normal toward the receiver, making triangle emitters two-sided in NEE — matching the BSDF path, which always treated them that way (`trace_ray_path` returns `ke` for whichever face is hit, no facing check). Verified: a deliberately backwards-wound panel now renders within 0.01% of a correct one (was +73% apart). Historical note, since it explains the old scene files: when NEE was one-sided, a backwards panel lost its direct light *entirely* (NEE rejected every sample via `cos_l <= 0`, **and** the diffuse continuation ray returned zero because emission is suppressed for NEE-registered lights). Only *specular* bounces still carried its light, so mirror-rich rooms looked plausible while diffuse-only rooms went near-black. `gold-gallery`, `suzanne-bust` and `torus-knot` all shipped with that bug (same triangle-construction snippet); all are now wound correctly anyway.
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

- **Per-triangle materials** — a mesh shades each triangle with its own material (`Mesh::get_material_id(sub_id)` → `triangles[sub_id].material_id`). An OBJ with a `.mtl` renders multi-material; the `.mtl` materials are appended to the material list after the JSON `material.N` ones. Triangles whose `.mtl` failed to load (missing file) or that have no `usemtl` fall back to `material.0`, so single-material meshes still just need `material.0` defined. NOTE: `.mtl` import forces `ke=0` (emissive not imported) and only `map_Kd` is imported (see Textures below) — `map_Ks`/`map_Bump`/etc. and `Ke` lines are still dropped. The `obj.N.material` key is still not read by the loader.
- **Textures (`map_Kd` only)** — a `.mtl`'s `map_Kd` (diffuse/albedo texture) is decoded once at load (`Material::map_kd: Option<Arc<Texture>>`), sRGB→linear-converted up front (the renderer shades in linear light; sampling raw 8-bit values without this washes out/wrong-contrasts the albedo), and sampled at each triangle's interpolated UV (`Triangle::uvs`, loaded the same `single_index` way as `vn`/smooth normals) in place of `kd` wherever `kd` would otherwise be used — `Material::albedo(uv)` is the single access point (falls back to flat `kd` when there's no texture, so untextured materials are unaffected). Path tracer and Whitted ray tracer both resolve it (`trace_ray_path`'s diffuse branch; `trace_ray` swaps in a cheap owned `Material` clone only when `map_kd.is_some()`, via `Cow`, to avoid cloning every hit). UV `v` is flipped on sample (OBJ convention is bottom-left origin; image rows go top-down) and wrapped with `rem_euclid` for tiling. `map_Ks`/`map_Bump`/normal/roughness maps are NOT read — only diffuse albedo. **A nonzero `Ks` on a textured material can hide the texture entirely**: in path-tracing mode a material is diffuse *or* mirror, never both, so `ks != 0` makes the whole surface a pure mirror and `kd`/the texture are never even sampled; in ray-tracing mode it's `(1-ks)` diluted by a mirror blend of whatever the reflection ray sees (a plain sky washes it out). Real downloaded `.mtl`s very often carry a nonzero default `Ks` from the exporter (verified: a Blender-exported model `.mtl` shipped `Ks 0.5 0.5 0.5`, which reduced its diffuse texture to a near-flat grey/white mirror-ish blob; zeroing it revealed the full texture) — zero it out if the texture should actually be visible.
- **Smooth (interpolated) normals** — when an OBJ has `vn`, each triangle stores its three vertex normals and `get_normal` interpolates them (barycentric weights of the hit point, renormalized) for Phong-style smooth shading, instead of the flat per-face normal. Loaded via `single_index: true` in tobj's `LoadOptions`, which unifies the position/normal/texcoord index streams so `mesh.normals[i]` lines up with `mesh.positions[i]`. Meshes with no `vn` (trolley, cow, teddy — check with `grep -c "^vn " obj/foo.obj`) fall back to flat shading exactly as before, so this is purely additive. `teapot.obj` and `buddha.obj` both have `vn` and render visibly smoother as a result. Normals are rotated with the mesh (no translate; no scale — uniform scale doesn't change a direction, and the result is renormalized anyway).
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

## UI (egui) Notes

- **The window icon is eframe's built-in default egui logo, not ours.** `egui_main`'s `ViewportBuilder` never calls `.with_icon(...)`, and the repo ships no icon files (an old PWA icon set was deleted in `77042fd`). eframe 0.35 docs (`epi.rs`): "If you don't set an icon, a default egui icon will be used." To change it: `ViewportBuilder::default().with_icon(<embedded PNG>)`; to suppress it entirely, pass `egui::IconData::default()`.

## Known Bugs (verified in code, 2026-07)

- ~~**Mesh AABB drops wall-covering triangles → rectangular sky-holes**~~ — FIXED 2026-07. `AABB::triangle_inside` (aabb.rs) assigned a triangle to an octree cell only if a *vertex* was inside the box or one of its *edges* intersected it (the code even carried an `XXX: not correct` comment). A triangle whose face covers a cell without touching it that way was silently missing from the leaf, so rays through the cell hit nothing and rendered as clean-edged rectangular sky-holes (found on sponza's lion-head wall: two black rectangles flanking the medallion from inside; wall solid from outside). Replaced with the exact Akenine-Moller 13-axis SAT triangle-box overlap test (`tri_box_overlap`), which is complete for convex shapes — no touching triangle is ever dropped. Verified on the lion zoom: 6033 near-black pixels before, 244 after (remainder are genuine dark crevices). Expect a small render slowdown on meshed scenes (leaves now correctly hold more triangles): sponza 480x300 p32 went 20.5s → 24.6s.

- ~~**Checkered floors render as 1D stripes, not a checkerboard**~~ — FIXED 2026-07. `Plane::get_texture_2d` (three_d.rs) hardcoded the plane's texture frame to the world (ŷ, ẑ) axes, so on a horizontal (z-normal) plane `v·ẑ == 0` everywhere, the second UV coordinate was constant, and `do_checker`'s XOR collapsed to a single-axis test → infinite stripes along x, plus a phase seam across y=0 from the `+0.125` negative-coordinate hack. Now builds an orthonormal tangent frame from the plane's normal (helper = world axis least aligned with n) and `do_checker` wraps UVs with `rem_euclid(1.0)` instead of `fract()` (which maps all negatives into (−1,0], never passing the `> 0.5` test — the reason the hack existed). Note buddha.json only ever looked right because its "floor" plane is x-normal (the scene uses `camera.up=(1,0,0)`), where the old frame accidentally worked. Any high-frequency checkered pattern still aliases into Moiré bands at grazing angles — that's ordinary 1-sample/pixel aliasing, not this bug.

- ~~**An enclosing emissive sphere (sky dome) contributes zero light under NEE**~~ — FIXED 2026-07. `NeeLight::sample` now takes the shading point and orients a sphere light's sampled normal toward it (flipping to the inward side when the receiver is *inside* the sphere). Was: sampling always produced an *outward* normal, so for a dome — where every receiver is inside — `direct_light`'s `cos_l <= 0` test rejected every sample, while the diffuse continuation ray's emission was simultaneously suppressed *because* the dome is an NEE light (anti-double-counting). Both paths cut ⇒ a dome-only scene rendered pure black even at `ke=0.5`. Verified after the fix against theory: a diffuse sphere (`kd=0.6`) enclosed by a dome (`le=0.5`) converges to 0.294 vs the analytic `kd*le = 0.30`. No-op for ordinary lights (receiver outside ⇒ normal unchanged; the inward-facing half is self-occluded anyway).
- ~~**Rotation matrices apply transposed**~~ — FIXED 2026-07. `Vec3::multiply` (vec3.rs) indexed `mat[i + j*3]` (the transpose of the matrix as written), so `rotx/roty/rotz` rotated by −angle vs. their standard CCW definitions. Fixed to `mat[i*3 + j]`. Every scene rotation angle was negated in the same change (cow, teapot, trolley: `rotx -90 → 90`; buddha: `rotz 90 → -90`) to keep every render pixel-identical — see `git log -- src/vec3.rs` for that commit.
- ~~**`Vec3::gen_rnd_sphere` is not uniform**~~ — FIXED 2026-07. Now samples components in `[-1,1]` so the `n > 1` rejection actually fires, giving directions uniform on the unit sphere (was: cube `[-0.5,0.5]³` normalized, biased toward cube diagonals).
- ~~**Path-tracer diffuse is not Lambertian**~~ — FIXED 2026-07. `trace_ray_path` now scatters cosine-weighted around the surface normal (`hit_normal + gen_rnd_sphere`, with a degenerate-direction guard) instead of around the mirror-reflection direction. Diffuse shading is now view-independent.
- ~~**Per-triangle mesh materials are ignored**~~ — FIXED 2026-07. `Mesh::get_material_id(sub_id)` now returns the hit triangle's material; `load_mesh` range-checks tobj's per-face ids against the count of successfully-loaded `.mtl` materials (missing-mtl meshes fall back to `material.0` instead of indexing out of bounds). The `obj.N.material` key is still not read.
- **Scene loader silently drops keys after a numbering gap** — `material.N`/`sphere.N`/… loading stops at the first missing index with no warning.
- ~~**`report_progress` divides by zero** for renders smaller than 128 total pixels (`denom / 128 == 0`)~~ — FIXED 2026-07. Now `(denom / 128).max(1)`, so tiny smoke renders (64×64, 8×8) no longer panic.
- Fixed 2026-07: the UI reset `path_level` to 1 every frame while the path-tracing checkbox was off, so re-enabling it silently rendered 1 sample/pixel.
- ~~**`-n` (generate) overwrites the `-l` scene file**~~ — FIXED 2026-07 by removing the feature: the `-n`/`--num-spheres-to-generate` and `-b`/`--add-box` flags and the `generate_scene` function are gone (it was a random-sphere scene generator that, sharing `-l`'s path, silently replaced a hand-tuned scene file). This also retired the dead `num_planes` key it used to write.
- ~~**`cargo test` clobbers `pic.png`**~~ — FIXED 2026-07. Every test in `tests/cli.rs` now passes `--img-file /tmp/rayflex-test-<scene>.png`, so running the suite no longer overwrites the user's working render, and parallel test threads no longer race on the same file.
- **`material.shininess` is parsed but never used** (verified 2026-07). It flows from JSON into the `Material` struct and then nowhere — `grep -rn shininess src/` shows no shading use. Spot-light specular hardcodes the exponent: `light_vec_norm.dot(dir).powi(80)` (`light.rs:56`), so scenes setting `shininess: 40` vs `200` render identically. `VectorLight` ignores it too and uses a hardcoded quartic falloff `powi(4)` (`light.rs:127`) instead of a Lambertian cos term.
- **UI render-thread panic leaves the app stuck on "Stop"** (verified 2026-07). `start_rendering` runs on the spawned thread and does `load_scene(cfg).unwrap()` (`app.rs:70-71`) and `job.save_image().expect("output file")` (`app.rs:88`). A typo in the scene-file box or an unwritable output path panics the thread; `rendering_active` is only reset on the success path (`app.rs:90`), so the Start/Stop button stays "Stop" until the app is restarted. Should catch the error, surface it in the UI, and reset the flag.
- ~~**UI shows black bands after switching macOS Spaces mid-render**~~ — FIXED 2026-07. The render thread *pushed* texture updates (`texture_handle.set` + `ctx.request_repaint` in `update_func`). When the window is occluded (another Space / minimized), the repaint runs no frames, so the final full-image upload never reached the GPU — the texture kept its last pre-occlusion state, with tiles unrendered at that point stuck as black bands even after the render completed and the window became visible again. Fixed with a pull model: the render thread now only hands the live image buffer to the UI (via a `render_buffer` slot filled after `alloc_image`) and updates progress; `ui()` uploads the buffer itself whenever `pct` advanced since the last upload (`last_uploaded_pct`), so the first frame after the window becomes visible again always refreshes from the complete buffer. Same upload cadence as before (one upload per progress tick, ~128/render).
- ~~**Progress bar shows "done" before the render finishes**~~ — FIXED 2026-07. `render_image_box` reported `step * step` pixels per tile regardless of actual tile size, so when the resolution isn't a multiple of `step` (32 classic / 10 PT) the accumulated total overshot `res_x * res_y` and pct hit 1.0 early (672x416 @ step=10: 285600 reported vs 279552 actual → "done" ~60 tiles early; up to +56% on tiny images). Now reports each tile's true clamped pixel count, so 100% fires exactly when the last tile's pixels are in the buffer. Side benefit: also closes a hole in the pull-model texture upload (pct reaching 1.0 early meant the UI never uploaded again, so the genuinely final pixels could stay off the texture).
- ~~**UI width/height sliders snap typed values to odd numbers (600 → 608)**~~ — FIXED 2026-07. The sliders used `.step_by(64.0)` over `32..=2048`, and egui anchors the step grid at the range minimum, so the only legal values were `32 + k*64` = 32, 96, …, 608, 672 — typing 600 snapped to 608, and the scene presets that set 600x400 were silently off-grid too. Dropped `step_by`; any integer size is accepted (edge tiles clamp, so arbitrary resolutions render fine).

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
- ~~Smooth (interpolated vertex) normals for meshes~~ — DONE 2026-07 (see Meshes section). With per-triangle materials + transforms + smooth normals all in place, an existing single- or multi-material OBJ with `vn` now loads, places, and shades reasonably out of the box.
- ~~OBJ translation + scale~~ — DONE 2026-07 (`obj.N.scale`, `obj.N.translate`; see Meshes section).
- ~~Per-triangle materials at shading time~~ — DONE 2026-07 (see Known Bugs).

Workflow/UI:
- ~~**CLI progress visibility on long renders**~~ — DONE 2026-07 (main.rs). The CLI progress callback now (a) flushes the in-progress image to `--img-file` every 60s (`FLUSH_INTERVAL` const) so long renders can be eyeballed mid-way (valid PNG with unrendered boxes black), (b) prints a `progress: NN% -- Xs elapsed` stdout line every 10% (gated to renders >5s to keep fast renders/test output quiet) for nohup/piped runs where indicatif hides itself, and (c) styles the indicatif bar with percent/elapsed/ETA (`{percent}% {wide_bar} [{elapsed_precise}] [{eta_precise}]`) for interactive use. Verified on Sponza at 960x600 `-p 100`: flush fired at ~60s mid-render, decile logs streamed.
- Expose `reflection_max_depth` in the UI (hardcoded to 5 there; mirror-heavy path-traced scenes need 8+).
- Progressive preview: accumulate samples and refresh the texture, instead of one fixed-sample pass.
- Discover the scene dropdown from `scenes/*.json` instead of a hardcoded list in app.rs.
- Loader: strict serde structs with arrays instead of numbered keys; warn on unknown/gap keys.
- Environment map (HDRI) background for path tracing instead of returning black on miss.

## Prompt for Augmenting This File

If you discover something not covered here (new object types, material properties, lighting models, export formats, performance tuning flags, test commands, common pitfalls), add a new section to this file. Keep sections short and example-driven. Prefer concrete values and commands over abstract descriptions. If a section grows beyond ~30 lines, split it into a subsection.
