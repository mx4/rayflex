use colored::Colorize;
use rand::Rng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::ProgressFunc;
use crate::Ray;
use crate::RenderStats;
use crate::camera::Camera;
use crate::color::RGB;
use crate::image::Image;
use crate::light::Light;
use crate::material::Material;
use crate::three_d::Object;
use crate::vec3::EPSILON;
use crate::vec3::Float;
use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::vec3::rand01;

const PI: Float = std::f32::consts::PI;

/// Per-sample radiance ceiling for the firefly clamp (see calc_ray_box_path
/// / RGB::clamp_max). This is a bias/variance knob: lower removes more
/// speckle but dims genuinely bright indirect highlights; higher is closer
/// to unbiased but leaves the worst outliers. Tuned empirically for the
/// bundled scenes at 6.0 -- above the tone-map knee (0.75) where highlights
/// compress toward white, so clamped bright reflections (the gold teapot,
/// mirror spheres) are visually unaffected, while the rare
/// diffuse->mirror->light speckle is trimmed. Full speckle removal needs
/// MIS, which this deliberately is not.
const FIREFLY_CLAMP: Float = 6.0;

/// A light primitive registered for next-event estimation (direct light
/// sampling). Built at scene-load time from every emissive sphere and
/// standalone triangle. `obj_idx` is the index into `RenderJob::objects`
/// of the same primitive, so shadow rays can (a) exclude the light itself
/// as an occluder and (b) let the path integrator skip re-counting the
/// light's emission when a scattered ray lands on it (avoiding double
/// counting -- see `trace_ray_path`).
pub enum NeeLight {
    Sphere {
        obj_idx: usize,
        center: Point,
        radius: Float,
        le: RGB,
    },
    Triangle {
        obj_idx: usize,
        p0: Point,
        e1: Vec3,
        e2: Vec3,
        normal: Vec3,
        area: Float,
        le: RGB,
    },
}

/// A point sampled on a light's surface, with everything the direct-light
/// estimator needs.
struct LightSample {
    point: Point,
    normal: Vec3,
    area: Float,
    le: RGB,
}

impl NeeLight {
    pub fn from_sphere(obj_idx: usize, center: Point, radius: Float, le: RGB) -> Self {
        NeeLight::Sphere {
            obj_idx,
            center,
            radius,
            le,
        }
    }

    pub fn from_triangle(obj_idx: usize, points: [Point; 3], le: RGB) -> Self {
        let e1 = points[1] - points[0];
        let e2 = points[2] - points[0];
        let cross = e1.cross(e2);
        let area = 0.5 * cross.norm();
        NeeLight::Triangle {
            obj_idx,
            p0: points[0],
            e1,
            e2,
            normal: cross.normalize(),
            area,
            le,
        }
    }

    pub fn obj_idx(&self) -> usize {
        match self {
            NeeLight::Sphere { obj_idx, .. } => *obj_idx,
            NeeLight::Triangle { obj_idx, .. } => *obj_idx,
        }
    }

