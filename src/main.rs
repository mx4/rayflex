use structopt::StructOpt;
use std::sync::atomic::{AtomicBool, Ordering};
use serde_json;
use rand::Rng;
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
use raymax::light::Light;
use raymax::light::VectorLight;
use raymax::light::SpotLight;
use raymax::light::AmbientLight;
use raymax::camera::Camera;
use raymax::image::Image;
use raymax::Ray;

mod three_d;

use three_d::Material;
use three_d::Object;
use three_d::Sphere;
use three_d::Plane;

static CTRLC_HIT : AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct RenderStats {
    num_rays_sampling: u64,
    num_rays_reflection: u64,
    num_rays_hit_max_level: u64,
    num_intersects_plane: u64,
    num_intersects_sphere: u64,
}

impl RenderStats {
    fn new() -> RenderStats {
        RenderStats {
            num_rays_sampling: 0,
            num_rays_reflection: 0,
            num_rays_hit_max_level: 0,
            num_intersects_plane: 0,
            num_intersects_sphere: 0,
        }
    }
    fn add(&mut self, other: RenderStats) {
        self.num_rays_sampling      = self.num_rays_sampling + other.num_rays_sampling;
        self.num_rays_reflection    = self.num_rays_reflection + other.num_rays_reflection;
        self.num_rays_hit_max_level = self.num_rays_hit_max_level + other.num_rays_hit_max_level;
        self.num_intersects_sphere  = self.num_intersects_sphere + other.num_intersects_sphere;
        self.num_intersects_plane   = self.num_intersects_plane + other.num_intersects_plane;
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name="rtest", about="minimal raytracer")]
struct Options {
    #[structopt(long, default_value = "pic.png")]
    img_file: PathBuf,
    #[structopt(long, default_value = "scene.json")]
    scene_file: PathBuf,
     #[structopt(short="x", long, default_value = "0")]
    res_x: u32,
     #[structopt(short="y", long, default_value = "0")]
    res_y: u32,
     #[structopt(short="n", long, default_value = "0")]
    num_spheres_to_generate: u32,
     #[structopt(short="a", long, default_value = "0")]
    adaptive_sampling: u8,
     #[structopt(long, default_value = "2")]
    adaptive_max_depth: u32,
     #[structopt(long, default_value = "4")]
    reflection_max_depth: u32,
     #[structopt(short="r", long, default_value = "1")]
    use_reflection: u32,
     #[structopt(short="g", long, default_value = "0")]
    use_gamma: u32,
     #[structopt(short="b", long, default_value = "1")]
    use_box: u32,
}


struct RenderJob {
    opt: Options,
    camera: Option<Camera>,
    objects: Vec<Arc<Box<dyn Object + 'static + Send + Sync>>>,
    lights: Vec<Arc<Box<dyn Light + 'static + Send + Sync>>>,
    image: Mutex<Image>,
}


