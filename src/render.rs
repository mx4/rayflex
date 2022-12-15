use colored::Colorize;
use rand::Rng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::camera::Camera;
use crate::color::RGB;
use crate::image::Image;
use crate::light::AmbientLight;
use crate::light::Light;
use crate::light::SpotLight;
use crate::light::VectorLight;
use crate::material::Material;
use crate::vec3::Float;
use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::vec3::EPSILON;
use crate::Ray;
use crate::RenderStats;

use crate::three_d::Mesh;
use crate::three_d::Object;
use crate::three_d::Plane;
use crate::three_d::Sphere;
use crate::three_d::Triangle;
use crate::ProgressFunc;

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
    camera: Option<Camera>,
    objects: Vec<Arc<dyn Object + 'static + Send + Sync>>,
    lights: Vec<Arc<dyn Light + 'static + Send + Sync>>,
    materials: Vec<Arc<Material>>,
    pub image: Arc<Mutex<Image>>,
    cfg: RenderConfig,
    progress_total: Mutex<usize>,
    progress_func: ProgressFunc,
    start_ts: Instant,
    total_stats: Mutex<RenderStats>,
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

    pub fn new(cfg: RenderConfig) -> Self {
        Self {
            camera: None,
            image: Arc::new(Mutex::new(Image::new(false, 0, 0))),
            objects: vec![],
            lights: vec![],
            materials: vec![],
            cfg,
            progress_total: Mutex::new(0),
            progress_func: ProgressFunc {
                func: Box::new(|_| {}),
            },
            start_ts: Instant::now(),
            total_stats: Mutex::new(Default::default()),
        }
    }
    fn trace_ray(&self, stats: &mut RenderStats, ray: &Ray, depth: u32) -> RGB {
        if depth > self.cfg.reflection_max_depth {
            stats.num_rays_reflection_max += 1;
            return RGB::new();
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

            let mut c = self.lights.iter().fold(RGB::new(), |acc, light| {
                let mut c_light = RGB::new();

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

            if hit_material.ks > 0.0 {
                stats.num_rays_reflection += 1;
                let reflected_ray = ray.get_reflection(hit_point, hit_normal);
                let c_reflect = self.trace_ray(stats, &reflected_ray, depth + 1);
                let ks = 0.1;
                c = c * (1.0 - ks) + c_reflect * ks;
            }
            c
        } else {
            //let z = (ray.dir.z + 0.5).clamp(0.0, 1.0) as f32;
            let screen_v = self.camera.as_ref().unwrap().screen_v.normalize();
            let s = ray.dir.dot(screen_v).abs() / ray.dir.norm();
            let cmax = RGB {
                r: 1.0,
                g: 1.0,
                b: 1.0,
            };
            let cyan = RGB {
                r: 0.4,
                g: 0.6,
                b: 0.9,
            };
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
            return RGB::new();
        }
        let mut s_id = 0;
        let mut t = Float::MAX;

        let hit_obj = self
            .objects
            .iter()
            .filter(|obj| obj.intercept(stats, ray, EPSILON, &mut t, false, &mut s_id))
            .last();

        if hit_obj.is_none() {
            return RGB::new();
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
        if hit_material.ks == 0.0 {
            let dir = reflected_ray.dir.normalize() + Vec3::gen_rnd_sphere(rnd_state);
            reflected_ray.dir = dir.normalize();
        }
        let c0 = self.trace_ray_path(stats, rnd_state, &reflected_ray, depth + 1);
        if hit_material.ks == 0.0 {
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
            // need to use f64 otherwise loss of precision bites us
            key = (1e12 * (u as f64 + 0.5) + 1e6 * (v as f64 + 0.5)) as u64;
            if self.cfg.use_adaptive_sampling {
                if let Some(c) = pmap.get(&key) {
                    return *c;
                }
            }
        }
        let ray = self.camera.as_ref().unwrap().get_ray(u, v);

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

        let mut c = RGB::new();
        let mut rng = rand::thread_rng();
        let mut rnd_state = rng.gen_range(0..u64::MAX);

        for _i in 0..self.cfg.path_tracing {
            let off_u = rng.gen_range(0.0..du);
            let off_v = rng.gen_range(0.0..dv);
            let ray = self
                .camera
                .as_ref()
                .unwrap()
                .get_ray(pos_u + off_u, pos_v + off_v);

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
            format!("{:6.precision$} {suffix}", val)
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
        let xray_sec_str = format!("{:.3}", v);

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
                format!("{}:", s),
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
                format!("{}:", s),
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
        let ny = (self.cfg.res_y + step - 1) / step;
        let nx = (self.cfg.res_x + step - 1) / step;
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
        assert!(self.camera.is_some());

        if self.cfg.use_lines {
            self.render_image_lines(exit_req);
        } else {
            self.render_image_box(exit_req);
        }
    }

    pub fn load_scene(&mut self) -> std::io::Result<()> {
        if !self.cfg.scene_file.is_file() {
            println!("file '{}' not found.", self.cfg.scene_file.display());
            println!("pwd={}", std::env::current_dir()?.display());
            panic!("scene file {} not present.", self.cfg.scene_file.display());
        }
        println!(
            "loading scene file {}",
            self.cfg.scene_file.display().to_string().bold()
        );

        let data = fs::read_to_string(&self.cfg.scene_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        let mut num_planes = 0;
        let mut num_spheres = 0;
        let mut num_triangles = 0;
        let mut num_triangles_in_all_objs = 0;
        let mut num_materials = 0;
        let mut num_vec_lights = 0;
        let mut num_spot_lights = 0;
        let mut num_objs = 0;

        if self.cfg.res_x == 0 && self.cfg.res_y == 0 {
            if let Some(array) = json[&"resolution".to_string()].as_array() {
                self.cfg.res_x = array[0].as_u64().unwrap() as u32;
                self.cfg.res_y = array[1].as_u64().unwrap() as u32;
            }
        }
        let res_str = format!("{}x{}", self.cfg.res_x, self.cfg.res_y).bold();
        let mut smp_str = "".cyan();
        if self.cfg.use_adaptive_sampling {
            smp_str = " w/ adaptive sampling".cyan();
        }
        println!("-- img resolution: {}{}", res_str, smp_str);

        let mut camera: Camera = serde_json::from_value(json["camera"].clone()).unwrap();
        camera.aspect = self.cfg.res_x as Float / self.cfg.res_y as Float;
        camera.init();
        self.camera = Some(camera);

        if let Ok(ambient) = serde_json::from_value::<AmbientLight>(json["ambient"].clone()) {
            self.lights.push(Arc::new(ambient));
        }

        loop {
            let s = format!("material.{}", num_materials);
            match serde_json::from_value::<Material>(json[&s].clone()) {
                Err(_error) => break,
                Ok(mat) => {
                    self.materials.push(Arc::new(mat));
                    num_materials += 1;
                }
            }
        }
        loop {
            let s = format!("spot-light.{}", num_spot_lights);
            match serde_json::from_value::<SpotLight>(json[&s].clone()) {
                Err(_error) => break,
                Ok(mut spot) => {
                    spot.name = s;
                    self.lights.push(Arc::new(spot));
                    num_spot_lights += 1;
                }
            }
        }
        loop {
            let s = format!("vec-light.{}", num_vec_lights);
            match serde_json::from_value::<VectorLight>(json[&s].clone()) {
                Err(_error) => break,
                Ok(mut v) => {
                    v.name = s;
                    v.dir = v.dir.normalize();
                    self.lights.push(Arc::new(v));
                    num_vec_lights += 1;
                }
            }
        }

        loop {
            let s = format!("plane.{}", num_planes);
            match serde_json::from_value::<Plane>(json[s].clone()) {
                Err(_error) => break,
                Ok(p) => {
                    self.objects.push(Arc::new(p));
                    num_planes += 1;
                }
            }
        }
        loop {
            let s = format!("sphere.{}", num_spheres);
            match serde_json::from_value::<Sphere>(json[s].clone()) {
                Err(_error) => break,
                Ok(o) => {
                    self.objects.push(Arc::new(o));
                    num_spheres += 1;
                }
            }
        }
        loop {
            let s = format!("triangle.{}", num_triangles);
            match serde_json::from_value::<Triangle>(json[s].clone()) {
                Err(_error) => break,
                Ok(o) => {
                    self.objects.push(Arc::new(o));
                    num_triangles += 1;
                }
            }
        }
        loop {
            let name = format!("obj.{}.path", num_objs);
            if json[&name].is_null() {
                break;
            }
            let path = json[&name].as_str().unwrap();
            let rxname = format!("obj.{}.rotx", num_objs);
            let ryname = format!("obj.{}.roty", num_objs);
            let rzname = format!("obj.{}.rotz", num_objs);
            let mut angle_x = 0.0;
            let mut angle_y = 0.0;
            let mut angle_z = 0.0;
            let mut angle_x_rad = 0.0;
            let mut angle_y_rad = 0.0;
            let mut angle_z_rad = 0.0;
            let mut num_triangles_in_obj = 0;
            if let Some(alpha) = json[&rxname].as_f64() {
                angle_x = alpha;
                angle_x_rad = angle_x.to_radians() as Float;
            }
            if let Some(alpha) = json[&ryname].as_f64() {
                angle_y = alpha;
                angle_y_rad = angle_y.to_radians() as Float;
            }
            if let Some(alpha) = json[&rzname].as_f64() {
                angle_z = alpha;
                angle_z_rad = angle_z.to_radians() as Float;
            }

            let opt = tobj::LoadOptions {
                triangulate: true, // converts polygon into triangles
                ignore_lines: true,
                ignore_points: true,
                ..Default::default()
            };
            let (models, materials) = tobj::load_obj(path, &opt).expect("tobj");
            if let Ok(mat) = materials {
                mat.iter().for_each(|m| {
                    println!("material: {:?} -- {:?}", m.name, m);
                });
            } else {
                println!(
                    "{} {:?}",
                    "Error loading materials:".red().bold(),
                    materials.unwrap_err()
                );
            }

            models.iter().for_each(|m| {
                let mesh = &m.mesh;
                let n = mesh.indices.len() / 3;
                println!(
                    "-- model {} has {} triangles w/ {} vertices",
                    m.name,
                    n,
                    mesh.positions.len()
                );
                assert!(mesh.indices.len() % 3 == 0);
                num_triangles_in_all_objs += n;
                num_triangles_in_obj += n;
                let mut triangles = Vec::with_capacity(n);
                let mut num_skipped = 0;
                for i in 0..n {
                    let i0 = mesh.indices[3 * i] as usize;
                    let i1 = mesh.indices[3 * i + 1] as usize;
                    let i2 = mesh.indices[3 * i + 2] as usize;
                    let x0 = mesh.positions[3 * i0] as Float;
                    let y0 = mesh.positions[3 * i0 + 1] as Float;
                    let z0 = mesh.positions[3 * i0 + 2] as Float;
                    let x1 = mesh.positions[3 * i1] as Float;
                    let y1 = mesh.positions[3 * i1 + 1] as Float;
                    let z1 = mesh.positions[3 * i1 + 2] as Float;
                    let x2 = mesh.positions[3 * i2] as Float;
                    let y2 = mesh.positions[3 * i2 + 1] as Float;
                    let z2 = mesh.positions[3 * i2 + 2] as Float;
                    let mut p0 = Point {
                        x: x0,
                        y: y0,
                        z: z0,
                    };
                    let mut p1 = Point {
                        x: x1,
                        y: y1,
                        z: z1,
                    };
                    let mut p2 = Point {
                        x: x2,
                        y: y2,
                        z: z2,
                    };

                    if p0 == p1 || p0 == p2 || p1 == p2 {
                        num_skipped += 1;
                        continue;
                    }
                    p0 = p0.rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad);
                    p1 = p1.rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad);
                    p2 = p2.rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad);
                    let mut triangle = Triangle::new([p0, p1, p2], 0);
                    triangle.mesh_id = triangles.len();
                    triangles.push(triangle);
                }
                if num_skipped > 0 {
                    println!("-- skipped {} malformed triangles", num_skipped);
                }
                self.objects.push(Arc::new(Mesh::new(triangles, 0)));
                num_objs += 1;
            });
            println!(
                "-- loaded {} w/ {} triangles -- rotx={} roty={} rotz={}",
                path.green(),
                num_triangles_in_obj,
                angle_x,
                angle_y,
                angle_z
            );
        }
        println!(
            "-- mesh={} triangles={} spheres={} planes={} materials={}",
            num_objs,
            num_triangles + num_triangles_in_all_objs,
            num_spheres,
            num_planes,
            num_materials
        );
        self.camera.as_ref().unwrap().display();

        self.lights.iter().for_each(|light| light.display());
        Ok(())
    }

    pub fn save_image(&mut self) -> std::io::Result<()> {
        return self.image.lock().unwrap().save_image(&self.cfg.image_file);
    }
}
