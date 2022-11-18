use std::sync::atomic::Ordering;
use wavefront;
use colored::Colorize;
use serde_json;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use indicatif::ProgressBar;
use rayon::prelude::*;

use raymax::color::RGB;
use raymax::vec3::Vec3;
use raymax::vec3::Point;
use raymax::vec3::Vec2;
use raymax::light::Light;
use raymax::light::VectorLight;
use raymax::light::SpotLight;
use raymax::light::AmbientLight;
use raymax::camera::Camera;
use raymax::image::Image;
use raymax::Ray;

use raymax::three_d::Material;
use raymax::three_d::Object;
use raymax::three_d::Sphere;
use raymax::three_d::Triangle;
use raymax::three_d::Plane;

#[derive(Clone, Copy)]
struct RenderStats {
    num_rays_sampling: u64,
    num_rays_reflection: u64,
    num_rays_hit_max_level: u64,
    num_intersects_plane: u64,
    num_intersects_sphere: u64,
    num_intersects_triangle: u64,
}

impl RenderStats {
    fn new() -> RenderStats {
        RenderStats {
            num_rays_sampling: 0,
            num_rays_reflection: 0,
            num_rays_hit_max_level: 0,
            num_intersects_plane: 0,
            num_intersects_sphere: 0,
            num_intersects_triangle: 0,
        }
    }
    fn intersect_obj(&mut self, is_sphere: bool, is_triangle: bool) {
        if is_triangle {
            self.num_intersects_triangle += 1;
        } else if is_sphere {
            self.num_intersects_sphere += 1;
        } else {
            self.num_intersects_plane += 1;
        }
    }
    fn add(&mut self, other: RenderStats) {
        self.num_rays_sampling       += other.num_rays_sampling;
        self.num_rays_reflection     += other.num_rays_reflection;
        self.num_rays_hit_max_level  += other.num_rays_hit_max_level;
        self.num_intersects_sphere   += other.num_intersects_sphere;
        self.num_intersects_plane    += other.num_intersects_plane;
        self.num_intersects_triangle += other.num_intersects_triangle;
    }
}

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
        let mut t = f64::MAX;
        let mut tmin = 0.0001;
        if depth == 0 {
            tmin = ray.dir.norm();
        }

        let hit_obj = self.objects.iter().filter(|obj| {
            stats.intersect_obj(obj.is_sphere(), obj.is_triangle());
            obj.intercept(&ray, tmin, &mut t)
        }).fold(None, |_acc, obj| Some(obj));

        if hit_obj.is_some() {
            let hit_point = ray.orig + ray.dir * t;
            let hit_normal = hit_obj.clone().unwrap().get_normal(hit_point);
            let hit_mat_id = hit_obj.clone().unwrap().get_material_id();
            let hit_material = &self.materials[hit_mat_id as usize];
            let mut hit_text2d = Vec2::new();
            if hit_material.checkered {
                hit_text2d = hit_obj.clone().unwrap().get_texture_2d(hit_point);
            }
            let mut c = self.lights.iter().fold(RGB::new(), |acc, light| {
                let c_light;

                if ! light.is_spot() {
                    c_light = light.get_contrib(&hit_material, hit_point, hit_normal);
                } else {
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let light_ray = Ray{orig: hit_point, dir: light_vec};
                    let mut t = 1.0;
                    let shadow = self.objects.iter().find(|obj| obj.intercept(&light_ray, 0.0001, &mut t)).is_some();

                    if shadow {
                        c_light = RGB::new();
                    } else {
                        c_light = light.get_contrib(&hit_material, hit_point, hit_normal);
                    }
                }
                acc + c_light * hit_material.albedo
            });

            c = hit_material.do_checker(c, hit_text2d);

            if self.cfg.use_reflection && hit_material.reflectivity > 0.0 {
                stats.num_rays_reflection += 1;
                let reflected_ray = ray.get_reflection(hit_point, hit_normal);
                let c_reflect = self.calc_ray_color(stats, reflected_ray, depth + 1);
                c = c * (1.0 - hit_material.reflectivity) + c_reflect * hit_material.reflectivity;
            }
            c
        } else {
	    let z = (ray.dir.z + 0.5).clamp(0.0, 1.0) as f32;
	    let cmax = RGB{ r: 1.0, g: 1.0, b: 1.0 };
	    let cyan = RGB{ r: 0.4, g: 0.6, b: 0.9 };
            cmax * (1.0 - z) + cyan * z
        }
    }

    fn calc_one_ray(&self, stats: &mut RenderStats, pmap: &mut HashMap<String,RGB>, u: f64, v: f64) -> RGB {
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
    fn calc_ray_box(&self, stats: &mut RenderStats, pmap: &mut HashMap<String,RGB>, pos_u: f64, pos_v: f64, du: f64, dv: f64, lvl: u32) -> RGB {
        if ! self.cfg.use_adaptive_sampling {
            return self.calc_one_ray(stats, pmap, pos_u + du / 2.0, pos_v + dv / 2.0);
        }
        let mut c00 = self.calc_one_ray(stats, pmap, pos_u,      pos_v);
        let mut c01 = self.calc_one_ray(stats, pmap, pos_u,      pos_v + dv);
        let mut c10 = self.calc_one_ray(stats, pmap, pos_u + du, pos_v);
        let mut c11 = self.calc_one_ray(stats, pmap, pos_u + du, pos_v + dv);

        if lvl < self.cfg.adaptive_max_depth {
            let color_diff = RGB::difference(c00, c01, c10, c11) > 0.3;
            if color_diff {
                let du2 = du / 2.0;
                let dv2 = dv / 2.0;
                c00 = self.calc_ray_box(stats, pmap, pos_u,       pos_v,       du2, dv2, lvl + 1);
                c01 = self.calc_ray_box(stats, pmap, pos_u,       pos_v + dv2, du2, dv2, lvl + 1);
                c10 = self.calc_ray_box(stats, pmap, pos_u + du2, pos_v,       du2, dv2, lvl + 1);
                c11 = self.calc_ray_box(stats, pmap, pos_u + du2, pos_v + dv2, du2, dv2, lvl + 1);
            }
        } else {
            stats.num_rays_hit_max_level += 1;
        }
        (c00 + c01 + c10 + c11) * 0.25
    }

    fn print_stats(&self, start_time: Instant, stats: RenderStats) {
        let elapsed = start_time.elapsed();
        let tot_lat_str = format!("{:.2} sec", elapsed.as_millis() as f64 / 1000.0);
        let ray_lat_str = format!("{:.3} usec", elapsed.as_micros() as f64 / (stats.num_rays_sampling + stats.num_rays_reflection) as f64);
        println!("duration: {} -- {} per ray", tot_lat_str.bold(), ray_lat_str.bold());
        println!("num_intersects Sphere:   {:14}", stats.num_intersects_sphere);
        println!("num_intersects Plane:    {:14}", stats.num_intersects_plane);
        println!("num_intersects Triangle: {:14}", stats.num_intersects_triangle);

        let num_pixels = (self.cfg.res_x * self.cfg.res_y) as u64;
        println!("num_rays_sampling:   {:12} {}%", stats.num_rays_sampling, 100 * stats.num_rays_sampling / num_pixels);
        println!("num_rays_reflection: {:12} {}%", stats.num_rays_reflection, 100 * stats.num_rays_reflection / stats.num_rays_sampling);
        println!("num_rays_max_level:  {:12} {}%", stats.num_rays_hit_max_level, 100 * stats.num_rays_hit_max_level / stats.num_rays_sampling);
    }

    fn render_pixel_box(&self, x0: u32, y0: u32, nx: u32, ny: u32, stats: &mut RenderStats) {
        let u = 1.0;
        let v = 1.0;
        let du = u / self.cfg.res_x as f64;
        let dv = v / self.cfg.res_y as f64;
        let y_max = (y0 + ny).min(self.cfg.res_y);
        let x_max = (x0 + nx).min(self.cfg.res_x);

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

        let step = 64;
        let ny = (self.cfg.res_y + step - 1) / step;
        let nx = (self.cfg.res_x + step - 1) / step;
        let pb = ProgressBar::new((nx * ny) as u64);

        let total_stats = Mutex::new(RenderStats::new());

        (0..ny*nx).into_par_iter().for_each(|v| {
            let mut stats = RenderStats::new();
            let x = (v % nx) * step;
            let y = (v / nx) * step;

            if crate::CTRLC_HIT.load(Ordering::SeqCst) {
                pb.inc(1);
                return;
            }
            self.render_pixel_box(x, y, step, step, &mut stats);
            pb.inc(1);
            total_stats.lock().unwrap().add(stats);
        });

        pb.finish_and_clear();
        self.print_stats(start_time, *total_stats.lock().unwrap());
    }

    pub fn load_scene(&mut self, scene_file: PathBuf) -> std::io::Result<()> {
        if ! scene_file.is_file() {
             panic!("scene file {} not present.", scene_file.display());
        }
        println!("loading scene file {}", scene_file.display().to_string().bold());

        let data = fs::read_to_string(&scene_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        let num_planes;
        let num_spheres;
        let num_triangles;
        let mut num_obj_triangles = 0;

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

        let mut camera : Camera = serde_json::from_value(json["camera"].clone()).unwrap();
        camera.calc_uv_after_deserialize();
        self.camera = Some(camera);

        let ambient : AmbientLight = serde_json::from_value(json["ambient"].clone()).unwrap();
        self.lights.push(Arc::new(Box::new(ambient)));

        {
            let mut i = 0;
            loop {
                let name = format!("material.{}", i);
                if json[&name].is_null() {
                    break;
                }
                let mat : Material = serde_json::from_value(json[&name].clone()).unwrap();
                self.materials.push(Arc::new(Box::new(mat)));
                i += 1;
            }
            println!("-- {} materials", i);
        }
        {
            let num_spot_lights = json["num_spot_lights"].as_u64().unwrap();
            for i in 0..num_spot_lights {
                let s = format!("spot-light.{}", i);
                let spot : SpotLight = serde_json::from_value(json[&s].clone()).unwrap();
                self.lights.push(Arc::new(Box::new(spot)));
            }
        }
        {
            let num_vec_lights = json["num_vec_lights"].as_u64().unwrap();
            for i in 0..num_vec_lights {
                let s = format!("vec-light.{}", i);
                let mut vec : VectorLight = serde_json::from_value(json[&s].clone()).unwrap();
                vec.dir = vec.dir.normalize();
                self.lights.push(Arc::new(Box::new(vec)));
            }
        }

        {
            num_planes = json["num_planes"].as_u64().unwrap();
            for i in 0..num_planes {
                let s = format!("plane.{}", i);
                let p : Plane = serde_json::from_value(json[&s].clone()).unwrap();
                self.objects.push(Arc::new(Box::new(p)));
            }
        }
        {
            num_spheres = json["num_spheres"].as_u64().unwrap();
            for i in 0..num_spheres {
                let s = format!("sphere.{}", i);
                let sphere : Sphere = serde_json::from_value(json[&s].clone()).unwrap();
                self.objects.push(Arc::new(Box::new(sphere)));
            }
        }
        {
            let num_objs = json["num_objs"].as_u64().unwrap();
            for i in 0..num_objs {
                let name    = format!("obj.{}.path", i);
                let path = json[&name].as_str().unwrap();
                let model = wavefront::Obj::from_file(path).unwrap();
                let mut angle_x = 0.0;
                let mut angle_y = 0.0;
                let mut angle_z = 0.0;
                let rname = format!("obj.{}.rotx", i);
                if let Some(v) = json[&rname].as_f64() {
                    angle_x = v;
                }
                let rname = format!("obj.{}.roty", i);
                if let Some(v) = json[&rname].as_f64() {
                    angle_y = v;
                }
                let rname = format!("obj.{}.rotz", i);
                if let Some(v) = json[&rname].as_f64() {
                    angle_z = v;
                }
                let path = json[&name].as_str().unwrap();
                let mut n = 0;
                for [a, b, c] in model.triangles() {
                    let a0 = a.position();
                    let b0 = b.position();
                    let c0 = c.position();
                    let mut p0 = Point{ x: a0[0] as f64, y: a0[1] as f64, z: a0[2] as f64};
                    let mut p1 = Point{ x: b0[0] as f64, y: b0[1] as f64, z: b0[2] as f64};
                    let mut p2 = Point{ x: c0[0] as f64, y: c0[1] as f64, z: c0[2] as f64};
                    p0 = p0.rotx(angle_x).roty(angle_y).rotz(angle_z);
                    p1 = p1.rotx(angle_x).roty(angle_y).rotz(angle_z);
                    p2 = p2.rotx(angle_x).roty(angle_y).rotz(angle_z);
                    n += 1;
                    let mut triangle = Triangle {
                        points: [ p0, p1, p2 ],
                        material_id: 0, // XXX
                        has_normal: false, normal: Vec3::new(),
                    };
                    triangle.calc_normal();
                    num_obj_triangles += 1;
                    self.objects.push(Arc::new(Box::new(triangle)));
                }
                println!("-- loaded {} w/ {} triangles -- rotx={} roty={} rotz={}",
                        path.green(), n, angle_x, angle_y, angle_z);
            }
        }
        {
            num_triangles = json["num_triangles"].as_u64().unwrap();
            for i in 0..num_triangles {
                let s = format!("t{}", i);
                let triangle : Triangle = serde_json::from_value(json[&s].clone()).unwrap();
                self.objects.push(Arc::new(Box::new(triangle)));
            }
        }
        println!("-- {} objects: num_triangles={} num_spheres={} num_planes={}", self.objects.len(), num_triangles + num_obj_triangles, num_spheres, num_planes);
        self.camera.as_ref().unwrap().display();

        self.lights.iter().for_each(|light| light.display());
        Ok(())
    }

    pub fn save_image(&mut self, img_file: PathBuf) -> std::io::Result<()> {
        return self.image.lock().unwrap().save_image(PathBuf::from(&img_file), self.cfg.use_gamma);
    }
}
