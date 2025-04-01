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
use crate::vec3::Vec3;

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

    fn trace_ray(&self, stats: &mut RenderStats, ray: &Ray, depth: u32) -> RGB {
        if depth > self.cfg.reflection_max_depth {
            stats.num_rays_reflection_max += 1;
            return RGB::zero();
        }
        let mut s_id = 0;
        let mut t = Float::MAX;

        let hit_obj_opt = self
            .objects
            .iter()
            .filter(|obj| obj.intercept(stats, ray, EPSILON, &mut t, false, &mut s_id))
            .last();

        if let Some(hit_obj) = hit_obj_opt {
            let hit_point = ray.orig + ray.dir * t;
            let hit_normal = hit_obj.get_normal(hit_point, s_id);
            let hit_mat_id = hit_obj.get_material_id();
            let hit_material = &self.materials[hit_mat_id];

            let mut c = self.lights.iter().fold(RGB::zero(), |acc, light| {
                let mut c_light = RGB::zero();

                if !light.is_spot() {
                    c_light = light.get_contrib(ray, hit_material, hit_point, hit_normal);
                } else {
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let light_ray = Ray::new(hit_point, light_vec);
                    if !self.objects.iter().any(|obj| {
                        let mut tmax0 = 1.0;
                        let mut oid0 = 0;
                        obj.intercept(stats, &light_ray, EPSILON, &mut tmax0, true, &mut oid0)
                    }) {
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
                let c_reflect = self.trace_ray(stats, &reflected_ray, depth + 1);
                let ks = 0.1;
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
    fn trace_ray_path(
        &self,
        stats: &mut RenderStats,
        rnd_state: &mut u64,
        ray: &Ray,
        depth: u32,
    ) -> RGB {
        if depth > self.cfg.reflection_max_depth {
            stats.num_rays_reflection_max += 1;
            return RGB::zero();
        }
        let mut s_id = 0;
        let mut t = Float::MAX;

        let hit_obj = self
            .objects
            .iter()
            .filter(|obj| obj.intercept(stats, ray, EPSILON, &mut t, false, &mut s_id))
            .last();

        if hit_obj.is_none() {
            return RGB::zero();
        }

        let hit_mat_id = hit_obj.unwrap().get_material_id();
        let hit_material = &self.materials[hit_mat_id];

        if !hit_material.ke.is_zero() {
            return hit_material.ke;
        }

        let hit_point = ray.orig + ray.dir * t;
        let hit_normal = hit_obj.unwrap().get_normal(hit_point, s_id);
        stats.num_rays_reflection += 1;
        let mut reflected_ray = ray.get_reflection(hit_point, hit_normal);
        if hit_material.ks.is_zero() {
            let dir = reflected_ray.dir.normalize() + Vec3::gen_rnd_sphere(rnd_state);
            reflected_ray.dir = dir.normalize();
        }
        let c0 = self.trace_ray_path(stats, rnd_state, &reflected_ray, depth + 1);
        if hit_material.ks.is_zero() {
            c0 * hit_material.kd
        } else {
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

        let c = self.trace_ray(stats, &ray, 0 /* depth */);
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

            c += self.trace_ray_path(stats, &mut rnd_state, &ray, 0);
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
        self.image = Arc::new(Mutex::new(Image::new(
            self.cfg.use_gamma,
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