    /// Sample a point uniformly on the light's surface.
    ///
    /// `ref_point` is the surface being lit. The sampled normal is always
    /// oriented toward it, so `direct_light`'s `cos_l > 0` test can never
    /// reject a light for merely being "the wrong way round" -- see the
    /// per-variant notes below for why that orientation is the physically
    /// right one in each case.
    fn sample(&self, rnd_state: &mut u64, ref_point: Point) -> LightSample {
        match self {
            NeeLight::Sphere {
                center,
                radius,
                le,
                ..
            } => {
                let dir = Vec3::gen_rnd_sphere(rnd_state);
                // Orient the normal toward the receiver. Sampling always
                // yields an *outward* normal, which is right when the
                // receiver is outside (the inward-facing half is
                // self-occluded anyway, and direct_light's cos_l <= 0 test
                // discards it for free). But for a sphere that ENCLOSES the
                // scene -- a "sky dome" -- every receiver is inside, so the
                // outward normal makes cos_l < 0 for every sample and the
                // dome lights nothing at all. Flipping to the inward side
                // there is what the geometry actually radiates from.
                let inside = (ref_point - *center).norm() < *radius;
                let normal = if inside { dir * -1.0 } else { dir };
                LightSample {
                    point: *center + dir * *radius,
                    normal,
                    area: 4.0 * PI * radius * radius,
                    le: *le,
                }
            }
            NeeLight::Triangle {
                p0,
                e1,
                e2,
                normal,
                area,
                le,
                ..
            } => {
                let mut r1 = rand01(rnd_state);
                let mut r2 = rand01(rnd_state);
                if r1 + r2 > 1.0 {
                    r1 = 1.0 - r1;
                    r2 = 1.0 - r2;
                }
                let point = *p0 + *e1 * r1 + *e2 * r2;
                // Triangle emitters are TWO-SIDED, so orient the normal
                // toward the receiver. The BSDF path already treats them
                // that way -- trace_ray_path returns `ke` for whichever
                // face a ray lands on, with no facing check -- so NEE has
                // to agree or the two disagree about what the light is.
                //
                // When they disagreed, a panel wound "backwards" (geometric
                // normal facing away from the room) lost its direct light
                // *entirely* rather than merely being one-sided: NEE
                // rejected every sample via cos_l <= 0, while the diffuse
                // continuation ray that hit it returned zero because
                // emission is suppressed for NEE-registered lights. Three
                // shipped scenes had exactly that bug, and nothing warned:
                // an emitter looks identical from both sides to the camera.
                // Orienting here makes winding irrelevant by construction.
                let to_ref = ref_point - point;
                let normal = if normal.dot(to_ref) < 0.0 {
                    *normal * -1.0
                } else {
                    *normal
                };
                LightSample {
                    point,
                    normal,
                    area: *area,
                    le: *le,
                }
            }
        }
    }
}

/// Identifies the specific primitive a ray hit: which object in
/// `RenderJob::objects`, and which sub-primitive within it (only
/// meaningful for `Mesh`, where it's a triangle index; ignored -- but
/// still excludes the whole object -- for single-primitive objects like
/// `Sphere`, `Plane`, and standalone `Triangle`).
///
/// Passed to secondary rays (reflection, shadow) so they can exclude the
/// exact primitive they originate from, rather than relying purely on a
/// distance-based epsilon bias. Combined with EPSILON, this avoids
/// self-intersection artifacts (e.g. dark banding on reflective floors
/// near the horizon, or self-shadowing noise on curved surfaces lit by
/// spot lights) without needing a larger epsilon that could otherwise
/// reject legitimate nearby intersections.
#[derive(Clone, Copy)]
struct HitId {
    obj_idx: usize,
    sub_id: usize,
}

/// Find the closest object in `objects` that `ray` intersects, optionally
/// excluding one specific primitive (see `HitId`). Returns the index of
/// the hit object, the sub-id reported by that object's `intercept`, and
/// updates `t` to the intersection distance.
fn find_closest_hit(
    objects: &[Arc<dyn Object + Send + Sync>],
    stats: &mut RenderStats,
    ray: &Ray,
    tmin: Float,
    t: &mut Float,
    exclude: Option<HitId>,
) -> Option<HitId> {
    let mut hit = None;
    for (obj_idx, obj) in objects.iter().enumerate() {
        let exclude_sub_id = match exclude {
            Some(e) if e.obj_idx == obj_idx => Some(e.sub_id),
            _ => None,
        };
        let mut sub_id = 0;
        if obj.intercept(stats, ray, tmin, t, false, &mut sub_id, exclude_sub_id) {
            hit = Some(HitId { obj_idx, sub_id });
        }
    }
    hit
}

/// Test whether any object in `objects` occludes `ray`, optionally
/// excluding one specific primitive (see `HitId`). Used for shadow rays.
fn is_occluded(
    objects: &[Arc<dyn Object + Send + Sync>],
    stats: &mut RenderStats,
    ray: &Ray,
    tmin: Float,
    tmax: Float,
    exclude: Option<HitId>,
) -> bool {
    objects.iter().enumerate().any(|(obj_idx, obj)| {
        let exclude_sub_id = match exclude {
            Some(e) if e.obj_idx == obj_idx => Some(e.sub_id),
            _ => None,
        };
        let mut tmax0 = tmax;
        let mut oid0 = 0;
        obj.intercept(
            stats,
            ray,
            tmin,
            &mut tmax0,
            true,
            &mut oid0,
            exclude_sub_id,
        )
    })
}

