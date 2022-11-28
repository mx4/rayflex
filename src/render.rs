use colored::Colorize;
use indicatif::ProgressBar;
use rayon::prelude::*;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use raymax::camera::Camera;
use raymax::color::RGB;
use raymax::image::Image;
use raymax::light::AmbientLight;
use raymax::light::Light;
use raymax::light::SpotLight;
use raymax::light::VectorLight;
use raymax::material::Material;
use raymax::vec3::Point;
use raymax::Ray;
use raymax::RenderStats;

use raymax::three_d::Mesh;
use raymax::three_d::Object;
use raymax::three_d::Plane;
use raymax::three_d::Sphere;
use raymax::three_d::Triangle;
use raymax::three_d::EPSILON;

pub struct RenderConfig {
    pub use_lines: bool,
    pub use_hashmap: bool,
    pub use_adaptive_sampling: bool,
    pub use_gamma: bool,
    pub adaptive_max_depth: u32,
    pub reflection_max_depth: u32,
    pub res_x: u32,
    pub res_y: u32,
}

pub struct RenderJob {
    camera: Option<Camera>,
    objects: Vec<Arc<Box<dyn Object + 'static + Send + Sync>>>,
    lights: Vec<Arc<Box<dyn Light + 'static + Send + Sync>>>,
    materials: Vec<Arc<Box<Material>>>,
    image: Mutex<Image>,
    cfg: RenderConfig,
}