impl RenderJob {
    pub fn new(opt: Options) -> Self {
        Self {
            camera: None,
            image: Mutex::new(Image::new(0, 0)),
            objects: vec![],
            lights: vec![],
            opt : opt,
        }
    }
    fn calc_ray_color(&self, stats: &mut RenderStats, ray: Ray, view_all: bool, depth: u32) -> RGB {
        let mut c = RGB::new();
        if depth > self.opt.reflection_max_depth {
            return c
        }
        let mut hit_point = Point::new();
        let mut hit_normal = Vec3::new();
        let mut hit_material = Material::new();
        let (mut hitx, mut hity) : (f64,f64) = (0.0, 0.0);
        let mut raylen = ray.dir.norm();
        if view_all {
            raylen = 0.0001;
        }

        let mut t = f64::MAX;
        for obj in &self.objects {
            if obj.intercept(&ray, raylen, &mut t) {
                if obj.is_sphere() {
                    stats.num_intersects_sphere += 1;
                } else {
                    stats.num_intersects_plane += 1;
                }
                hit_point = ray.orig + ray.dir * t;
                hit_normal = obj.get_normal(hit_point);
                hit_material = obj.get_material();
                if hit_material.checkered {
                    (hitx, hity) = obj.get_texture_2d(hit_point);
                }
            }
        }
        if t < f64::MAX {
            for light in &self.lights {
                let light_intensity = light.get_intensity();
                let light_rgb = light.get_color();
                let mut c_light = RGB::new();
                let mut c_res = RGB{
                    r : hit_material.rgb.r * light_rgb.r,
                    g : hit_material.rgb.g * light_rgb.g,
                    b : hit_material.rgb.b * light_rgb.b,
                };
                c_res = c_res * light_intensity;

                if light.is_ambient() {
                    c_light = c_res;
                } else if light.is_vector() {
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let mut v_prod = hit_normal.dot(light_vec) as f32;
                    if v_prod > 0.0 { // only show visible side
                        v_prod = 0.0;
                    }
                    let v = v_prod.powi(4);
                    c_light = c_res * v;
                } else {
                    assert!(light.is_spot());
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let light_vec_norm = light_vec.normalize();
                    let mut shadow = false;
                    let light_ray = Ray{orig: hit_point, dir: light_vec};
                    let mut t : f64 = 1.0;
                    for obj in &self.objects {
                        if obj.intercept(&light_ray, 0.001, &mut t) {
                            shadow = true;
                            break;
                        }
                    }
                    if !shadow {
                        let dist_sq = light_vec.dot(light_vec) as f32;
                        let mut v_prod = hit_normal.dot(light_vec_norm) as f32;
                        if v_prod < 0.0 { // only show visible side
                            v_prod = 0.0;
                        }
                        let v = v_prod.powi(4) / (1.0 + 4.0 * std::f32::consts::PI * dist_sq);
                        assert!(v >= 0.0);
                        c_light = c_res * v ;
                    }
                }
                if hit_material.checkered {
                    let pattern = ((hitx * 4.0).fract() > 0.5) ^ ((hity * 4.0).fract() > 0.5);
                    if pattern {
                        c_light = c_light * (1.0 / 3.0);
                    }
                }
                assert!(hit_material.albedo >= 0.0);
                c += c_light * hit_material.albedo;
            }
            if self.opt.use_reflection > 0 && hit_material.reflectivity > 0.0 {
                stats.num_rays_reflection += 1;
                let reflected_vec = ray.dir.reflect(hit_normal);
                let reflected_ray = Ray{orig: hit_point, dir: reflected_vec};
                let c_reflect = self.calc_ray_color(stats, reflected_ray, true, depth + 1);
                c = c * (1.0 - hit_material.reflectivity) + c_reflect * hit_material.reflectivity;
            }
        } else {
	    let mut z = (ray.dir.z + 0.5) as f32;
            z = z.clamp(0.0, 1.0);
	    let cmax = RGB{ r: 1.0, g: 1.0, b: 1.0 };
	    let cyan = RGB{ r: 0.4, g: 0.6, b: 0.9 };
            assert!(z >= 0.0);
            assert!(z <= 1.0);
            c = cmax * (1.0 - z) + cyan * z;
        }
        c
    }

    fn color_difference(c00: RGB, c01: RGB, c10: RGB, c11: RGB) -> bool {
        let avg = (c00 + c01 + c10 + c11) * 0.25;
        let d = avg.distance2(c00) + avg.distance2(c01) + avg.distance2(c10) + avg.distance2(c11);
        d > 0.3
        //let d = avg.distance(c00) + avg.distance(c01) + avg.distance(c10) + avg.distance(c11);
        //d > 0.5
    }

