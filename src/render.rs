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
use wavefront;

use raymax::camera::Camera;
use raymax::color::RGB;
use raymax::image::Image;
use raymax::light::AmbientLight;
use raymax::light::Light;
use raymax::light::SpotLight;
use raymax::light::VectorLight;
use raymax::vec3::Point;
use raymax::Ray;
use raymax::RenderStats;

use raymax::three_d::Material;
use raymax::three_d::Mesh;
use raymax::three_d::Object;
use raymax::three_d::Plane;
use raymax::three_d::Sphere;
use raymax::three_d::Triangle;
use raymax::three_d::EPSILON;

pub struct RenderConfig {
    pub use_adaptive_sampling: bool,
    pub use_reflection: bool,
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
    fn calc_ray_color(&self, stats: &mut RenderStats, ray: Ray, depth: u32) -> RGB {
        if depth > self.cfg.reflection_max_depth {
            return RGB::new();
        }
        let mut s_id: usize = 0;
        let mut t = f64::MAX;
        let mut tmin = EPSILON;
        if depth == 0 {
            // look for intersection beyond the screen/sensor
            tmin = ray.dir.norm();
        }

        let hit_obj = self
            .objects
            .iter()
            .filter(|obj| obj.intercept(stats, &ray, tmin, &mut t, false, &mut s_id))
            .last();

        if hit_obj.is_some() {
            let hit_point = ray.orig + ray.dir * t;
            let hit_normal = hit_obj.unwrap().get_normal(hit_point, s_id);
            let hit_mat_id = hit_obj.unwrap().get_material_id();
            let hit_material = &self.materials[hit_mat_id];

            let mut c = self.lights.iter().fold(RGB::new(), |acc, light| {
                let c_light;

                if !light.is_spot() {
                    c_light = light.get_contrib(&hit_material, hit_point, hit_normal);
                } else {
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let light_ray = Ray {
                        orig: hit_point,
                        dir: light_vec,
                    };
                    let mut t0 = 1.0;
                    let mut oid0: usize = 0;
                    let shadow = self
                        .objects
                        .iter()
                        .find(|obj| {
                            obj.intercept(stats, &light_ray, EPSILON, &mut t0, true, &mut oid0)
                        })
                        .is_some();

                    if shadow {
                        c_light = RGB::new();
                    } else {
                        c_light = light.get_contrib(&hit_material, hit_point, hit_normal);
                    }
                }
                acc + c_light * hit_material.albedo
            });

            if hit_material.checkered {
                let hit_text2d = hit_obj.unwrap().get_texture_2d(hit_point);
                c = hit_material.do_checker(c, hit_text2d);
            }

            if self.cfg.use_reflection && hit_material.reflectivity > 0.0 {
                stats.num_rays_reflection += 1;
                let reflected_ray = ray.get_reflection(hit_point, hit_normal);
                let c_reflect = self.calc_ray_color(stats, reflected_ray, depth + 1);
                c = c * (1.0 - hit_material.reflectivity) + c_reflect * hit_material.reflectivity;
            }
            c
        } else {
            let z = (ray.dir.z + 0.5).clamp(0.0, 1.0) as f32;
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
            cmax * (1.0 - z) + cyan * z
        }
    }

    fn calc_one_ray(
        &self,
        stats: &mut RenderStats,
        pmap: &mut HashMap<String, RGB>,
        u: f64,
        v: f64,
    ) -> RGB {
        if self.cfg.use_adaptive_sampling {
            let key = format!("{}-{}", u, v);
            if let Some(c) = pmap.get(&key) {
                return *c;
            }
        }
        let ray = self.camera.as_ref().unwrap().get_ray(u, v);

        stats.num_rays_sampling += 1;

        let c = self.calc_ray_color(stats, ray, 0);
        if self.cfg.use_adaptive_sampling {
            let key = format!("{}-{}", u, v);
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
        pmap: &mut HashMap<String, RGB>,
        pos_u: f64,
        pos_v: f64,
        du: f64,
        dv: f64,
        lvl: u32,
    ) -> RGB {
        if !self.cfg.use_adaptive_sampling {
            return self.calc_one_ray(stats, pmap, pos_u + du / 2.0, pos_v + dv / 2.0);
        }
        let mut c00 = self.calc_one_ray(stats, pmap, pos_u, pos_v);
        let mut c01 = self.calc_one_ray(stats, pmap, pos_u, pos_v + dv);
        let mut c10 = self.calc_one_ray(stats, pmap, pos_u + du, pos_v);
        let mut c11 = self.calc_one_ray(stats, pmap, pos_u + du, pos_v + dv);

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
            stats.num_rays_hit_max_level += 1;
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
                suffix = "";
                precision = 0
            }
            format!("{:6.precision$} {suffix}", val)
        };
        let elapsed = start_time.elapsed();
        let num_rays = (stats.num_rays_sampling + stats.num_rays_reflection) as f64;
        let tot_lat_str = format!("{:.2} sec", elapsed.as_millis() as f64 / 1000.0);
        let ray_lat_str = format!("{:.3} usec", elapsed.as_micros() as f64 / num_rays as f64);
        let kray_sec_str = format!("{:.3}", num_rays / elapsed.as_secs_f64() / 1_000_f64);
        println!(
            "duration: {} -- {} per ray -- {} Krays/sec",
            tot_lat_str.bold(),
            ray_lat_str.bold(),
            kray_sec_str.bold()
        );
        println!(
            "num_intersects Sphere:   {:>12}",
            pretty_print(stats.num_intersects_sphere)
        );
        println!(
            "num_intersects Plane:    {:>12}",
            pretty_print(stats.num_intersects_plane)
        );
        println!(
            "num_intersects Triangle: {:>12}",
            pretty_print(stats.num_intersects_triangle)
        );

