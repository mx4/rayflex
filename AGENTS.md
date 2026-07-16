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

## Testing & Validation

When adjusting camera or scene parameters, always:
1. Render to a temp file: `--img-file /tmp/test.png`
2. Verify render logs show expected camera direction vectors
3. Check `max_h`/`max_v` angles computed from bounding box projection stay within FOV limits
4. If changing centering, recompute NDC bbox center after every look_at adjustment

## Prompt for Augmenting This File

If you discover something not covered here (new object types, material properties, lighting models, export formats, performance tuning flags, test commands, common pitfalls), add a new section to this file. Keep sections short and example-driven. Prefer concrete values and commands over abstract descriptions. If a section grows beyond ~30 lines, split it into a subsection.