impl RenderJob {
    pub fn new(cfg: RenderConfig) -> Self {
        Self {
            camera: None,
            image: Mutex::new(Image::new(0, 0)),
            objects: vec![],
            lights: vec![],
            materials: vec![],
            cfg: cfg,
        }
    }
    fn trace(&self, stats: &mut RenderStats, ray: &Ray, depth: u32) -> RGB {
        if depth > self.cfg.reflection_max_depth {
            stats.num_rays_reflection_max += 1;
            return RGB::new();
        }
        let mut s_id = 0;
        let mut t = f64::MAX;

        let hit_obj = self
            .objects
            .iter()
            .filter(|obj| obj.intercept(stats, &ray, EPSILON, &mut t, false, &mut s_id))
            .last();

        if hit_obj.is_some() {
            let hit_point = ray.orig + ray.dir * t;
            let hit_normal = hit_obj.unwrap().get_normal(hit_point, s_id);
            let hit_mat_id = hit_obj.unwrap().get_material_id();
            let hit_material = &self.materials[hit_mat_id];

            let mut c = self.lights.iter().fold(RGB::new(), |acc, light| {
                let mut c_light = RGB::new();

                if !light.is_spot() {
                    c_light = light.get_contrib(ray, &hit_material, hit_point, hit_normal);
                } else {
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let light_ray = Ray::new(hit_point, light_vec);
                    self.objects
                        .iter()
                        .find(|obj| {
                            let mut tmax0 = 1.0;
                            let mut oid0 = 0;
                            obj.intercept(stats, &light_ray, EPSILON, &mut tmax0, true, &mut oid0)
                        })
                        .is_none()
                        .then(|| {
                            c_light = light.get_contrib(ray, &hit_material, hit_point, hit_normal)
                        });
                }
                acc + c_light
            });

            if hit_material.checkered {
                let hit_text2d = hit_obj.unwrap().get_texture_2d(hit_point);
                c = hit_material.do_checker(c, hit_text2d);
            }

            if hit_material.ks > 0.0 {
                stats.num_rays_reflection += 1;
                let reflected_ray = ray.get_reflection(hit_point, hit_normal);
                let c_reflect = self.trace(stats, &reflected_ray, depth + 1);
                let ks = 0.1;
                c = c * (1.0 - ks) + c_reflect * ks;
            }
            c
        } else {
            //let z = (ray.dir.z + 0.5).clamp(0.0, 1.0) as f32;
            let screen_v = self.camera.as_ref().unwrap().screen_v.normalize();
            let s = ray.dir.dot(screen_v).abs() as f32 / ray.dir.norm() as f32;
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

    fn trace_primary_ray(
        &self,
        stats: &mut RenderStats,
        pmap: &mut HashMap<u64, RGB>,
        u: f64,
        v: f64,
    ) -> RGB {
        let mut key = 0;
        if self.cfg.use_hashmap {
            // key = format!("{}-{}", u, v);
            key = ((u + 0.5) * 1000_000_0000_000_f64 + 1000_000_f64 * (v + 0.5)) as u64;
            if self.cfg.use_adaptive_sampling {
                if let Some(c) = pmap.get(&key) {
                    return *c;
                }
            }
        }
        let ray = self.camera.as_ref().unwrap().get_ray(u, v);

        stats.num_rays_sampling += 1;

        let c = self.trace(stats, &ray, 0 /* depth */);
        if self.cfg.use_hashmap && self.cfg.use_adaptive_sampling {
            pmap.insert(key, c);
        }
        c
    }

    /*
     * pos_u: -0.5 .. 0.5
     * pos_v: -0.5 .. 0.5
     */
    fn calc_ray_box(
        &self,
        stats: &mut RenderStats,
        pmap: &mut HashMap<u64, RGB>,
        pos_u: f64,
        pos_v: f64,
        du: f64,
        dv: f64,
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

    fn print_stats(&self, start_time: Instant, stats: RenderStats) {
        let pretty_print = |n| {
            let mut precision = 3;
            let suffix;
            let val;
            if n > 1_000_000_000_000 {
                val = n as f64 / 1_000_000_000_000.0;
                suffix = "T";
            } else if n > 1_000_000_000 {
                val = n as f64 / 1_000_000_000.0;
                suffix = "G";
            } else if n >= 1_000_000 {
                val = n as f64 / 1_000_000.0;
                suffix = "M";
            } else {
                val = n as f64;
                suffix = " ";
                precision = 0
            }
            format!("{:6.precision$} {suffix}", val)
        };
        let elapsed = start_time.elapsed();
        let num_rays = (stats.num_rays_sampling + stats.num_rays_reflection) as f64;
        let tot_lat_str = format!("{:.2} sec", elapsed.as_millis() as f64 / 1000.0);
        let ray_lat_str = format!("{:.3} usec", elapsed.as_micros() as f64 / num_rays as f64);
        let kray_per_secs = num_rays / elapsed.as_secs_f64() / 1_000_f64;
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
        let du = u / self.cfg.res_x as f64;
        let dv = v / self.cfg.res_y as f64;
        let y_max = (y0 + sz_y).min(self.cfg.res_y);
        let x_max = (x0 + sz_x).min(self.cfg.res_x);

        let mut pmap = HashMap::new();

        for y in y0..y_max {
            let pos_v = v / 2.0 - (y as f64) * dv;
            for x in x0..x_max {
                let pos_u = u / 2.0 - (x as f64) * du;
                let c = self.calc_ray_box(stats, &mut pmap, pos_u, pos_v, du, dv, 0);

                self.image.lock().unwrap().push_pixel(x, y, c);
            }
        }
    }

    pub fn render_image_lines(
        &mut self,
        pb: &mut ProgressBar,
        total_stats: &mut Mutex<RenderStats>,
    ) {
        (0..self.cfg.res_y).into_par_iter().for_each(|y| {
            let mut stats = RenderStats::new();

            if crate::CTRLC_HIT.load(Ordering::SeqCst) {
                pb.inc(self.cfg.res_y.into());
                return;
            }
            self.render_pixel_box(0, y, self.cfg.res_x, 1, &mut stats);
            pb.inc(self.cfg.res_y.into());
            total_stats.lock().unwrap().add(stats);
        });
    }

    pub fn render_image_box(&mut self, pb: &mut ProgressBar, total_stats: &mut Mutex<RenderStats>) {
        let step = 32;
        let ny = (self.cfg.res_y + step - 1) / step;
        let nx = (self.cfg.res_x + step - 1) / step;

        (0..ny * nx).into_par_iter().for_each(|v| {
            let mut stats = RenderStats::new();
            let x = (v % nx) * step;
            let y = (v / nx) * step;

            if crate::CTRLC_HIT.load(Ordering::SeqCst) {
                pb.inc((step * step) as u64);
                return;
            }
            self.render_pixel_box(x, y, step, step, &mut stats);
            pb.inc((step * step) as u64);
            total_stats.lock().unwrap().add(stats);
        });
    }

    pub fn render_scene(&mut self) {
        self.image = Mutex::new(Image::new(self.cfg.res_x, self.cfg.res_y));
        let start_time = Instant::now();
        assert!(self.camera.is_some());
        let mut total_stats = Mutex::new(RenderStats::new());
        let mut pb = ProgressBar::new((self.cfg.res_x * self.cfg.res_y) as u64);

        if self.cfg.use_lines {
            self.render_image_lines(&mut pb, &mut total_stats);
        } else {
            self.render_image_box(&mut pb, &mut total_stats);
        }

        pb.finish_and_clear();
        self.print_stats(start_time, *total_stats.lock().unwrap());
    }

    pub fn load_scene(&mut self, scene_file: PathBuf) -> std::io::Result<()> {
        if !scene_file.is_file() {
            println!("file '{}' not found.", scene_file.display());
            println!("pwd={}", std::env::current_dir()?.display());
            panic!("scene file {} not present.", scene_file.display());
        }
        println!(
            "loading scene file {}",
            scene_file.display().to_string().bold()
        );

        let data = fs::read_to_string(&scene_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        let mut num_planes = 0;
        let mut num_spheres = 0;
        let mut num_triangles = 0;
        let mut num_obj_triangles = 0;
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
        let mut smp_str = format!("").cyan();
        if self.cfg.use_adaptive_sampling {
            smp_str = format!(" w/ adaptive sampling").cyan();
        }
        println!("-- img resolution: {}{}", res_str, smp_str);

        let mut camera: Camera = serde_json::from_value(json["camera"].clone()).unwrap();
        camera.aspect = self.cfg.res_x as f64 / self.cfg.res_y as f64;
        camera.init();
        self.camera = Some(camera);

        if !json["ambient"].is_null() {
            let ambient: AmbientLight = serde_json::from_value(json["ambient"].clone()).unwrap();
            self.lights.push(Arc::new(Box::new(ambient)));
        }

        loop {
            let name = format!("material.{}", num_materials);
            if json[&name].is_null() {
                break;
            }
            let mat: Material = serde_json::from_value(json[&name].clone()).unwrap();
            self.materials.push(Arc::new(Box::new(mat)));
            num_materials += 1;
        }
        loop {
            let s = format!("spot-light.{}", num_spot_lights);
            if json[&s].is_null() {
                break;
            }
            let mut spot: SpotLight = serde_json::from_value(json[&s].clone()).unwrap();
            spot.name = s;
            self.lights.push(Arc::new(Box::new(spot)));
            num_spot_lights += 1;
        }
        loop {
            let s = format!("vec-light.{}", num_vec_lights);
            if json[&s].is_null() {
                break;
            }
            let mut vec: VectorLight = serde_json::from_value(json[&s].clone()).unwrap();
            vec.name = s;
            vec.dir = vec.dir.normalize();
            self.lights.push(Arc::new(Box::new(vec)));
            num_vec_lights += 1;
        }

        loop {
            let s = format!("plane.{}", num_planes);
            if json[&s].is_null() {
                break;
            }
            let p: Plane = serde_json::from_value(json[&s].clone()).unwrap();
            self.objects.push(Arc::new(Box::new(p)));
            num_planes += 1;
        }
        loop {
            let s = format!("sphere.{}", num_spheres);
            if json[&s].is_null() {
                break;
            }
            let sphere: Sphere = serde_json::from_value(json[&s].clone()).unwrap();
            self.objects.push(Arc::new(Box::new(sphere)));
            num_spheres += 1;
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
            let angle_x = json[&rxname].as_f64().unwrap();
            let angle_y = json[&ryname].as_f64().unwrap();
            let angle_z = json[&rzname].as_f64().unwrap();
            let angle_x_rad = angle_x.to_radians();
            let angle_y_rad = angle_y.to_radians();
            let angle_z_rad = angle_z.to_radians();

            let mut opt = tobj::LoadOptions::default();
            opt.triangulate = true; // converts polygon into triangles
            opt.ignore_lines = true;
            opt.ignore_points = true;
            let (models, _materials) = tobj::load_obj(&path, &opt).expect("tobj");
            assert!(models.len() == 1);
            models.iter().for_each(|m| {
                let mesh = &m.mesh;
                let n = mesh.indices.len() / 3;
                println!(
                    "-- model has {} triangles w/ {} vertices",
                    n,
                    mesh.positions.len()
                );
                assert!(mesh.indices.len() % 3 == 0);
                num_obj_triangles += n;
                let mut triangles = Vec::with_capacity(n);
                let mut num_skipped = 0;
                for i in 0..n {
                    let i0 = mesh.indices[3 * i + 0] as usize;
                    let i1 = mesh.indices[3 * i + 1] as usize;
                    let i2 = mesh.indices[3 * i + 2] as usize;
                    let x0 = mesh.positions[3 * i0 + 0] as f64;
                    let y0 = mesh.positions[3 * i0 + 1] as f64;
                    let z0 = mesh.positions[3 * i0 + 2] as f64;
                    let x1 = mesh.positions[3 * i1 + 0] as f64;
                    let y1 = mesh.positions[3 * i1 + 1] as f64;
                    let z1 = mesh.positions[3 * i1 + 2] as f64;
                    let x2 = mesh.positions[3 * i2 + 0] as f64;
                    let y2 = mesh.positions[3 * i2 + 1] as f64;
                    let z2 = mesh.positions[3 * i2 + 2] as f64;
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
                self.objects
                    .push(Arc::new(Box::new(Mesh::new(triangles, 0))));
                num_objs += 1;
                println!(
                    "-- loaded {} w/ {} triangles -- rotx={} roty={} rotz={}",
                    path.green(),
                    n,
                    angle_x,
                    angle_y,
                    angle_z
                );
            });
        }
        loop {
            let s = format!("triangle.{}", num_triangles);
            if json[&s].is_null() {
                break;
            }
            let triangle: Triangle = serde_json::from_value(json[&s].clone()).unwrap();
            self.objects.push(Arc::new(Box::new(triangle)));
            num_triangles += 1;
        }
        println!(
            "-- mesh={} triangles={} spheres={} planes={} materials={}",
            num_objs,
            num_triangles + num_obj_triangles,
            num_spheres,
            num_planes,
            num_materials
        );
        self.camera.as_ref().unwrap().display();

        self.lights.iter().for_each(|light| light.display());
        Ok(())
    }

    pub fn save_image(&mut self, img_file: PathBuf) -> std::io::Result<()> {
        return self
            .image
            .lock()
            .unwrap()
            .save_image(PathBuf::from(&img_file), self.cfg.use_gamma);
    }
}
