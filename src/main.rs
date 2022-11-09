//#![allow(unused)]

use std::fs;
use structopt::StructOpt;
use std::path::PathBuf;
use std::time::Instant;
use serde_json;

mod image;
mod three_d;
use three_d::Vector;
use three_d::Point;
use three_d::Ray;
use three_d::Sphere;
use three_d::Camera;


#[derive(StructOpt, Debug)]
#[structopt(name="rtest", about="Maxime's playground")]
struct Options {
    #[structopt(long, default_value = "")]
    ppm_file: PathBuf,
    #[structopt(long, default_value = "")]
    scene_file: PathBuf,
     #[structopt(long, default_value = "320")]
    res_x: u32,
     #[structopt(long, default_value = "320")]
    res_y: u32,
}


struct RenderJob {
    opt: Options,
    start_time: Instant,
    camera: Camera,
    sphere: Sphere,
    image: image::Image,
    light: Vector,
}

impl RenderJob {
    pub fn new(opt: Options) -> Self {
        Self {
            start_time : Instant::now(),
            camera: Camera::new(Point  { x: 0.0, y: 0.0, z: 0.0 },
                                Vector { x: 1.0, y: 0.0, z: 0.0 }),
            sphere: Sphere::new(Point  { x: 3.0, y: 0.0, z: 0.0 }, 0.6 ),
            image: image::Image::new(opt.res_x, opt.res_y),
            light: Vector{ x: 0.5, y: -0.5, z: -0.5 },
            opt : opt,
        }
    }
    pub fn render_scene(&mut self) {
        let mut n = 0;
        for i in 0..self.opt.res_y {
            for j in 0..self.opt.res_x {
                let u = 1.0;
                let v = 1.0;
                let sy = u / 2.0 - j as f64 * u / self.opt.res_x as f64;
                let sz = v / 2.0 - i as f64 * v / self.opt.res_y as f64;
                let pixel = Point{ x: 1.0, y: sy, z: sz };

                let vec = Vector::create(&self.camera.pos, &pixel);
                let ray = Ray{ orig: self.camera.pos, dir: vec };

                let mut c = image::RGB{ r: 0, g: 0, b: 0 };
                if let Some(normal) = self.sphere.intercept(&ray) {
                    let v : f64 = 200.0 * normal.scalar(&self.light).powi(2);
                    let v8 : u8 = v as u8;
                    c = image::RGB{ r: v8, g: v8, b: v8 };
                    n += 1;
                } 

                self.image.push_pixel(c);
            }
        }
        println!("{} intercept", n);
    }
    pub fn parse_input_scene(&mut self) -> std::io::Result<()> {
        assert!(self.opt.scene_file.is_file());
        println!("output file: {:?}", self.opt.scene_file);

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
        self.camera.pos = p;
        self.camera.dir = v;
        let p = Point {
            x: json["sphere.0"][0].as_f64().unwrap(),
            y: json["sphere.0"][1].as_f64().unwrap(),
            z: json["sphere.0"][2].as_f64().unwrap()
        };
        let r = json["sphere.0"][3].as_f64().unwrap();
        self.sphere = Sphere::new(p, r);
        println!("camera: {:?}", self.camera.dir);
        println!("camera: {:?}", self.camera.pos);
        println!("sphere: {:?} r={:?}", self.sphere.center, self.sphere.radius);
        Ok(())
    }

    pub fn save_image(&mut self) -> std::io::Result<()> {
        let time_elapsed = self.start_time.elapsed();
        println!("duration: {:?}", time_elapsed);
        self.image.save_image(PathBuf::from(&self.opt.ppm_file))?;
        Ok(())
    }
}

fn main() -> std::io::Result<()> {
    let opt = Options::from_args();

    println!("all opt {:?}", opt);

    let mut job = RenderJob::new(opt);

    job.parse_input_scene()?;
    job.render_scene();
    job.save_image()?;
    Ok(())
}
