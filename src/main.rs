use structopt::StructOpt;
use serde_json;
use rand::Rng;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{ AtomicU64, Ordering};
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
}


struct RenderJob {
    opt: Options,
    start_time: Instant,
    camera: Option<Camera>,
    objects: Vec<Arc<Box<dyn Object + 'static + Send + Sync>>>,
    lights: Vec<Arc<Box<dyn Light + 'static + Send + Sync>>>,
    image: Mutex<Image>,
    pmap: Mutex<HashMap<String,RGB>>,
    num_rays_sampling: AtomicU64,
    num_rays_reflection: AtomicU64,
    hit_max_level: AtomicU64,
}


impl RenderJob {
    pub fn new(opt: Options) -> Self {
        Self {
            start_time : Instant::now(),
            camera: None,
            image: Mutex::new(Image::new(0, 0)),
            objects: vec![],
            lights: vec![],
            pmap: Mutex::new(HashMap::new()),
            opt : opt,
            num_rays_sampling: AtomicU64::new(0),
            num_rays_reflection: AtomicU64::new(0),
            hit_max_level: AtomicU64::new(0),
        }
    }
    fn calc_ray_color(&self, ray: Ray, view_all: bool, depth: u32) -> RGB {
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

                    let shadow = false;
                   //let light_ray = Ray{orig: hit_point, dir: light_vec};
                   //let mut t : f64 = 0.0;
                   //for obj in &self.objects {
                   //    if obj.intercept(&light_ray, 0.001, f64::MAX, &mut t) {
                   //        shadow = true;
                   //        break;
                   //    }
                   //}
                    if ! shadow {
                        let mut v_prod = (hit_normal * light_vec) as f32;
                        if v_prod > 0.0 { // only show visible side
                            v_prod = 0.0;
                        }
                        let v = v_prod.powi(4);
                        c_light = c_res * v;
                    }
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
                        let dist_sq = (light_vec * light_vec) as f32;
                        let mut v_prod = (hit_normal * light_vec_norm) as f32;
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
                self.num_rays_reflection.fetch_add(1, Ordering::SeqCst);
                let reflected_vec = ray.dir.reflect(hit_normal);
                let reflected_ray = Ray{orig: hit_point, dir: reflected_vec};
                let c_reflect = self.calc_ray_color(reflected_ray, true, depth + 1);
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

    pub fn corner_difference(c00: RGB, c01: RGB, c10: RGB, c11: RGB) -> bool {
        let avg = (c00 + c01 + c10 + c11) * 0.25;
        let d = avg.distance2(c00) + avg.distance2(c01) + avg.distance2(c10) + avg.distance2(c11);
        d > 0.3
        //let d = avg.distance(c00) + avg.distance(c01) + avg.distance(c10) + avg.distance(c11);
        //d > 0.5
    }

    /*
     * pos_u: -0.5 .. 0.5
     * pos_v: -0.5 .. 0.5
     */
    pub fn calc_ray_box(&self, pos_u: f64, pos_v: f64, du: f64, dv: f64, lvl: u32) -> RGB {
        let camera_pos = self.camera.as_ref().unwrap().pos;
        let camera_dir = self.camera.as_ref().unwrap().dir;
        let camera_u = self.camera.as_ref().unwrap().screen_u;
        let camera_v = self.camera.as_ref().unwrap().screen_v;

        let calc_one_corner = |u0, v0| -> RGB {
            let key = format!("{}-{}", u0, v0);
            {
                let pmap = self.pmap.lock().unwrap();
                if let Some(c) = pmap.get(&key) {
                    return *c;
                }
            }
            let pixel = camera_pos + camera_dir + camera_u * u0 + camera_v * v0;
            let ray = Ray{ orig: camera_pos, dir: pixel - camera_pos };
            self.num_rays_sampling.fetch_add(1, Ordering::SeqCst);
            let c = self.calc_ray_color(ray, false, 0);
            let mut pmap = self.pmap.lock().unwrap();
            pmap.insert(key, c);
            c
        };
        let mut c00 = calc_one_corner(pos_u,      pos_v);
        let mut c01 = calc_one_corner(pos_u,      pos_v + dv);
        let mut c10 = calc_one_corner(pos_u + du, pos_v);
        let mut c11 = calc_one_corner(pos_u + du, pos_v + dv);

        let color_diff = Self::corner_difference(c00, c01, c10, c11);
        if self.opt.adaptive_sampling > 0 && color_diff && lvl >= self.opt.adaptive_max_depth {
            self.hit_max_level.fetch_add(1, Ordering::SeqCst);
        }
        if self.opt.adaptive_sampling > 0 && lvl < self.opt.adaptive_max_depth && color_diff {
            let du2 = du / 2.0;
            let dv2 = dv / 2.0;
            c00 = self.calc_ray_box(pos_u,       pos_v,       du2, dv2, lvl + 1);
            c01 = self.calc_ray_box(pos_u,       pos_v + dv2, du2, dv2, lvl + 1);
            c10 = self.calc_ray_box(pos_u + du2, pos_v,       du2, dv2, lvl + 1);
            c11 = self.calc_ray_box(pos_u + du2, pos_v + dv2, du2, dv2, lvl + 1);
        }
        (c00 + c01 + c10 + c11) * 0.25
    }

    pub fn render_scene(&mut self) {
        let pb = ProgressBar::new(self.opt.res_y as u64);
        self.image = Mutex::new(Image::new(self.opt.res_x, self.opt.res_y));
        self.start_time = Instant::now();
        assert!(self.camera.is_some());
        let u = 1.0;
        let v = 1.0;
        let du = u / self.opt.res_x as f64;
        let dv = v / self.opt.res_y as f64;

        (0..self.opt.res_y).into_par_iter().for_each(|i| {
        //(0..self.opt.res_y).for_each(|i| {
            let mut pos_u = u / 2.0;
            let pos_v = v / 2.0 - (i as f64) * dv;
            for j in 0..self.opt.res_x {
                let c = self.calc_ray_box(pos_u, pos_v, du, dv, 0);

                let mut img = self.image.lock().unwrap();
                img.push_pixel(j, i, c);
                pos_u -= du;
            }
            pb.inc(1);
        });
        pb.finish_with_message("done");

        let num_pixels = self.opt.res_x * self.opt.res_y;
        println!("sampling: {} rays {}%", self.num_rays_sampling.load(Ordering::SeqCst), 100 * self.num_rays_sampling.load(Ordering::SeqCst) / num_pixels as u64);
        if self.num_rays_sampling.load(Ordering::SeqCst) > 0 {
            println!("reflection: {} rays {}%", self.num_rays_reflection.load(Ordering::SeqCst), 100 * self.num_rays_reflection.load(Ordering::SeqCst) / self.num_rays_sampling.load(Ordering::SeqCst));
            println!("{} sample rays hit max-depth {}%", self.hit_max_level.load(Ordering::SeqCst), 100 * self.hit_max_level.load(Ordering::SeqCst) / self.num_rays_sampling.load(Ordering::SeqCst));
        }
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
        println!("Loading scene file: {:?}", self.opt.scene_file);

        let data = fs::read_to_string(&self.opt.scene_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;

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
            let num_planes = json["num_planes"].as_u64().unwrap();
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
            let num_spheres = json["num_spheres"].as_u64().unwrap();
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
        println!("camera: pos: {:?}", self.camera.as_ref().unwrap().pos);
        println!("camera: dir: {:?}", self.camera.as_ref().unwrap().dir);
        println!("Scene has {} objects", self.objects.len());
        for light in &self.lights {
            light.display();
        }
        Ok(())
    }

    pub fn save_image(&mut self) -> std::io::Result<()> {
        let elapsed = self.start_time.elapsed();
        println!("duration: {} sec", elapsed.as_millis() as f64 / 1000.0);
        let mut img = self.image.lock().unwrap();
        return img.save_image(PathBuf::from(&self.opt.img_file), self.opt.use_gamma > 0);
    }
}

fn generate_scene(num_spheres_to_generate: u32, scene_file: PathBuf) ->  std::io::Result<()> {
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
        "sphere.0.radius" : 1,
        "sphere.0.color": [ 0.8, 0.7, 0.9],
        "sphere.0.checkered": true,
        "sphere.0.reflectivity" : 0.5,
        "num_planes": 6,
        "plane.0.position" : [0, 0, -1], // bottom
        "plane.0.normal" : [0, 0, 1],
        "plane.0.reflectivity" : 0.1,
        "plane.1.position" : [0, 0, 3], // top
        "plane.1.normal" : [0, 0, -1],
        "plane.2.position" : [4.5, 0, 0], // front
        "plane.2.normal" : [-1, 0, 0],
        "plane.2.color": [ 0.5, 0.9, 0.5],
        "plane.3.position" : [0, 3, 0], // left
        "plane.3.normal" : [0, -1, 0],
        "plane.3.color": [ 1, 0.2, 0.2],
        "plane.4.position" : [0, -3, 0], // right
        "plane.4.normal" : [0, 1, 0],
        "plane.4.color": [ 0.5, 0.5, 1],
        "plane.5.position" : [-3, 0, 0], // back
        "plane.5.normal" : [1, 0, 0],
        "plane.5.color": [ 1, 1, 1],
    });
    json["num_spheres"] = serde_json::json!(num_spheres_to_generate);

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
    println!("resolution: {}x{}", opt.res_x, opt.res_y);
    println!("gamma-correction: {}", opt.use_gamma);
    println!("adaptive-sampling: {} max-depth={}", opt.adaptive_sampling, opt.adaptive_max_depth);
    println!("reflection: {} max-depth={}", opt.use_reflection, opt.reflection_max_depth);
}

fn main() -> std::io::Result<()> {
    let opt = Options::from_args();

    let mut job = RenderJob::new(opt);

    if job.opt.num_spheres_to_generate != 0 {
        return generate_scene(job.opt.num_spheres_to_generate, job.opt.scene_file);
    }

    print_opt(&job.opt);

    job.load_scene()?;
    job.render_scene();
    job.save_image()?;

    Ok(())
}