    fn calc_one_ray(&self, stats: &mut RenderStats, pmap: &mut HashMap<String,RGB>, u: f64, v: f64) -> RGB {
        if self.opt.adaptive_sampling != 0 {
            let key = format!("{}-{}", u, v);
            if let Some(c) = pmap.get(&key) {
                return *c;
            }
        }
        let ray = self.camera.as_ref().unwrap().get_ray(u, v);

        stats.num_rays_sampling += 1;

        let c = self.calc_ray_color(stats, ray, false, 0);
        if self.opt.adaptive_sampling != 0 {
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
        if self.opt.adaptive_sampling == 0 {
            return self.calc_one_ray(stats, pmap, pos_u + du / 2.0, pos_v + dv / 2.0);
        }
        let mut c00 = self.calc_one_ray(stats, pmap, pos_u,      pos_v);
        let mut c01 = self.calc_one_ray(stats, pmap, pos_u,      pos_v + dv);
        let mut c10 = self.calc_one_ray(stats, pmap, pos_u + du, pos_v);
        let mut c11 = self.calc_one_ray(stats, pmap, pos_u + du, pos_v + dv);

        if lvl < self.opt.adaptive_max_depth {
            let color_diff = Self::color_difference(c00, c01, c10, c11);
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
        println!("duration: {} sec", elapsed.as_millis() as f64 / 1000.0);
        println!("num_intersects Sphere: {:10}", stats.num_intersects_sphere);
        println!("num_intersects Plane:  {:10}", stats.num_intersects_plane);

        let num_pixels = (self.opt.res_x * self.opt.res_y) as u64;
        let num_rays_sampling = stats.num_rays_sampling;
        let num_rays_reflection = stats.num_rays_reflection;
        let num_rays_hit_max_level = stats.num_rays_hit_max_level;
        println!("num_rays_sampling:   {:12} {}%", num_rays_sampling, 100 * num_rays_sampling / num_pixels);
        println!("num_rays_reflection: {:12} {}%", num_rays_reflection, 100 * num_rays_reflection / num_rays_sampling);
        println!("num_rays_max_level:  {:12} {}%", num_rays_hit_max_level, 100 * num_rays_hit_max_level / num_rays_sampling);
        println!("{:.2} usec per ray", elapsed.as_micros() as f64 / (num_rays_sampling + num_rays_reflection) as f64);
    }

    fn render_pixel_box(&self, x0: u32, y0: u32, nx: u32, ny: u32, stats: &mut RenderStats) {
        let u = 1.0;
        let v = 1.0;
        let du = u / self.opt.res_x as f64;
        let dv = v / self.opt.res_y as f64;
        let y_max = std::cmp::min(y0 + ny, self.opt.res_y);
        let x_max = std::cmp::min(x0 + nx, self.opt.res_x);

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
        self.image = Mutex::new(Image::new(self.opt.res_x, self.opt.res_y));
        let start_time = Instant::now();
        assert!(self.camera.is_some());

        let step = 64;
        let ny = (self.opt.res_y + step - 1) / step;
        let nx = (self.opt.res_x + step - 1) / step;
        let pb = ProgressBar::new((nx * ny) as u64);

        let total_stats = Mutex::new(RenderStats::new());

        (0..ny*nx).into_par_iter().for_each(|v| {
            let mut stats = RenderStats::new();
            let x = (v % nx) * step;
            let y = (v / nx) * step;

            if CTRLC_HIT.load(Ordering::SeqCst) {
                pb.inc(1);
                return;
            }
            self.render_pixel_box(x, y, step, step, &mut stats);
            pb.inc(1);
            total_stats.lock().unwrap().add(stats);
        });

        pb.finish_with_message("done");
        self.print_stats(start_time, *total_stats.lock().unwrap());
    }

    fn get_json_reflectivity(json: &serde_json::Value, key: String) -> f32 {
        let mut r : f32 = 0.0;
        if let Some(v) = json[&key].as_f64() {
            r = v as f32;
        }
        r
    }
    fn get_json_checkered(json: &serde_json::Value, key: String) -> bool {
        let mut checkered = false;
        if let Some(v) = json[&key].as_bool() {
            checkered = v;
        }
        checkered
    }
    fn get_json_albedo(json: &serde_json::Value, key: String) -> f32 {
        let mut albedo: f32 = 1.0;
        if let Some(v) = json[&key].as_f64() {
            albedo = v as f32;
        }
        assert!(albedo >= 0.0);
        albedo
    }
    fn get_json_color(json: &serde_json::Value, key: String) -> RGB {
        let mut cr : f32 = 1.0;
        let mut cg : f32 = 1.0;
        let mut cb : f32 = 1.0;
        if let Some(array) = json[&key].as_array() {
            cr = array[0].as_f64().unwrap() as f32;
            cg = array[1].as_f64().unwrap() as f32;
            cb = array[2].as_f64().unwrap() as f32;
        }
        assert!(cr >= 0.0);
        assert!(cg >= 0.0);
        assert!(cb >= 0.0);
        RGB{ r: cr, g: cg, b: cb }
    }
    fn get_json_vec3(json: &serde_json::Value, key: String) -> Vec3 {
        Point {
            x: json[&key][0].as_f64().unwrap(),
            y: json[&key][1].as_f64().unwrap(),
            z: json[&key][2].as_f64().unwrap()
        }
    }
    pub fn load_scene(&mut self) -> std::io::Result<()> {
        if ! self.opt.scene_file.is_file() {
            panic!("scene file {} not present.", self.opt.scene_file.display());
        }
        println!("Loading scene file..");

        let data = fs::read_to_string(&self.opt.scene_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        let num_planes;
        let num_spheres;

        if self.opt.res_x == 0 && self.opt.res_y == 0 {
            if let Some(array) = json[&"resolution".to_string()].as_array() {
                self.opt.res_x = array[0].as_u64().unwrap() as u32;
                self.opt.res_y = array[1].as_u64().unwrap() as u32;
            }
        }
        {
            let p = Self::get_json_vec3(&json, "camera.position".to_string());
            let v = Self::get_json_vec3(&json, "camera.direction".to_string());
            self.camera = Some(Camera::new(p, v));
        }
        {
            let name = "ambient.color";
            let c = Self::get_json_color(&json, name.to_string());
            let name = "ambient.intensity";
            let r = json[&name].as_f64().unwrap() as f32;
            assert!(r >= 0.0);
            self.lights.push(Arc::new(Box::new(AmbientLight{ name: name.to_string(), rgb: c, intensity: r })));
        }
        {
            let num_spot_lights = json["num_spot_lights"].as_u64().unwrap();
            for i in 0..num_spot_lights {
                let name  = format!("spot-light.{}.position", i);
                let cname = format!("spot-light.{}.color", i);
                let iname = format!("spot-light.{}.intensity", i);
                let sname = format!("spot-light.{}", i);
                let c = Self::get_json_color(&json, cname);
                let p = Self::get_json_vec3(&json, name);
                let i = json[&iname].as_f64().unwrap() as f32;
                assert!(i >= 0.0);
                self.lights.push(Arc::new(Box::new(SpotLight{ name: sname, pos: p, rgb: c, intensity: i })));
            }
        }
        {
            let num_vec_lights = json["num_vec_lights"].as_u64().unwrap();
            for i in 0..num_vec_lights {
                let name  = format!("vec-light.{}.vector", i);
                let iname = format!("vec-light.{}.intensity", i);
                let cname = format!("vec-light.{}.color", i);
                let sname = format!("vec-light.{}", i);
                let c     = Self::get_json_color(&json, cname);
                let mut v = Self::get_json_vec3(&json, name);
                v = v.normalize();
                let i = json[&iname].as_f64().unwrap() as f32;
                assert!(i >= 0.0);
                self.lights.push(Arc::new(Box::new(VectorLight{ name: sname, dir: v, rgb: c, intensity: i })));
            }
        }

        {
            num_planes = json["num_planes"].as_u64().unwrap();
            for i in 0..num_planes {
                let name  = format!("plane.{}.position", i);
                let nname = format!("plane.{}.normal", i);
                let cname = format!("plane.{}.color", i);
                let aname = format!("plane.{}.albedo", i);
                let tname = format!("plane.{}.checkered", i);
                let rname = format!("plane.{}.reflectivity", i);
                let oname = format!("plane.{}", i);
                let p         = Self::get_json_vec3(&json, name);
                let normal    = Self::get_json_vec3(&json, nname);
                let rgb       = Self::get_json_color(&json, cname);
                let albedo    = Self::get_json_albedo(&json, aname);
                let checkered = Self::get_json_checkered(&json, tname);
                let r         = Self::get_json_reflectivity(&json, rname);
                let material = Material { rgb: rgb, albedo: albedo, checkered: checkered, reflectivity: r };
                self.objects.push(Arc::new(Box::new(Plane::new(oname, p, normal, material))));
            }
        }
        {
            num_spheres = json["num_spheres"].as_u64().unwrap();
            for i in 0..num_spheres {
                let name  = format!("sphere.{}.center", i);
                let rname = format!("sphere.{}.radius", i);
                let cname = format!("sphere.{}.color", i);
                let aname = format!("sphere.{}.albedo", i);
                let tname = format!("sphere.{}.checkered", i);
                let refname = format!("sphere.{}.reflectivity", i);
                let oname = format!("sphere.{}", i);
                let radius = json[&rname].as_f64().unwrap();
                let center    = Self::get_json_vec3(&json, name);
                let rgb       = Self::get_json_color(&json, cname);
                let albedo    = Self::get_json_albedo(&json, aname);
                let checkered = Self::get_json_checkered(&json, tname);
                let r         = Self::get_json_reflectivity(&json, refname);
                let material = Material { rgb: rgb, albedo: albedo, checkered: checkered, reflectivity: r };
                self.objects.push(Arc::new(Box::new(Sphere::new(oname, center, radius, material))));
            }
        }
        println!("resolution: {}x{}", self.opt.res_x, self.opt.res_y);
        println!("Scene has {} objects: num_spheres={} num_planes={}", self.objects.len(), num_spheres, num_planes);
        self.camera.as_ref().unwrap().display();
        for light in &self.lights {
            light.display();
        }
        Ok(())
    }

    pub fn save_image(&mut self) -> std::io::Result<()> {
        return self.image.lock().unwrap().save_image(PathBuf::from(&self.opt.img_file), self.opt.use_gamma > 0);
    }
}

fn generate_scene(num_spheres_to_generate: u32, scene_file: PathBuf, use_box: bool) -> std::io::Result<()> {
    let mut rng = rand::thread_rng();
    let mut json: serde_json::Value;

    println!("Generating scene w/ {} spheres", num_spheres_to_generate);
    json = serde_json::json!({
        "resolution": [ 400, 400 ],
        "camera.position": [ -2.5, 0.0, 1.0 ],
        "camera.direction": [ 1, 0, -0.10 ],
        "ambient.color": [ 1, 1, 1 ],
        "ambient.intensity": 0.2,
        "num_vec_lights": 1,
        "num_spot_lights": 2,
        "vec-light.0.vector": [ 0.5, 0.5, -0.5],
        "vec-light.0.intensity": 1.5,
        "vec-light.0.color": [ 1, 1, 1],
        "spot-light.0.position": [ 0.5, 2.5, 1],
        "spot-light.0.intensity": 200,
        "spot-light.0.color": [ 0.4, 0.4, 0.7],
        "spot-light.1.position": [ 0.5, -2, 0],
        "spot-light.1.intensity": 80,
        "spot-light.1.color": [ 0.8, 0.3, 0.3],
        "sphere.0.center" : [3, 0, -0.5],
        "sphere.0.radius" : 1.3,
        "sphere.0.color": [ 0.8, 0.7, 0.9],
        "sphere.0.checkered": true,
        "sphere.0.reflectivity" : 0.5,
        "num_planes" : 0
    });
    json["num_spheres"] = serde_json::json!(num_spheres_to_generate);

    if use_box {
        println!("using box!");
        json["num_planes"]        = serde_json::json!(6);
        json["plane.0.position" ] = serde_json::json!([0, 0, -1]); // bottom
        json["plane.0.normal" ]   = serde_json::json!([0, 0, 1]);
        json["plane.0.reflectivity" ] = serde_json::json!(0.1);
        json["plane.1.position" ] = serde_json::json!([0, 0, 3]); // top
        json["plane.1.normal" ]   = serde_json::json!([0, 0, -1]);
        json["plane.2.position" ] = serde_json::json!([4.5, 0, 0]); // front
        json["plane.2.normal" ]   = serde_json::json!([-1, 0, 0]);
        json["plane.2.color"]     = serde_json::json!([ 0.5, 0.9, 0.5]);
        json["plane.3.position" ] = serde_json::json!([0, 3, 0]); // left
        json["plane.3.normal" ]   = serde_json::json!([0, -1, 0]);
        json["plane.3.color"]     = serde_json::json!([ 1, 0.2, 0.2]);
        json["plane.4.position" ] = serde_json::json!([0, -3, 0]); // right
        json["plane.4.normal" ]   = serde_json::json!([0, 1, 0]);
        json["plane.4.color"]     = serde_json::json!([ 0.5, 0.5, 1]);
        json["plane.5.position" ] = serde_json::json!([-3, 0, 0]); // back
        json["plane.5.normal" ]   = serde_json::json!([1, 0, 0]);
        json["plane.5.color"]     = serde_json::json!([ 1, 1, 1]);
    }

    let line = false;
    for i in 1..num_spheres_to_generate {
        let mut x = rng.gen_range(2.0..5.0);
        let mut y = rng.gen_range(-2.0..2.0);
        let mut z = rng.gen_range(-2.0..2.0);
        let mut r = rng.gen_range(0.2..0.4);
        if line {
            x = i as f64 * 2.0;
            y = -1.0;
            z = -0.5;
            r = 0.7;
        }
        let cr = rng.gen_range(0.3..1.0);
        let cg = rng.gen_range(0.3..1.0);
        let cb = rng.gen_range(0.3..1.0);
        let albedo = rng.gen_range(0.5..1.0);
        let reflectivity = rng.gen_range(0.0..1.0);
        let checkered = rng.gen_range(0..100) % 2;
        let name  = format!("sphere.{}.center", i);
        let rname = format!("sphere.{}.radius", i);
        let cname = format!("sphere.{}.color", i);
        let aname = format!("sphere.{}.albedo", i);
        let tname = format!("sphere.{}.checkered", i);
        let refname = format!("sphere.{}.reflectivity", i);
        json[name]  = serde_json::json!([x, y, z ]);
        json[rname] = serde_json::json!(r);
        json[cname] = serde_json::json!([cr, cg, cb]);
        json[aname] = serde_json::json!(albedo);
        json[tname] = serde_json::json!(checkered > 0);
        json[refname] = serde_json::json!(reflectivity);
    }
    let s0 = serde_json::to_string_pretty(&json)?;
    println!("Writing scene file {}", scene_file.display());
    return fs::write(&scene_file, s0);
}

fn print_opt(opt: &Options) {
    println!("scene-file: {}", opt.scene_file.display());
    println!("image-file: {}", opt.img_file.display());
    println!("gamma-correction: {}", opt.use_gamma);
    println!("adaptive-sampling: {} max-depth: {}", opt.adaptive_sampling, opt.adaptive_max_depth);
    println!("reflection: {} max-depth: {}", opt.use_reflection, opt.reflection_max_depth);
}

fn main() -> std::io::Result<()> {
    let opt = Options::from_args();

    let mut job = RenderJob::new(opt);

     ctrlc::set_handler(move || {
         CTRLC_HIT.store(true, Ordering::SeqCst);
     })
     .expect("Error setting Ctrl-C handler");

    if job.opt.num_spheres_to_generate != 0 {
        return generate_scene(job.opt.num_spheres_to_generate, job.opt.scene_file, job.opt.use_box > 0);
    }

    print_opt(&job.opt);
    println!("num_threads: {}", rayon::current_num_threads());

    job.load_scene()?;
    job.render_scene();
    job.save_image()?;

    Ok(())
}
