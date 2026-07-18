# Implementation Plan: Dielectrics / Refraction

## Goal

Add transparent, refractive materials (glass, water, clear plastic) to
rayflex. This unlocks glass spheres, water surfaces, caustics, lenses,
stained glass — the single biggest visual capability gap in the renderer
today.

The material model currently is diffuse (`kd`) / mirror (`ks`) / emissive
(`ke`), mutually exclusive in path-tracing mode. Dielectric becomes a
**fourth material type**, gated on `!kt.is_zero()`.

## Design principles

1. **Backwards compatible** — all `kt`/`ior` fields default to
   zero/one; existing scenes render identically.
2. **Two implementations**, one in each render path:
   - **`trace_ray_path`** (path tracing): probabilistic refraction via
     Snell + Fresnel, producing real **caustics** (the showcase).
   - **`trace_ray`** (Whitted): deterministic refraction + reflection
     blended by Fresnel, recursing to `reflection_max_depth`. No
     caustics, but glass spheres look right.
3. **NEE compatibility** — dielectrics are not diffuse, so NEE does not
   sample toward them; light passes *through* them and the diffuse
   surfaces on the far side get lit by the refracted ray + NEE at their
   bounce. No special handling needed.
4. **Total internal reflection (TIR)** handled — when `cos_θ_t² < 0`,
   reflect instead of refract.
5. **No dispersion** — single IOR for all wavelengths (RGB shares one
   IOR). Dispersion (different IOR per channel) is a follow-up; it
   triples the ray count and is rarely worth it.

## Files touched

| File | Change |
|---|---|
| `src/material.rs` | Add `kt`, `ior` fields; helper `is_dielectric()` |
| `src/vec3.rs` | Add `refract()` helper (Snell's law) |
| `src/render.rs` | `trace_ray_path`: dielectric branch; `trace_ray`: dielectric branch; ensure proper normal handling for rays entering/exiting |
| `src/scene.rs` | Load `kt`, `ior` from JSON into `Material` (serde defaults handle missing keys) |
| `tests/cli.rs` | New test: a glass-sphere scene |
| `scenes/glass-ball.json` | New showcase scene |
| `AGENTS.md` | Document the new material type |

## Step 1 — Material model (`src/material.rs`)

### 1.1 Add fields

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    #[serde(default)]
    pub ks: RGB,
    #[serde(default)]
    pub kd: RGB,
    #[serde(default)]
    pub ke: RGB,
    #[serde(default)]
    pub kt: RGB,        // NEW: transmission tint (Beer-Lambert later)
    #[serde(default = "default_ior")]
    pub ior: f32,        // NEW: index of refraction (1.0 = vacuum/air)
    #[serde(default)]
    pub shininess: f32,
    #[serde(default)]
    pub checkered: bool,
    #[serde(skip, default)]
    pub map_kd: Option<Arc<Texture>>,
}

fn default_ior() -> f32 { 1.0 }
```

### 1.2 Helper predicates

The path tracer branches on material type. Add helpers so the dispatch
is readable:

```rust
impl Material {
    /// Emitter: ke != 0. Path terminates, returns ke.
    pub fn is_emitter(&self) -> bool { !self.ke.is_zero() }

    /// Dielectric: kt != 0. Transmits + reflects via Fresnel.
    pub fn is_dielectric(&self) -> bool { !self.kt.is_zero() }

    /// Mirror: ks != 0 (and not dielectric). Perfect specular reflection.
    pub fn is_mirror(&self) -> bool { !self.ks.is_zero() && self.kt.is_zero() }