pub struct RenderConfig {
    pub path_tracing: u32,
    pub use_lines: bool,
    pub use_hashmap: bool,
    pub use_adaptive_sampling: bool,
    pub use_gamma: bool,
    pub adaptive_max_depth: u32,
    pub reflection_max_depth: u32,
    pub res_x: u32,
    pub res_y: u32,
    pub scene_file: PathBuf,
    pub image_file: PathBuf,
}

pub struct RenderJob {
    pub camera: Camera,
    pub objects: Vec<Arc<dyn Object + 'static + Send + Sync>>,
    pub lights: Vec<Arc<dyn Light + 'static + Send + Sync>>,
    pub materials: Vec<Arc<Material>>,
    pub image: Arc<Mutex<Image>>,
    pub cfg: RenderConfig,
    pub progress_total: Mutex<usize>,
    pub progress_func: ProgressFunc,
    pub start_ts: Instant,
    pub total_stats: Mutex<RenderStats>,
    /// Emissive primitives importance-sampled for next-event estimation.
    pub nee_lights: Vec<NeeLight>,
    /// Parallel to `objects`: true where an object is one of `nee_lights`.
    /// Lets the path integrator avoid double-counting a light that a
    /// scattered ray happens to hit after it was already sampled by NEE.
    pub obj_is_nee_light: Vec<bool>,
}

impl RenderJob {
    pub fn set_progress_func(&mut self, func: Box<dyn Fn(f32) + Send + Sync>) {
        self.progress_func.func = func;
    }
    fn report_progress(&self, v: u32) {
        let denom = self.cfg.res_x * self.cfg.res_y;
        let mut total = self.progress_total.lock().unwrap();
        let before = (*total).div_euclid((denom / 128) as usize);
        *total += v as usize;
        let after = (*total).div_euclid((denom / 128) as usize);
        let d = before != after || 100 * (denom as i32 - *total as i32).unsigned_abs() / denom < 1;
        if d {
            let pct = *total as f32 / denom as f32;
            (self.progress_func.func)(pct.min(1.0));
        }
    }

