//#![allow(unused)]

use std::fs;
use structopt::StructOpt;
use std::path::PathBuf;
use std::time::Instant;
use serde_json;
use rand::Rng;
use raymax::color::RGB;

mod image;
mod three_d;
use three_d::Object;
use three_d::Vector;
use three_d::Point;
use three_d::Ray;
use three_d::Sphere;
use three_d::Camera;

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
}


struct RenderJob {
    opt: Options,
    start_time: Instant,
    camera: Option<Camera>,
    objects: Vec<Box<dyn Object + 'static>>,
    image: image::Image,
    light: Vector,
}

impl RenderJob { // ??
    pub fn new(opt: Options) -> Self {
        Self {
            start_time : Instant::now(),
            camera: None,
            image: image::Image::new(opt.res_x, opt.res_y),
            light: Vector{ x: 0.5, y: -0.5, z: -0.5 },
            objects: vec![],
            opt : opt,
        }
    }
    pub fn render_scene(&mut self) {
        assert!(self.camera.is_some());
        let camera_pos = self.camera.as_ref().unwrap().pos;
        let mut n = 0;
        for i in 0..self.opt.res_y {
            for j in 0..self.opt.res_x {
                let u = 1.0;
                let v = 1.0;
                let sy = u / 2.0 - j as f64 * u / self.opt.res_x as f64;
                let sz = v / 2.0 - i as f64 * v / self.opt.res_y as f64;
                let pixel = Point{ x: 1.0, y: sy, z: sz };

                let vec = Vector::create(&camera_pos, &pixel);
                let ray = Ray{ orig: camera_pos, dir: vec };

                let mut c = RGB{ r: 0.0, g: 0.0, b: 0.0 };
                let mut tmin = f64::MAX;
                for obj in &self.objects {
                    let mut t : f64 = 0.0;
                    if obj.intercept(&ray, &mut t) {
                        if t < tmin {
                            let scaled_dir = ray.dir.scale(t);
                            let point = ray.orig.add(&scaled_dir);
                            let normal = obj.get_normal(&point);
                            let mut v_prod = normal.scalar(&self.light);
                            if v_prod > 0.0 { // only show visible side
                                v_prod = 0.0;
                            }
                            let (x, y) = obj.get_texture_2d(&point);
                            let rgb = obj.get_color(&point);
                            let mut v = v_prod * v_prod;
                            let pattern = ((x * 4.0).fract() > 0.5) ^ ((y * 4.0).fract() > 0.5);
                            if pattern {
                                v /= 1.4;
                            }
                            c = RGB{ r: v * rgb.r, g: v * rgb.g, b: v * rgb.b };
                            tmin = t;
                        }
                        n += 1;
                    }
                }

                self.image.push_pixel(c);
            }
        }
        println!("{} intercepts", n);
    }
    pub fn parse_input_scene(&mut self) -> std::io::Result<()> {
        if ! self.opt.scene_file.is_file() {
            panic!("scene file {} not present.", self.opt.scene_file.display());
        }
        println!("Loading scene file: {:?}", self.opt.scene_file);

        let data = fs::read_to_string(&self.opt.scene_file)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;

        let p = Point {
            x: json["camera.position"][0].as_f64().unwrap(),
            y: json["camera.position"][1].as_f64().unwrap(),
            z: json["camera.position"][2].as_f64().unwrap()
        };
        let mut v = Vector {
            x: json["camera.direction"][0].as_f64().unwrap(),
            y: json["camera.direction"][1].as_f64().unwrap(),
            z: json["camera.direction"][2].as_f64().unwrap()
        };
        v.normalize();
        self.camera = Some(Camera::new(p, v));
        let mut v = Vector {
            x: json["light.0"][0].as_f64().unwrap(),
            y: json["light.0"][1].as_f64().unwrap(),
            z: json["light.0"][2].as_f64().unwrap()
        };
        v.normalize(); // how about intensity?
        self.light = v;

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
        println!("camera: {:?}", self.camera.as_ref().unwrap().pos);
        println!("camera: {:?}", self.camera.as_ref().unwrap().dir);
        println!("light: {:?}", self.light);
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
        "light.0": [ 0.5, -0.5, -0.5]
    });
    json["num_spheres"] = serde_json::json!(num_spheres_to_generate);

    for i in 0..num_spheres_to_generate {
        let x = rng.gen_range(2.0..5.0);
        let y = rng.gen_range(-2.0..2.0);
        let z = rng.gen_range(-2.0..2.0);
        let r = rng.gen_range(0.05..0.3);
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
    job.parse_input_scene()?;
    job.render_scene();
    job.save_image()?;

    Ok(())
}