        let num_pixels = (self.cfg.res_x * self.cfg.res_y) as u64;
        println!(
            "num_rays_sampling:       {:>12} -- {:3}%",
            pretty_print(stats.num_rays_sampling),
            100 * stats.num_rays_sampling / num_pixels
        );
        println!(
            "num_rays_reflection:     {:>12} -- {:3}%",
            pretty_print(stats.num_rays_reflection),
            100 * stats.num_rays_reflection / stats.num_rays_sampling
        );
        println!(
            "num_rays_max_level:      {:>12} -- {:3}%",
            pretty_print(stats.num_rays_hit_max_level),
            100 * stats.num_rays_hit_max_level / stats.num_rays_sampling
        );
    }

    fn render_pixel_box(&self, x0: u32, y0: u32, sz: u32, stats: &mut RenderStats) {
        let u = 1.0;
        let v = 1.0;
        let du = u / self.cfg.res_x as f64;
        let dv = v / self.cfg.res_y as f64;
        let y_max = (y0 + sz).min(self.cfg.res_y);
        let x_max = (x0 + sz).min(self.cfg.res_x);

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

    pub fn render_scene(&mut self) {
        self.image = Mutex::new(Image::new(self.cfg.res_x, self.cfg.res_y));
        let start_time = Instant::now();
        assert!(self.camera.is_some());

        let step = 32;
        let ny = (self.cfg.res_y + step - 1) / step;
        let nx = (self.cfg.res_x + step - 1) / step;
        let pb = ProgressBar::new((nx * ny) as u64);

        let total_stats = Mutex::new(RenderStats::new());

        (0..ny * nx).into_par_iter().for_each(|v| {
            let mut stats = RenderStats::new();
            let x = (v % nx) * step;
            let y = (v / nx) * step;

            if crate::CTRLC_HIT.load(Ordering::SeqCst) {
                pb.inc(1);
                return;
            }
            self.render_pixel_box(x, y, step, &mut stats);
            pb.inc(1);
            total_stats.lock().unwrap().add(stats);
        });

        pb.finish_and_clear();
        self.print_stats(start_time, *total_stats.lock().unwrap());
    }

    pub fn load_scene(&mut self, scene_file: PathBuf) -> std::io::Result<()> {
        if !scene_file.is_file() {
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
        camera.calc_uv_after_deserialize();
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
            let model = wavefront::Obj::from_file(path).unwrap();
            let mut angle_x = 0.0;
            let mut angle_y = 0.0;
            let mut angle_z = 0.0;
            let rname = format!("obj.{}.rotx", num_objs);
            if let Some(v) = json[&rname].as_f64() {
                angle_x = v;
            }
            let rname = format!("obj.{}.roty", num_objs);
            if let Some(v) = json[&rname].as_f64() {
                angle_y = v;
            }
            let rname = format!("obj.{}.rotz", num_objs);
            if let Some(v) = json[&rname].as_f64() {
                angle_z = v;
            }
            let n = model.triangles().count();
            num_obj_triangles += n;
            let mut triangles = Vec::with_capacity(n);
            for [a, b, c] in model.triangles() {
                let a0 = a.position();
                let b0 = b.position();
                let c0 = c.position();
                let mut p0 = Point {
                    x: a0[0] as f64,
                    y: a0[1] as f64,
                    z: a0[2] as f64,
                };
                let mut p1 = Point {
                    x: b0[0] as f64,
                    y: b0[1] as f64,
                    z: b0[2] as f64,
                };
                let mut p2 = Point {
                    x: c0[0] as f64,
                    y: c0[1] as f64,
                    z: c0[2] as f64,
                };
                p0 = p0.rotx(angle_x).roty(angle_y).rotz(angle_z);
                p1 = p1.rotx(angle_x).roty(angle_y).rotz(angle_z);
                p2 = p2.rotx(angle_x).roty(angle_y).rotz(angle_z);
                let mut triangle = Triangle::new([p0, p1, p2], 0);
                triangle.calc_normal();
                triangles.push(triangle);
            }
            let mut id = 0;
            triangles.iter_mut().for_each(|t| {
                t.mesh_id = id;
                id += 1;
            });
            let mesh = Mesh::new(triangles, 0);
            self.objects.push(Arc::new(Box::new(mesh)));
            num_objs += 1;
            println!(
                "-- loaded {} w/ {} triangles -- rotx={} roty={} rotz={}",
                path.green(),
                n,
                angle_x,
                angle_y,
                angle_z
            );
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
            "-- materials={} vec_lights={} spot_lights={}",
            num_materials, num_vec_lights, num_vec_lights
        );
        println!(
            "-- {} surfaces: mesh={} triangles={} spheres={} planes={}",
            self.objects.len(),
            num_objs,
            num_triangles + num_obj_triangles,
            num_spheres,
            num_planes
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