    fn trace_ray(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        depth: u32,
        exclude: Option<HitId>,
    ) -> RGB {
        if depth > self.cfg.reflection_max_depth {
            stats.num_rays_reflection_max += 1;
            return RGB::zero();
        }
        let mut t = Float::MAX;

        let hit = find_closest_hit(&self.objects, stats, ray, EPSILON, &mut t, exclude);

        if let Some(hit_id) = hit {
            let hit_obj = &self.objects[hit_id.obj_idx];
            let hit_point = ray.orig + ray.dir * t;
            let mut hit_normal = hit_obj.get_normal(hit_point, hit_id.sub_id);
            // Two-sided shading: some meshes (e.g. buddha.obj) have
            // inconsistent triangle winding, so a triangle's geometric
            // normal may point away from the viewer. Flip it to face the
            // incoming ray so lighting is computed on the visible side,
            // avoiding scattered mis-lit ("speckle") pixels.
            if hit_normal.dot(ray.dir) > 0.0 {
                hit_normal = hit_normal * -1.0;
            }
            let hit_mat_id = hit_obj.get_material_id(hit_id.sub_id);
            let hit_material = &self.materials[hit_mat_id];

            let mut c = self.lights.iter().fold(RGB::zero(), |acc, light| {
                let mut c_light = RGB::zero();

                if !light.is_spot() {
                    c_light = light.get_contrib(ray, hit_material, hit_point, hit_normal);
                } else {
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let light_ray = Ray::new(hit_point, light_vec);
                    // Shadow rays originate from the surface we just hit,
                    // so exclude that exact primitive (hit_id) rather than
                    // relying on a distance-based epsilon bias -- avoids
                    // self-shadowing artifacts regardless of scene scale
                    // or mesh feature size (see HitId doc comment).
                    if !is_occluded(&self.objects, stats, &light_ray, EPSILON, 1.0, Some(hit_id)) {
                        c_light = light.get_contrib(ray, hit_material, hit_point, hit_normal)
                    }
                }
                acc + c_light
            });

            if hit_material.checkered {
                let hit_text2d = hit_obj.get_texture_2d(hit_point);
                c = hit_material.do_checker(c, hit_text2d);
            }

            if !hit_material.ks.is_zero() {
                stats.num_rays_reflection += 1;
                let reflected_ray = ray.get_reflection(hit_point, hit_normal);
                // Same reasoning as shadow rays above: exclude the exact
                // primitive the reflection ray originates from.
                let c_reflect = self.trace_ray(stats, &reflected_ray, depth + 1, Some(hit_id));
                let ks = hit_material
                    .ks
                    .r
                    .max(hit_material.ks.g)
                    .max(hit_material.ks.b);
                c = c * (1.0 - ks) + c_reflect * ks;
            }
            c
        } else {
            let screen_v = self.camera.screen_v.normalize();
            let s = ray.dir.dot(screen_v).abs() / ray.dir.norm();
            let cmax = RGB::new(1.0, 1.0, 1.0);
            let cyan = RGB::new(0.4, 0.6, 0.9);
            cmax * s + cyan * (1.0 - s)
        }
    }
    /// Estimate direct illumination at a diffuse surface point by sampling
    /// one light (next-event estimation). Picks a light uniformly, samples
    /// a point on it, and adds its contribution if the point is visible.
    ///
    /// The estimator matches this path tracer's convention where the
    /// cosine-weighted continuation returns `kd * L_i` (implicit BRDF
    /// `kd/PI`), so the direct term carries an explicit `1/PI`:
    ///   L = (kd/PI) * Le * (cos_s * cos_l / dist^2) * Area * N
    /// with `Area * N` = 1 / pdf_area for uniformly picking one of N lights
    /// and then a uniform point on its surface.
    fn direct_light(
        &self,
        stats: &mut RenderStats,
        rnd_state: &mut u64,
        hit_point: Point,
        hit_normal: Vec3,
        kd: RGB,
        origin: HitId,
    ) -> RGB {
        let n = self.nee_lights.len();
        if n == 0 {
            return RGB::zero();
        }
        // Pick one light uniformly.
        let idx = (rand01(rnd_state) * n as Float) as usize;
        let light = &self.nee_lights[idx.min(n - 1)];
        let s = light.sample(rnd_state, hit_point);

        let to_light = s.point - hit_point;
        let dist2 = to_light.dot(to_light);
        if dist2 < EPSILON {
            return RGB::zero();
        }
        let dist = dist2.sqrt();
        let wi = to_light / dist;

        let cos_s = hit_normal.dot(wi);
        let cos_l = s.normal.dot(wi * -1.0);
        // The surface must face the light. `cos_l` is now only a degenerate
        // case guard: `sample()` already orients the light's normal toward
        // this point, so cos_l <= 0 just means the receiver lies in the
        // light's own plane (zero projected area -- it radiates nothing
        // this way). It is no longer a "wrong side" rejection.
        if cos_s <= 0.0 || cos_l <= 0.0 {
            return RGB::zero();
        }

        // Visibility: shadow ray toward the sampled point. Stop just short
        // of the light so the light's own surface doesn't count as an
        // occluder; exclude the originating surface primitive.
        let shadow_ray = Ray::new(hit_point, wi);
        let tmax = dist * (1.0 - 1e-3);
        stats.num_rays_reflection += 1;
        if is_occluded(&self.objects, stats, &shadow_ray, EPSILON, tmax, Some(origin)) {
            return RGB::zero();
        }

        let g = cos_s * cos_l / dist2;
        kd * s.le * (g * s.area * n as Float / PI)
    }

