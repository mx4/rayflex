//#![allow(unused)]

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

use three_d::Object;
use three_d::Sphere;

#[derive(StructOpt, Debug)]
#[structopt(name="rtest", about="minimal raytracer")]
struct Options {
    #[structopt(long, default_value = "pic.ppm")]
    ppm_file: PathBuf,
    #[structopt(long, default_value = "scene.json")]
    scene_file: PathBuf,
     #[structopt(long, default_value = "400")]
    res_x: u32,
     #[structopt(long, default_value = "400")]
    res_y: u32,
     #[structopt(long, default_value = "0")]
    num_spheres_to_generate: u32,
     #[structopt(long, default_value = "1")]
    adaptive_sampling: u8,
     #[structopt(long, default_value = "3")]
    adaptive_max_depth: u32,
}


struct RenderJob {
    opt: Options,
    start_time: Instant,
    camera: Option<Camera>,
    objects: Vec<Box<dyn Object + 'static>>,
    lights: Vec<Box<dyn Light + 'static>>,
    image: Image,
    pmap: HashMap<String,RGB>,
    num_rays: u64,
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
            num_rays: 0,
            hit_max_level: 0,
        }
    }
    fn calc_ray_color(&mut self, ray: Ray, n: &mut u32) -> RGB {
        self.num_rays += 1;
        let mut c = RGB{ r: 0.0, g: 0.0, b: 0.0 };
        let mut tmin = f64::MAX;
        let mut nointercept = true;
        for obj in &self.objects {
            let mut t : f64 = 0.0;
            if obj.intercept(&ray, &mut t) {
                nointercept = false;
                if t < tmin {
                    let point = ray.orig + ray.dir * t;
                    let normal = obj.get_normal(point);
                    let (x, y) = obj.get_texture_2d(point);
                    let obj_rgb = obj.get_color(point);
                    let pattern = ((x * 4.0).fract() > 0.5) ^ ((y * 4.0).fract() > 0.5);

                    c = RGB{ r: 0.0, g: 0.0, b: 0.0 };
                    for light in &self.lights {
                        let intensity = light.get_intensity();
                        let light_rgb = light.get_color();

                        if light.is_ambient() {
                            c = c + light_rgb * intensity;
                            c = c + obj_rgb * intensity;
                        } else {
                            let light_vec = light.get_vector(point);
                            let mut v_prod = normal * light_vec;
                            if v_prod > 0.0 { // only show visible side
                                v_prod = 0.0;
                            }
                            let mut v = v_prod.powi(4);
                            if pattern {
                                v /= 1.4;
                            }
                            c = c + obj_rgb * v;
                            c = c + light_rgb * v * intensity;
                        }
                    }
                    tmin = t;
                }
                *n += 1;
            }
        }
        if nointercept {
	    let z =  ray.dir.z + 1.0;
	    let cmax = RGB{ r: 1.0, g: 1.0, b: 1.0 };
	    let cyan = RGB{ r: 0.4, g: 0.6, b: 0.9 };
            c = cmax * (1.0 - z) + cyan * z;
        }
        c
    }

    pub fn corner_difference(c00: RGB, c01: RGB, c10: RGB, c11: RGB) -> bool {
        let avg = (c00 + c01 + c10 + c11) * 0.25;
        let d = avg.distance(c00) + avg.distance(c01) + avg.distance(c10) + avg.distance(c11);
        d > 0.5
    }

    pub fn calc_ray_box(&mut self, sy: f64, sz: f64, dy: f64, dz: f64, n: &mut u32, lvl: u32) -> RGB {
        let camera_pos = self.camera.as_ref().unwrap().pos;

        let mut calc_one_corner = |y0, z0| -> RGB {
            let key = format!("{}-{}", y0, z0);
            match self.pmap.get(&key) {
                Some(c) => return *c,
                _ =>  {
                    let pixel = Point{ x: 1.0, y: y0, z: z0 };
                    let vec = pixel - camera_pos;
                    let ray = Ray{ orig: camera_pos, dir: vec };
                    let c = self.calc_ray_color(ray, n);
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
            c00 = self.calc_ray_box(sy,       sz,       dy2, dz2, n, lvl + 1);
            c01 = self.calc_ray_box(sy,       sz - dz2, dy2, dz2, n, lvl + 1);
            c10 = self.calc_ray_box(sy - dy2, sz,       dy2, dz2, n, lvl + 1);
            c11 = self.calc_ray_box(sy - dy2, sz - dz2, dy2, dz2, n, lvl + 1);
        }
        (c00 + c01 + c10 + c11) * 0.25
    }
    pub fn render_scene(&mut self) {
        assert!(self.camera.is_some());
        let mut n : u32 = 0;
        let u = 1.0;
        let v = 1.0;
        let ny = self.opt.res_y + 0;
        let nx = self.opt.res_x + 0;
        let dy = u / self.opt.res_x as f64;
        let dz = v / self.opt.res_y as f64;
        let mut sz = v / 2.0;

        for _i in 0..ny {
            let mut sy = u / 2.0;
            for _j in 0..nx {
                let c = self.calc_ray_box(sy, sz, dy, dz, &mut n, 0);

                self.image.push_pixel(c);
                sy -= dy;
            }
            sz -= dz;
        }
        println!("{} intercepts", n);
        println!("{} rays traced, {}%", self.num_rays, 100 * self.num_rays / (self.opt.res_x * self.opt.res_y) as u64);
        println!("{} hit max-depth", self.hit_max_level);
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
            let mut v = Vec3 {
                x: json["camera.direction"][0].as_f64().unwrap(),
                y: json["camera.direction"][1].as_f64().unwrap(),
                z: json["camera.direction"][2].as_f64().unwrap()
            };
            v.normalize();
            self.camera = Some(Camera::new(p, v));
        }
        {
            let name = "ambient.light";
            let v = RGB {
                r: json[&name][0].as_f64().unwrap(),
                g: json[&name][1].as_f64().unwrap(),
                b: json[&name][2].as_f64().unwrap()
            };
            let r = json[&name][3].as_f64().unwrap();
            self.lights.push(Box::new(AmbientLight{ name: name.to_string(), rgb: v, intensity: r }));
        }
        {
            let num_vec_lights = json["num_spot_lights"].as_u64().unwrap();
            for i in 0..num_vec_lights {
                let name = format!("spot-light.{}.position", i);
                let p = Point {
                    x: json[&name][0].as_f64().unwrap(),
                    y: json[&name][1].as_f64().unwrap(),
                    z: json[&name][2].as_f64().unwrap()
                };
                let iname = format!("vec-light.{}.intensity", i);
                let r = json[&iname].as_f64().unwrap();
                let cname = format!("vec-light.{}.color", i);
                let c = RGB {
                    r: json[&cname][0].as_f64().unwrap(),
                    g: json[&cname][1].as_f64().unwrap(),
                    b: json[&cname][2].as_f64().unwrap()
                };
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
                let r = json[&iname].as_f64().unwrap();
                let cname = format!("vec-light.{}.color", i);
                let c = RGB {
                    r: json[&cname][0].as_f64().unwrap(),
                    g: json[&cname][1].as_f64().unwrap(),
                    b: json[&cname][2].as_f64().unwrap()
                };
                let sname = format!("vec-light.{}", i);
                self.lights.push(Box::new(VectorLight{ name: sname, dir: v, rgb: c, intensity: r }));
            }
        }

        {
            let num_spheres = json["num_spheres"].as_u64().unwrap();
            for i in 0..num_spheres {
                let name = format!("sphere.{}", i);
                let p = Point {
                    x: json[&name][0].as_f64().unwrap(),
                    y: json[&name][1].as_f64().unwrap(),
                    z: json[&name][2].as_f64().unwrap()
                };
                let r = json[&name][3].as_f64().unwrap();
                let cname = format!("sphere.{}.color", i);
                let cr = json[&cname][0].as_f64().unwrap();
                let cg = json[&cname][1].as_f64().unwrap();
                let cb = json[&cname][2].as_f64().unwrap();
                let rgb = RGB{ r: cr, g: cg, b: cb };
                self.objects.push(Box::new(Sphere::new(name, p, r, rgb)));
            }
        }
        println!("camera: {:?}", self.camera.as_ref().unwrap().pos);
        println!("camera: {:?}", self.camera.as_ref().unwrap().dir);
        for light in &self.lights {
            light.display();
        }
        println!("Scene has {} objects", self.objects.len());
        Ok(())
    }

    pub fn save_image(&mut self) -> std::io::Result<()> {
        let time_elapsed = self.start_time.elapsed();
        println!("duration: {:?}", time_elapsed);
        return self.image.save_image(PathBuf::from(&self.opt.ppm_file));
    }
}