    /// Diffuse: ks == 0 && kt == 0. Scatters cosine-weighted.
    pub fn is_diffuse(&self) -> bool { self.ks.is_zero() && self.kt.is_zero() }
}
```

**Precedence** (when multiple are non-zero, to keep behavior
predictable): `emitter` > `dielectric` > `mirror` > `diffuse`. Document
this in AGENTS.md.

### 1.3 Beer-Lambert absorption (optional, recommended)

Real glass absorbs light as it travels through the material — that's
why a thick glass block looks greenish-blue while a thin pane is clear.
Without it, all glass looks the same color regardless of thickness.

Add a `Vec3` to track **distance traveled inside the medium** on the
ray, or approximate it: tint the refracted ray by `kt^distance` where
`distance` is the path length to the next hit. This needs the ray to
carry an "inside material" state.

**Simpler v1 (skip Beer-Lambert)**: just tint the refracted ray by
`kt` once per refraction event. Good enough for a first cut; revisit
for colored-glass scenes.

## Step 2 — Snell's law helper (`src/vec3.rs`)

```rust
impl Vec3 {
    /// Refract `incident` through a surface with normal `n` (assumed
    /// pointing toward the incident side) and ratio `eta = n_i / n_t`.
    /// Returns the refracted direction, or `None` if total internal
    /// reflection occurs. `incident` must be normalized.
    pub fn refract(incident: Vec3, n: Vec3, eta: f32) -> Option<Vec3> {
        let cos_i = -incident.dot(n);          // > 0 if n faces the incident ray
        let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
        if sin2_t > 1.0 {
            return None;                        // TIR
        }
        let cos_t = (1.0 - sin2_t).sqrt();
        Some(eta * incident + (eta * cos_i - cos_t) * n)
    }
}
```

Reference: Bram de Greve's ["Reflections and Refractions in Ray
Tracing"](https://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf),
the standard reference for this formula.

## Step 3 — Normal orientation (the tricky part)

A ray hitting a glass sphere hits the **front face** when entering
(normal points outward, against the ray) and the **back face** when
exiting (normal points inward, also against the ray *from inside*).
The two-sided shading flip in `trace_ray_path` already does
`if hit_normal.dot(ray.dir) > 0.0 { hit_normal = hit_normal * -1.0; }`
— this makes the normal always face the incoming ray. **For refraction
we need the opposite convention**: the normal must point toward the
*incident medium* (where the ray came from).

Strategy: compute the **geometric** normal (un-flipped) once, then in
the dielectric branch derive `cos_i`, `eta`, and the correct normal
orientation explicitly:

```rust
// hit_normal was already flipped to face the incoming ray by the
// two-sided shading code above. For refraction we need it to point
// toward the incident medium, which (after the flip) it does.
let n = hit_normal;                     // faces the ray
let cos_i = (-ray.dir).dot(n).max(0.0);  // > 0
let entering = cos_i > 0.0;             // always true here, but keep for clarity
let eta = if entering { 1.0 / material.ior } else { material.ior };
// n already faces incident medium; no swap needed.
```

Wait — there's a subtlety. When the ray is *inside* the glass
(traveling through it) and hits the back surface from inside, the
geometric normal points outward (away from the ray). The two-sided
flip turns it to face the ray. From the ray's perspective it's still
"entering" the interface from the glass side. The incident medium is
glass (ior 1.5), the transmitted medium is air (ior 1.0). So
`eta = ior_glass / ior_air = 1.5`, not `1/1.5`.

The robust rule: **`eta = ior_incident / ior_transmitted`**. We need
to know which side we're on. With the flipped normal facing the ray:
- If the ray came from outside (normal was outward before flip → flip
  is no-op): incident = air (1.0), transmitted = glass (ior).
  `eta = 1.0/ior`.
- If the ray came from inside (normal was inward → flip turns it
  outward, away from ray): wait, that's wrong direction.

Let me reconsider. The cleanest approach (from pbrt) is:

```rust
// Geometric normal, not flipped:
let mut n = hit_obj.get_normal(hit_point, hit_id.sub_id);
let cos_i = n.dot(ray.dir);
let entering = cos_i < 0.0;   // normal opposes ray → ray hits front face
if !entering { n = -n; }      // make n face the incident side
let eta = if entering { 1.0 / material.ior } else { material.ior };
let cos_i = cos_i.abs();
```

So: don't use the two-sided-flipped normal for dielectrics. Re-derive
from the geometric normal. `entering` is true when the ray hits the
outside of the surface (normal naturally opposes ray). When the ray is
inside the glass hitting the back, `cos_i > 0` (normal points along
ray), `entering = false`, we flip `n`, and `eta = ior/1.0`.

This is the standard handling. Keep the two-sided flip for
diffuse/mirror (they don't care about inside/outside), but bypass it
for dielectrics.

## Step 4 — Fresnel reflectance

Use the **Schlick approximation** (cheap, accurate enough for
dielectrics; the exact Fresnel equations are a follow-up):

```rust
fn schlick_r0(cos: f32, ior: f32) -> f32 {
    let r0 = ((1.0 - ior) / (1.0 + ior)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cos).powi(5)
}
```

`cos` here is `cos_i` (angle of incidence). For TIR, reflectance = 1.0.

## Step 5 — Path tracing: `trace_ray_path` dielectric branch

Insert this branch **before** the mirror/diffuse branches, gated on
`is_dielectric()`:

```rust
if hit_material.is_dielectric() {
    stats.num_rays_refraction += 1;   // new stat counter (optional)

    // Geometric normal (not the two-sided-flipped one):
    let mut n = hit_obj.get_normal(hit_point, hit_id.sub_id);
    let cos_i = n.dot(ray.dir);
    let entering = cos_i < 0.0;
    if !entering { n = -n; }
    let cos_i = cos_i.abs();
    let eta = if entering { 1.0 / hit_material.ior } else { hit_material.ior };

    let wi = -ray.dir.normalize();
    let kr = schlick_r0(cos_i, hit_material.ior);

    // Probabilistic choice: reflect with prob kr, refract with prob (1-kr).
    // Weight each by 1/prob to keep the estimator unbiased.
    // A common simplification: pick reflect if rand < kr, else refract,
    // and weight by 1/kr or 1/(1-kr). Or weight the *result* by the
    // probability (no division) — equivalent for expected value.
    let mut rnd_state = ... ; // from the per-pixel rng state
    if rnd_state.gen::<f32>() < kr {
        // Reflect.
        let reflected = ray.get_reflection(hit_point, n);
        let c_reflect = self.trace_ray_path(stats, rnd_state, &reflected, depth + 1, Some(hit_id));
        return c_reflect * hit_material.ks;   // ks tints the reflection; kt tints the refraction
    } else {
        // Refract (or TIR → reflect).
        match Vec3::refract(wi, n, eta) {
            Some(refracted_dir) => {
                let refracted = Ray::new(hit_point, refracted_dir);
                let c_refract = self.trace_ray_path(stats, rnd_state, &refracted, depth + 1, Some(hit_id));
                return c_refract * hit_material.kt;
            }
            None => {
                // Total internal reflection.
                let reflected = ray.get_reflection(hit_point, n);
                let c_reflect = self.trace_ray_path(stats, rnd_state, &reflected, depth + 1, Some(hit_id));
                return c_reflect * hit_material.ks;
            }
        }
    }
}
```

**Important**: the `exclude` parameter (currently `Some(hit_id)`) must
be passed correctly. For a sphere it excludes the whole sphere — but
for refraction we need to re-hit the *same* sphere on the inside. So
`exclude` must be `None` for the refracted ray (we want it to hit the
back of the glass). For the reflected ray, `Some(hit_id)` is fine (it
bounces off, shouldn't re-hit the same point).

Wait — the current `exclude` mechanism excludes the entire object for
single-primitive objects (`Plane::intercept` returns false if
`exclude.is_some()`). For a glass sphere, the refracted ray must
re-enter the same sphere. So: **`exclude = None` for the refracted
ray**. This means the refracted ray will immediately re-intersect the
sphere at t≈0 (the same point) — but `tmin = EPSILON` handles that
(the existing intersection code rejects t ≤ tmin). Good, no
self-intersection.

Actually there's a subtlety: with `exclude = None` and
`tmin = EPSILON`, the refracted ray inside the sphere will correctly
find the far-side intersection. ✓. The near-side (where it just
exited) is rejected by `tmin`. ✓.

For the reflected ray, `exclude = Some(hit_id)` is correct — it
bounces off the surface and shouldn't re-hit the same point.

### 5.1 Beer-Lambert (v2, optional)

To add colored-glass absorption, the refracted ray needs to know how
far it traveled inside the medium before exiting, then tint by
`exp(-sigma_t * distance)` where `sigma_t = -ln(kt)`. This requires
either:
- Tracing the refracted ray and recording the distance to the next
  hit, applying the tint *after* recursion returns (wrap the recursive
  call), or
- Carrying an "inside material" stack on the ray (more general, needed
  for nested dielectrics).

For v1, skip Beer-Lambert; tint by `kt` once per refraction (constant
color regardless of thickness). Document as a limitation.

### 5.2 Caustics note

Caustics emerge naturally: a glass sphere on a diffuse floor will
focus light into bright patterns because the refracted rays concentrate
on the floor — and at each floor hit, NEE kicks in toward the lights,
but the *direct* refracted-ray contribution is what forms the caustic.
The catch: caustics from pure path tracing converge slowly (the
diffuse floor rarely samples a direction that happens to hit the glass
then the light). The standard fix is **MIS** or **photon mapping**,
both out of scope. For v1, expect noisy caustics that clean up with
high `-p` (1000+). This is expected and documented in AGENTS.md.

### 5.3 Nested dielectrics (out of scope for v1)

A glass sphere inside a glass sphere, or water inside glass, requires
tracking which medium the ray is currently in. pbrt carries a stack of
`Medium` pointers on the ray. For v1, assume dielectrics don't nest (a
glass sphere floating in air is fine; a glass sphere underwater is
not). Document the limitation.

## Step 6 — Whitted ray tracing: `trace_ray` dielectric branch

Classic mode doesn't sample — it deterministically traces both
reflected and refracted rays and blends by Fresnel:

```rust
if hit_material.is_dielectric() {
    stats.num_rays_refraction += 1;

    let mut n = hit_obj.get_normal(hit_point, hit_id.sub_id);
    let cos_i = n.dot(ray.dir);
    let entering = cos_i < 0.0;
    if !entering { n = -n; }
    let cos_i = cos_i.abs();
    let eta = if entering { 1.0 / hit_material.ior } else { hit_material.ior };

    let wi = -ray.dir.normalize();
    let kr = schlick_r0(cos_i, hit_material.ior);

    // Reflection ray (always traced, weighted by kr).
    let reflected = ray.get_reflection(hit_point, n);
    let c_reflect = self.trace_ray(stats, &reflected, depth + 1, Some(hit_id));

    // Refraction ray (weighted by 1-kr), or TIR (kr=1, no refraction).
    let c_refract = match Vec3::refract(wi, n, eta) {
        Some(refracted_dir) => {
            let refracted = Ray::new(hit_point, refracted_dir);
            self.trace_ray(stats, &refracted, depth + 1, None)  // exclude=None: must re-hit the glass
        }
        None => RGB::zero(),
    };

    return c_reflect * kr * hit_material.ks + c_refract * (1.0 - kr) * hit_material.kt;
}
```

This gives clean glass-sphere renders in classic mode (no caustics,
but the sphere itself looks glassy — reflection + refraction blended).
`reflection_max_depth` caps recursion.

## Step 7 — Scene loading (`src/scene.rs`)

Serde handles `kt` and `ior` automatically via `#[serde(default)]` /
`#[serde(default = "default_ior")]`. No code change needed in the
loader beyond ensuring the `Material` struct has those fields. Verify
by loading a scene with `kt` set.