    fn trace_ray_path(
        &self,
        stats: &mut RenderStats,
        rnd_state: &mut u64,
        ray: &Ray,
        depth: u32,
        exclude: Option<HitId>,
        // Whether emission from an emitter this ray lands on should be
        // added. True for camera rays and rays leaving a specular (mirror)
        // bounce; false for the diffuse continuation, whose direct light
        // is already accounted for by next-event estimation at the bounce
        // it left -- but only for lights that NEE actually samples (see
        // `obj_is_nee_light`), so emitters outside the NEE set still count.
        count_emission: bool,
    ) -> RGB {
        if depth > self.cfg.reflection_max_depth {
            stats.num_rays_reflection_max += 1;
            return RGB::zero();
        }
        let mut t = Float::MAX;

        let hit = find_closest_hit(&self.objects, stats, ray, EPSILON, &mut t, exclude);

        let Some(hit_id) = hit else {
            return RGB::zero();
        };

        let hit_obj = &self.objects[hit_id.obj_idx];
        let hit_mat_id = hit_obj.get_material_id(hit_id.sub_id);
        let hit_material = &self.materials[hit_mat_id];

        if !hit_material.ke.is_zero() {
            // Count this emitter unless it was already sampled by NEE at
            // the previous diffuse bounce (avoids double counting).
            if count_emission || !self.obj_is_nee_light[hit_id.obj_idx] {
                return hit_material.ke;
            }
            return RGB::zero();
        }

        let hit_point = ray.orig + ray.dir * t;
        let mut hit_normal = hit_obj.get_normal(hit_point, hit_id.sub_id);
        // Two-sided shading (see trace_ray): flip the normal to face the
        // incoming ray for meshes with inconsistent triangle winding.
        if hit_normal.dot(ray.dir) > 0.0 {
            hit_normal = hit_normal * -1.0;
        }
        stats.num_rays_reflection += 1;

        if hit_material.ks.is_zero() {
            // Diffuse: direct light via NEE + indirect via a cosine-weighted
            // continuation that does NOT re-count NEE-sampled emitters.
            let direct =
                self.direct_light(stats, rnd_state, hit_point, hit_normal, hit_material.kd, hit_id);

            // Lambertian scatter around the surface normal.
            let mut dir = hit_normal + Vec3::gen_rnd_sphere(rnd_state);
            if dir.norm() < EPSILON {
                dir = hit_normal;
            }
            let scattered_ray = Ray::new(hit_point, dir.normalize());
            let indirect =
                self.trace_ray_path(stats, rnd_state, &scattered_ray, depth + 1, Some(hit_id), false);
            direct + indirect * hit_material.kd
        } else {
            // Perfect mirror: no NEE (specular is a delta); the reflected
            // ray sees emitters directly.
            let reflected_ray = ray.get_reflection(hit_point, hit_normal);
            let c0 =
                self.trace_ray_path(stats, rnd_state, &reflected_ray, depth + 1, Some(hit_id), true);
            c0 * hit_material.ks
        }
    }

    fn trace_primary_ray(
        &self,
        stats: &mut RenderStats,
        pmap: &mut HashMap<u64, RGB>,
        u: Float,
        v: Float,
    ) -> RGB {
        let mut key = 0;
        if self.cfg.use_hashmap {
            // need to use f64 otherwise the loss of precision bites us
            key = (1e12 * (u as f64 + 0.5) + 1e6 * (v as f64 + 0.5)) as u64;
            if self.cfg.use_adaptive_sampling {
                if let Some(c) = pmap.get(&key) {
                    return *c;
                }
            }
        }
        let ray = self.camera.get_ray(u, v);

        stats.num_rays_sampling += 1;

        let c = self.trace_ray(stats, &ray, 0 /* depth */, None);
        if self.cfg.use_hashmap && self.cfg.use_adaptive_sampling {
            pmap.insert(key, c);
        }
        c
    }

    /*
     * pos_u: -0.5 .. 0.5
     * pos_v: -0.5 .. 0.5
     */
    fn calc_ray_box_path(
        &self,
        stats: &mut RenderStats,
        pos_u: Float,
        pos_v: Float,
        du: Float,
        dv: Float,
    ) -> RGB {
        assert!(!self.cfg.use_adaptive_sampling);
        assert!(self.cfg.path_tracing > 1);

        let mut c = RGB::zero();
        let mut rng = rand::thread_rng();
        let mut rnd_state = rng.gen_range(0..u64::MAX);

        for _i in 0..self.cfg.path_tracing {
            let off_u = rng.gen_range(0.0..du);
            let off_v = rng.gen_range(0.0..dv);
            let ray = self.camera.get_ray(pos_u + off_u, pos_v + off_v);

            stats.num_rays_sampling += 1;

            // Per-sample firefly clamp: cap outlier radiance from rare
            // high-variance paths before averaging (see RGB::clamp_max).
            // The ceiling is above scene emitter values so directly-visible
            // lights and normal shading are unaffected.
            let sample = self
                .trace_ray_path(stats, &mut rnd_state, &ray, 0, None, true)
                .clamp_max(FIREFLY_CLAMP);
            c += sample;
        }
        c / self.cfg.path_tracing as f32
    }