fn generate_scene(num_spheres_to_generate: u32, scene_file: PathBuf) ->  std::io::Result<()> {
    let mut rng = rand::thread_rng();
    let mut json: serde_json::Value;

    println!("Generating scene w/ {} spheres", num_spheres_to_generate);
    json = serde_json::json!({
        "camera.position": [ 0.0, 0.0, 0.0 ],
        "camera.direction": [ 1, 0, 0 ],
        "num_vec_lights": 1,
        "vec-light.0.vector": [ 0.5, -0.5, -0.5],
        "vec-light.0.intensity": 0.2,
        "vec-light.0.color": [ 1, 1, 1],
        "num_spot_lights": 1,
        "spot-light.0.position": [ 0, -1, -1],
        "spot-light.0.intensity": 0.2,
        "spot-light.0.color": [ 1, 1, 0.1],
        "ambient.light": [ 0.1, 0.1, 0.1, 0.05]
    });
    json["num_spheres"] = serde_json::json!(num_spheres_to_generate);

    for i in 0..num_spheres_to_generate {
        let x = rng.gen_range(2.0..5.0);
        let y = rng.gen_range(-2.0..2.0);
        let z = rng.gen_range(-2.0..2.0);
        let r = rng.gen_range(0.05..0.5);
        let cr = rng.gen_range(0.3..1.0);
        let cg = rng.gen_range(0.3..1.0);
        let cb = rng.gen_range(0.3..1.0);
        let name  = format!("sphere.{}", i);
        let cname = format!("sphere.{}.color", i);
        json[name]  = serde_json::json!([x, y, z, r ]);
        json[cname] = serde_json::json!([cr, cg, cb]);
    }
    let s0 = serde_json::to_string_pretty(&json)?;
    println!("Writing scene file {}", scene_file.display());
    return fs::write(&scene_file, s0);
}

fn main() -> std::io::Result<()> {
    let opt = Options::from_args();

    println!("all opt {:?}", opt);

    let mut job = RenderJob::new(opt);

    if job.opt.num_spheres_to_generate != 0 {
        return generate_scene(job.opt.num_spheres_to_generate, job.opt.scene_file);
    }
    job.load_scene()?;
    job.render_scene();
    job.save_image()?;

    Ok(())
}