### Scene JSON example — `scenes/glass-ball.json`

```json
{
  "resolution": [600, 600],
  "camera": {
    "pos": {"x": -4, "y": 0, "z": 1.2},
    "look_at": {"x": 0, "y": 0, "z": 0.6},
    "up": {"x": 0, "y": 0, "z": 1},
    "vfov": 45
  },
  "material.0": { "kd": {"r": 0.7, "g": 0.7, "b": 0.7}, "checkered": true },
  "material.1": { "kd": {"r": 0.8, "g": 0.6, "b": 0.3} },
  "material.2": { "kt": {"r": 0.95, "g": 0.95, "b": 0.95}, "ior": 1.5, "ks": {"r": 1, "g": 1, "b": 1} },
  "plane.0": { "point": {"x": 0, "y": 0, "z": 0}, "normal": {"x": 0, "y": 0, "z": 1}, "material_id": 0 },
  "sphere.0": { "center": {"x": 0, "y": 0, "z": 0.6}, "radius": 0.6, "material_id": 2 },
  "sphere.1": { "center": {"x": 1.5, "y": 0.5, "z": 0.4}, "radius": 0.4, "material_id": 1 },
  "ambient": { "rgb": {"r": 1, "g": 1, "b": 1}, "intensity": 0.3 },
  "spot-light.0": { "pos": {"x": -2, "y": 0, "z": 4}, "rgb": {"r": 1, "g": 1, "b": 1}, "intensity": 20 }
}
```