    /*
     * pos_u: -0.5 .. 0.5
     * pos_v: -0.5 .. 0.5
     */
    #[allow(clippy::too_many_arguments)]
    fn calc_ray_box(
        &self,
        stats: &mut RenderStats,
        pmap: &mut HashMap<u64, RGB>,
        pos_u: Float,
        pos_v: Float,
        du: Float,
        dv: Float,
        lvl: u32,
    ) -> RGB {
        if !self.cfg.use_adaptive_sampling {
            return self.trace_primary_ray(stats, pmap, pos_u + du / 2.0, pos_v + dv / 2.0);
        }
        let mut c00 = self.trace_primary_ray(stats, pmap, pos_u, pos_v);
        let mut c01 = self.trace_primary_ray(stats, pmap, pos_u, pos_v + dv);
        let mut c10 = self.trace_primary_ray(stats, pmap, pos_u + du, pos_v);
        let mut c11 = self.trace_primary_ray(stats, pmap, pos_u + du, pos_v + dv);

        if lvl < self.cfg.adaptive_max_depth {
            let color_diff = RGB::difference(c00, c01, c10, c11) > 0.3;
            if color_diff {
                let du2 = du / 2.0;
                let dv2 = dv / 2.0;
                c00 = self.calc_ray_box(stats, pmap, pos_u, pos_v, du2, dv2, lvl + 1);
                c01 = self.calc_ray_box(stats, pmap, pos_u, pos_v + dv2, du2, dv2, lvl + 1);
                c10 = self.calc_ray_box(stats, pmap, pos_u + du2, pos_v, du2, dv2, lvl + 1);
                c11 = self.calc_ray_box(stats, pmap, pos_u + du2, pos_v + dv2, du2, dv2, lvl + 1);
            }
        } else {
            stats.num_rays_sampling_max += 1;
        }
        (c00 + c01 + c10 + c11) * 0.25
    }

    pub fn print_stats(&self) {
        let stats = self.total_stats.lock().unwrap();
        let pretty_print = |n| {
            let mut precision = 3;
            let suffix;
            let val;
            if n > 1_000_000_000_000 {
                val = n as Float / 1_000_000_000_000.0;
                suffix = "T";
            } else if n > 1_000_000_000 {
                val = n as Float / 1_000_000_000.0;
                suffix = "G";
            } else if n >= 1_000_000 {
                val = n as Float / 1_000_000.0;
                suffix = "M";
            } else {
                val = n as Float;
                suffix = " ";
                precision = 0
            }
            format!("{val:6.precision$} {suffix}")
        };
        let elapsed = self.start_ts.elapsed();
        let num_rays = (stats.num_rays_sampling + stats.num_rays_reflection) as Float;
        let tot_lat_str = format!("{:.2} sec", elapsed.as_millis() as Float / 1000.0);
        let ray_lat_str = format!(
            "{:.3} usec",
            elapsed.as_micros() as Float / num_rays as Float
        );
        let kray_per_secs = num_rays / (elapsed.as_secs_f32() as Float) / 1_000 as Float;
        let mut v = kray_per_secs;
        let mut suffix = "K";
        if kray_per_secs >= 1000.0 {
            v = kray_per_secs / 1000.0;
            suffix = "M";
        }
        let xray_sec_str = format!("{v:.3}");

        println!(
            "duration: {} -- {} per ray -- {} {}rays/sec",
            tot_lat_str.bold(),
            ray_lat_str.bold(),
            xray_sec_str.bold(),
            suffix
        );
        let intersect_stats = [
            ("Sphere", stats.num_intersects_sphere),
            ("Plane", stats.num_intersects_plane),
            ("Triangle", stats.num_intersects_triangle),
            ("AABB", stats.num_intersects_aabb),
        ];

        for (s, n) in intersect_stats {
            println!(
                "num_intersects {:<10}{:>12}",
                format!("{s}:"),
                pretty_print(n)
            );
        }

        let num_pixels = (self.cfg.res_x * self.cfg.res_y) as u64;
        let ray_stats = [
            ("num_rays_sampling", stats.num_rays_sampling, num_pixels),
            (
                "num_rays_sampling_max",
                stats.num_rays_sampling_max,
                stats.num_rays_sampling,
            ),
            (
                "num_rays_reflection",
                stats.num_rays_reflection,
                stats.num_rays_sampling,
            ),
            (
                "num_rays_reflection_max",
                stats.num_rays_reflection_max,
                stats.num_rays_sampling,
            ),
        ];
        for (s, n, d) in ray_stats {
            println!(
                "{:<24} {:>12} -- {:3}%",
                format!("{s}:"),
                pretty_print(n),
                100 * n / d
            );
        }
    }

