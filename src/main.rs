//#![allow(unused)]

use structopt::StructOpt;
use std::path::PathBuf;
use std::time::Instant;

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
    name : String,
    #[structopt(long, default_value = "")]
    folder: PathBuf,
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
        println!("rendering {:?}", self.opt.name);
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
    pub fn parse_input_scene(&mut self) {
        println!("camera: {:?}", self.camera.dir);
        println!("camera: {:?}", self.camera.pos);
        println!("sphere: {:?} r={:?}", self.sphere.center, self.sphere.radius);
    }

    pub fn save_image(&mut self) {
        println!("saving result to {:?}", self.opt.folder);
        let time_elapsed = self.start_time.elapsed();
        println!("duration: {:?}", time_elapsed);
        self.image.save_image(PathBuf::from("./pic.ppm"));
    }
}

fn main() {
    let opt = Options::from_args();

    println!("all opt {:?}", opt);

    let mut job = RenderJob::new(opt);

    job.parse_input_scene();
    job.render_scene();
    job.save_image();
}