The glass sphere (`material.2`) sits on a checkered floor next to an
opaque ball. Render classic mode first (`-p 1`) to verify reflection +
refraction blend, then path-traced (`-p 200 -g`) to see caustics.

## Step 8 — Tests (`tests/cli.rs`)

```rust
#[test]
fn scene_glass_ball() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("rayflex")?;
    cmd.arg("-l").arg("scenes/glass-ball.json")
       .arg("-x").arg("200").arg("-y").arg("200")
       .assert().success();
    Ok(())
}
```

Plus a manual visual check: classic render should show the checkered
floor **visible through the glass sphere** (refracted/inverted), and an
opaque ball behind the glass should appear distorted. Path-traced
render should show a **caustic** on the floor under the glass sphere.

## Step 9 — Documentation (`AGENTS.md`)

Add to the "Path-Tracing Material Semantics" section:

```
- **Dielectric** (`kt` ≠ 0): transparent/refractive. Snell's law + Fresnel
  (Schlick). Probabilistic reflect-or-refract in PT mode; deterministic
  blend in Whitted mode. TIR handled. `ior` defaults to 1.5 (glass); use
  1.33 for water, 1.0 for air. `kt` tints transmitted light; `ks` tints
  the Fresnel reflection. Precedence: emitter > dielectric > mirror >
  diffuse. v1 limitations: no Beer-Lambert (constant tint regardless of
  thickness), no dispersion (single IOR for RGB), no nested dielectrics
  (no medium stack). Caustics emerge in PT mode but converge slowly
  (expect -p 1000+ for clean results; MIS would fix this).
```