    fn render_pixel_box(&self, x0: u32, y0: u32, sz_x: u32, sz_y: u32, stats: &mut RenderStats) {
        let u = 1.0;
        let v = 1.0;
        let du = u / self.cfg.res_x as Float;
        let dv = v / self.cfg.res_y as Float;
        let y_max = (y0 + sz_y).min(self.cfg.res_y);
        let x_max = (x0 + sz_x).min(self.cfg.res_x);

        let mut pmap = HashMap::new();

        for y in y0..y_max {
            let pos_v = v / 2.0 - (y as Float) * dv;
            for x in x0..x_max {
                let pos_u = u / 2.0 - (x as Float) * du;
                let c = if self.cfg.path_tracing > 1 {
                    self.calc_ray_box_path(stats, pos_u, pos_v, du, dv)
                } else {
                    self.calc_ray_box(stats, &mut pmap, pos_u, pos_v, du, dv, 0)
                };

                self.image.lock().unwrap().push_pixel(x, y, c);
            }
        }
    }

    fn render_image_lines(&mut self, exit_req: Arc<AtomicBool>) {
        (0..self.cfg.res_y).into_par_iter().for_each(|y| {
            let mut stats: RenderStats = Default::default();

            if exit_req.load(Ordering::SeqCst) {
                self.report_progress(self.cfg.res_x);
                return;
            }
            self.render_pixel_box(0, y, self.cfg.res_x, 1, &mut stats);
            self.report_progress(self.cfg.res_x);
            self.total_stats.lock().unwrap().add(stats);
        });
    }

    fn render_image_box(&mut self, exit_req: Arc<AtomicBool>) {
        let mut step = 32;
        if self.cfg.path_tracing > 1 {
            step = 10;
        }
        let ny = self.cfg.res_y.div_ceil(step);
        let nx = self.cfg.res_x.div_ceil(step);
        (0..ny * nx).into_par_iter().for_each(|v| {
            let mut stats: RenderStats = Default::default();
            let x = (v % nx) * step;
            let y = (v / nx) * step;

            if exit_req.load(Ordering::SeqCst) {
                self.report_progress(step * step);
                return;
            }
            self.render_pixel_box(x, y, step, step, &mut stats);
            self.report_progress(step * step);
            self.total_stats.lock().unwrap().add(stats);
        });
    }

    pub fn alloc_image(&mut self) {
        // Tone-map only in path-tracing mode (HDR radiance); classic
        // ray-traced scenes keep their original hard-clamp look.
        let tone_map = self.cfg.path_tracing > 1;
        self.image = Arc::new(Mutex::new(Image::new(
            self.cfg.use_gamma,
            tone_map,
            self.cfg.res_x,
            self.cfg.res_y,
        )));
    }

    pub fn render_scene(&mut self, exit_req: Arc<AtomicBool>) {
        if self.cfg.use_lines {
            self.render_image_lines(exit_req);
        } else {
            self.render_image_box(exit_req);
        }
    }

    pub fn save_image(&mut self) -> std::io::Result<()> {
        return self.image.lock().unwrap().save_image(&self.cfg.image_file);
    }
}
