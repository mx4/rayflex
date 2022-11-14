use structopt::StructOpt;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::collections::HashMap;
use serde_json;
use rand::Rng;

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
    #[structopt(long, default_value = "pic.ppm")]
    ppm_file: PathBuf,
    #[structopt(long, default_value = "scene.json")]
    scene_file: PathBuf,
     #[structopt(short="x", long, default_value = "400")]
    res_x: u32,
     #[structopt(short="y", long, default_value = "400")]
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
}


struct RenderJob {
    opt: Options,
    start_time: Instant,
    camera: Option<Camera>,
    objects: Vec<Box<dyn Object + 'static>>,
    lights: Vec<Box<dyn Light + 'static>>,
    image: Image,
    pmap: HashMap<String,RGB>,
    num_rays_sampling: u64,
    num_rays_reflection: u64,
    hit_max_level: u64,
}


impl RenderJob { // ??
    pub fn new(opt: Options) -> Self {
        Self {
            start_time : Instant::now(),
            camera: None,
            image: Image::new(opt.res_x, opt.res_y),
            objects: vec![],
            lights: vec![],
            pmap: HashMap::new(),
            opt : opt,
            num_rays_sampling: 0,
            num_rays_reflection: 0,
            hit_max_level: 0,
        }
    }
    fn calc_ray_color(&mut self, ray: Ray, view_all: bool, depth: u32) -> RGB {
        let mut c = RGB::new();
        let mut tmin = f64::MAX;
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

        for obj in &self.objects {
            let mut t : f64 = 0.0;
            if obj.intercept(&ray, raylen, tmin, &mut t) {
                assert!(t < tmin);
                hit_point = ray.orig + ray.dir * t;
                hit_normal = obj.get_normal(hit_point);
                hit_material = obj.get_material();
                if hit_material.checkered {
                    (hitx, hity) = obj.get_texture_2d(hit_point);
                }
                tmin = t;
            }
        }
        if tmin < f64::MAX {
            for light in &self.lights {
                let light_intensity = light.get_intensity();
                let light_rgb = light.get_color();
                let mut c_light = RGB::new();
                let mut c_res = RGB{
                    r : hit_material.rgb.r * light_rgb.r,
                    g : hit_material.rgb.g * light_rgb.g,
                    b : hit_material.rgb.b * light_rgb.b,
                };
                assert!(light_intensity >= 0.0);
                c_res = c_res * light_intensity;

                if light.is_ambient() {
                    c_light = c_res;
                } else if light.is_vector() {
                    let light_vec = light.get_vector(hit_point);
                    let mut v_prod = (hit_normal * light_vec) as f32;
                    if v_prod > 0.0 { // only show visible side
                        v_prod = 0.0;
                    }
                    let v = v_prod.powi(4);
                    assert!(v >= 0.0);
                    c_light = c_res * v;
                } else {
                    assert!(light.is_spot());
                    let light_vec = light.get_vector(hit_point) * -1.0;
                    let mut light_vec_norm = light_vec.clone();
                    light_vec_norm.normalize();
                    let mut shadow = false;
                    let light_ray = Ray{orig: hit_point, dir: light_vec};
                    let mut t : f64 = 0.0;
                    for obj in &self.objects {
                        if obj.intercept(&light_ray, 0.001, 1.0, &mut t) {
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
                        let v = v_prod.powi(4) / (4.0 * std::f32::consts::PI * dist_sq);
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
                c = c + c_light * hit_material.albedo;
            }
            if self.opt.use_reflection > 0 {
                self.num_rays_reflection += 1;
                let reflected_vec = ray.dir.reflect(hit_normal);
                let reflected_ray = Ray{orig: hit_point, dir: reflected_vec};
                c = c + self.calc_ray_color(reflected_ray, true, depth + 1) * 0.5;
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

    pub fn calc_ray_box(&mut self, sy: f64, sz: f64, dy: f64, dz: f64, lvl: u32) -> RGB {
        let camera_pos = self.camera.as_ref().unwrap().pos;
        let camera_dir = self.camera.as_ref().unwrap().dir;
        let camera_u = self.camera.as_ref().unwrap().screen_u;
        let camera_v = self.camera.as_ref().unwrap().screen_v;

        let mut calc_one_corner = |y0, z0| -> RGB {
            let key = format!("{}-{}", y0, z0);
            match self.pmap.get(&key) {
                Some(c) => return *c,
                _ =>  {
                    let pixel = camera_pos + camera_dir + camera_u * y0 + camera_v * z0;
                    let vec = pixel - camera_pos;
                    let ray = Ray{ orig: camera_pos, dir: vec };
                    self.num_rays_sampling += 1;
                    let c = self.calc_ray_color(ray, false, 0);
                    self.pmap.insert(key, c);
                    c
                }
            }
        };
        let mut c00 = calc_one_corner(sy,      sz);
        let mut c01 = calc_one_corner(sy,      sz - dz);
        let mut c10 = calc_one_corner(sy - dy, sz);
        let mut c11 = calc_one_corner(sy - dy, sz - dz);

        let color_diff = Self::corner_difference(c00, c01, c10, c11);
        if self.opt.adaptive_sampling > 0 && color_diff && lvl >= self.opt.adaptive_max_depth {
            self.hit_max_level += 1;
        }
        if self.opt.adaptive_sampling > 0 && lvl < self.opt.adaptive_max_depth && color_diff {
            let dy2 = dy / 2.0;
            let dz2 = dz / 2.0;
            c00 = self.calc_ray_box(sy,       sz,       dy2, dz2, lvl + 1);
            c01 = self.calc_ray_box(sy,       sz - dz2, dy2, dz2, lvl + 1);
            c10 = self.calc_ray_box(sy - dy2, sz,       dy2, dz2, lvl + 1);
            c11 = self.calc_ray_box(sy - dy2, sz - dz2, dy2, dz2, lvl + 1);
        }
        (c00 + c01 + c10 + c11) * 0.25
    }
    pub fn render_scene(&mut self) {
        self.start_time = Instant::now();
        assert!(self.camera.is_some());
        let u = 1.0;
        let v = 1.0;
        let dy = u / self.opt.res_x as f64;
        let dz = v / self.opt.res_y as f64;

        let mut sz = v / 2.0;
        for _i in 0..self.opt.res_y {
            let mut sy = u / 2.0;
            for _j in 0..self.opt.res_x {
                let c = self.calc_ray_box(sy, sz, dy, dz, 0);

                self.image.push_pixel(c);
                sy -= dy;
            }
            sz -= dz;
        }
        let num_pixels = self.opt.res_x * self.opt.res_y;
        println!("sampling: {} rays {}%", self.num_rays_sampling, 100 * self.num_rays_sampling / num_pixels as u64);
        println!("reflection: {} rays {}%", self.num_rays_reflection, 100 * self.num_rays_reflection / self.num_rays_sampling as u64);
        println!("{} sample rays hit max-depth {}%", self.hit_max_level, 100 * self.hit_max_level / self.num_rays_sampling);
    }
    pub fn load_scene(&mut self) -> std::io::Result<()> {
        if ! self.opt.scene_file.is_file() {
            panic!("scene file {} not present.", self.opt.scene_file.display());
        }
        println!("Loading scene file: {:?}", self.opt.scene_file);

        let data = fs::read_to_string(&self.opt.scene_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;

        {
            let p = Point {
                x: json["camera.position"][0].as_f64().unwrap(),
                y: json["camera.position"][1].as_f64().unwrap(),
                z: json["camera.position"][2].as_f64().unwrap()
            };
            let v = Vec3 {
                x: json["camera.direction"][0].as_f64().unwrap(),
                y: json["camera.direction"][1].as_f64().unwrap(),
                z: json["camera.direction"][2].as_f64().unwrap()
            };
            self.camera = Some(Camera::new(p, v));
        }
        {
            let name = "ambient.light";
            let c = RGB {
                r: json[&name][0].as_f64().unwrap() as f32,
                g: json[&name][1].as_f64().unwrap() as f32,
                b: json[&name][2].as_f64().unwrap() as f32
            };
            assert!(c.r >= 0.0);
            assert!(c.g >= 0.0);
            assert!(c.b >= 0.0);
            let r = json[&name][3].as_f64().unwrap() as f32;
            assert!(r >= 0.0);
            self.lights.push(Box::new(AmbientLight{ name: name.to_string(), rgb: c, intensity: r }));
        }
        {
            let num_spot_lights = json["num_spot_lights"].as_u64().unwrap();
            for i in 0..num_spot_lights {
                let name = format!("spot-light.{}.position", i);
                let p = Point {
                    x: json[&name][0].as_f64().unwrap(),
                    y: json[&name][1].as_f64().unwrap(),
                    z: json[&name][2].as_f64().unwrap()
                };
                let iname = format!("spot-light.{}.intensity", i);
                let r = json[&iname].as_f64().unwrap() as f32;
                assert!(r >= 0.0);
                let cname = format!("spot-light.{}.color", i);
                let c = RGB {
                    r: json[&cname][0].as_f64().unwrap() as f32,
                    g: json[&cname][1].as_f64().unwrap() as f32,
                    b: json[&cname][2].as_f64().unwrap() as f32
                };
                assert!(c.r >= 0.0);
                assert!(c.g >= 0.0);
                assert!(c.b >= 0.0);
                let sname = format!("spot-light.{}", i);
                self.lights.push(Box::new(SpotLight{ name: sname, pos: p, rgb: c, intensity: r }));
            }
        }
        {
            let num_vec_lights = json["num_vec_lights"].as_u64().unwrap();
            for i in 0..num_vec_lights {
                let name = format!("vec-light.{}.vector", i);
                let mut v = Vec3 {
                    x: json[&name][0].as_f64().unwrap(),
                    y: json[&name][1].as_f64().unwrap(),
                    z: json[&name][2].as_f64().unwrap()
                };
                v.normalize(); // how about intensity?
                let iname = format!("vec-light.{}.intensity", i);
                let r = json[&iname].as_f64().unwrap() as f32;
                assert!(r >= 0.0);
                let cname = format!("vec-light.{}.color", i);
                let c = RGB {
                    r: json[&cname][0].as_f64().unwrap() as f32,
                    g: json[&cname][1].as_f64().unwrap() as f32,
                    b: json[&cname][2].as_f64().unwrap() as f32
                };
                assert!(c.r >= 0.0);
                assert!(c.g >= 0.0);
                assert!(c.b >= 0.0);
                let sname = format!("vec-light.{}", i);
                self.lights.push(Box::new(VectorLight{ name: sname, dir: v, rgb: c, intensity: r }));
            }
        }

        {
            let num_planes = json["num_planes"].as_u64().unwrap();
            for i in 0..num_planes {
                let name = format!("plane.{}.position", i);
                let p = Point {
                    x: json[&name][0].as_f64().unwrap(),
                    y: json[&name][1].as_f64().unwrap(),
                    z: json[&name][2].as_f64().unwrap()
                };
                let name = format!("plane.{}.normal", i);
                let n = Vec3 {
                    x: json[&name][0].as_f64().unwrap(),
                    y: json[&name][1].as_f64().unwrap(),
                    z: json[&name][2].as_f64().unwrap()
                };
                let cname = format!("plane.{}.color", i);
                let cr = json[&cname][0].as_f64().unwrap() as f32;
                let cg = json[&cname][1].as_f64().unwrap() as f32;
                let cb = json[&cname][2].as_f64().unwrap() as f32;
                assert!(cr >= 0.0);
                assert!(cg >= 0.0);
                assert!(cb >= 0.0);
                let aname = format!("plane.{}.albedo", i);
                let albedo = json[&aname].as_f64().unwrap() as f32;
                assert!(albedo >= 0.0);
                let tname = format!("plane.{}.checkered", i);
                let checkered = json[&tname].as_bool().unwrap();
                let rgb = RGB{ r: cr, g: cg, b: cb };
                let material = Material { rgb: rgb, albedo: albedo, checkered: checkered };
                self.objects.push(Box::new(Plane::new(name, p, n, material)));
            }
        }
        {
            let num_spheres = json["num_spheres"].as_u64().unwrap();
            for i in 0..num_spheres {
                let name = format!("sphere.{}.center", i);
                let p = Point {
                    x: json[&name][0].as_f64().unwrap(),
                    y: json[&name][1].as_f64().unwrap(),
                    z: json[&name][2].as_f64().unwrap()
                };
                let r = json[&name][3].as_f64().unwrap();
                let cname = format!("sphere.{}.color", i);
                let cr = json[&cname][0].as_f64().unwrap() as f32;
                let cg = json[&cname][1].as_f64().unwrap() as f32;
                let cb = json[&cname][2].as_f64().unwrap() as f32;
                assert!(cr >= 0.0);
                assert!(cg >= 0.0);
                assert!(cb >= 0.0);
                let aname = format!("sphere.{}.albedo", i);
                let albedo = json[&aname].as_f64().unwrap() as f32;
                assert!(albedo >= 0.0);
                let tname = format!("sphere.{}.checkered", i);
                let checkered = json[&tname].as_bool().unwrap();
                let rgb = RGB{ r: cr, g: cg, b: cb };
                let material = Material { rgb: rgb, albedo: albedo, checkered: checkered };
                self.objects.push(Box::new(Sphere::new(name, p, r, material)));
            }
        }
        println!("camera: pos: {:?}", self.camera.as_ref().unwrap().pos);
        println!("camera: dir: {:?}", self.camera.as_ref().unwrap().dir);
        for light in &self.lights {
            light.display();
        }
        println!("Scene has {} objects", self.objects.len());
        Ok(())
    }

    pub fn save_image(&mut self) -> std::io::Result<()> {
        let elapsed = self.start_time.elapsed();
        println!("duration: {} sec", elapsed.as_millis() as f64 / 1000.0);
        return self.image.save_image(PathBuf::from(&self.opt.ppm_file));
    }
}

fn generate_scene(num_spheres_to_generate: u32, scene_file: PathBuf) ->  std::io::Result<()> {
    let mut rng = rand::thread_rng();
    let mut json: serde_json::Value;

    println!("Generating scene w/ {} spheres", num_spheres_to_generate);
    json = serde_json::json!({
        "camera.position": [ -3, 0.0, 1.2 ],
        "camera.direction": [ 1, 0, -0.25 ],
        "num_vec_lights": 1,
        "num_spot_lights": 1,
        "vec-light.0.vector": [ 0.5, -0.5, 0.5],
        "vec-light.0.intensity": 2.0,
        "vec-light.0.color": [ 1, 1, 1],
        "spot-light.0.position": [ 3, 3, 3],
        "spot-light.0.intensity": 400,
        "spot-light.0.color": [ 0.4, 0.5, 0.7],
        "ambient.light": [ 0.1, 0.1, 0.1, 0.5],
        "num_planes": 1,
        "plane.0.position" : [0, 0, -1],
        "plane.0.normal" : [0, 0, 1],
        "plane.0.color": [ 0.8, 0.7, 0.9],
        "plane.0.albedo": 0.8,
        "plane.0.checkered": false,
        "sphere.0.center" : [3, 0, -0.5, 1],
        "sphere.0.color": [ 0.8, 0.7, 0.9],
        "sphere.0.albedo": 0.8,
        "sphere.0.checkered": true,
        "sphere.1.center" : [2.2, -0.5, 0.5, 0.5],
        "sphere.1.color": [ 0.8, 0.7, 0.9],
        "sphere.1.albedo": 0.8,
        "sphere.1.checkered": true,
    });
    json["num_spheres"] = serde_json::json!(num_spheres_to_generate);

    let line = false;
    for i in 2..num_spheres_to_generate {
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
        let checkered = rng.gen_range(0..100) % 2;
        let name  = format!("sphere.{}.center", i);
        let cname = format!("sphere.{}.color", i);
        let aname = format!("sphere.{}.albedo", i);
        let tname = format!("sphere.{}.checkered", i);
        json[name]  = serde_json::json!([x, y, z, r ]);
        json[cname] = serde_json::json!([cr, cg, cb]);
        json[aname] = serde_json::json!(albedo);
        json[tname] = serde_json::json!(checkered > 0);
    }
    let s0 = serde_json::to_string_pretty(&json)?;
    println!("Writing scene file {}", scene_file.display());
    return fs::write(&scene_file, s0);
}

fn print_opt(opt: &Options) {
    println!("scene-file: {}", opt.scene_file.display());
    println!("image-file: {}", opt.ppm_file.display());
    println!("resolution: {}x{}", opt.res_x, opt.res_y);
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