## Step 10 — Validation checklist

- [ ] Existing scenes render pixel-identical (kt defaults to zero).
- [ ] `glass-ball.json` classic render: floor visible through the
      sphere (distorted), opaque ball behind visible through glass.
- [ ] `glass-ball.json` path-traced: caustic pattern on the floor
      under the sphere (bright focused spot).
- [ ] TIR visible: a glass sphere with high ior (2.0) shows internal
      reflections at grazing angles.
- [ ] `cargo test` passes (10 → 11 tests).
- [ ] No panic on rays fully inside glass (long path through a thick slab).

## Out of scope for v1 (follow-ups)

- **Beer-Lambert absorption** — colored glass that deepens with
  thickness. Needs distance tracking.
- **Dispersion** — per-channel IOR for rainbow prism effects. Triples
  ray count.
- **Nested dielectrics** — water inside glass inside air. Needs a
  medium stack on the ray.
- **Rough dielectrics** — frosted glass. Needs microfacet BSDF
  (roughness parameter).
- **MIS for caustics** — photon mapping or bidirectional path tracing
  to converge caustics fast.

## Effort estimate

- Material fields + helpers: 15 min
- `Vec3::refract` + `schlick_r0`: 15 min
- `trace_ray_path` branch: 45 min (normal orientation is the fiddly part)
- `trace_ray` branch: 20 min
- Scene + tests + docs: 30 min
- Debugging / visual validation: 1–2 hours (refraction is notoriously
  easy to get 95% right and 5% wrong — sign errors on the normal, wrong
  `eta` direction, missing TIR, self-intersection through the glass
  surface)

**Total: ~3–4 hours** for a working v1 with one showcase scene.
