//#![allow(unused)]

use structopt::StructOpt;
use std::path::PathBuf;
use std::time::Instant;
use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug)]
struct Vector {
    x: f64,
    y: f64,
    z: f64,
}

impl Point {
    fn add(&self, v: &Vector) -> Self {
        Point { x: self.x + v.x, y: self.y + v.y, z: self.z + v.z }
    }
}

impl Vector {
    fn norm(&self) -> f64 {
        self.scalar(&self).sqrt()
    }
    fn scalar(&self, v: &Vector) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }
    fn scale(&self, r: f64) -> Self {
        Vector { x: self.x * r, y: self.y * r, z: self.z * r }
    }
    fn normalize(&mut self) {
        let norm = self.norm();
        self.x /= norm;
        self.y /= norm;
        self.z /= norm;
    }
    fn create(src: &Point, dst: &Point) -> Self {
        Vector{ x: dst.x - src.x, y: dst.y - src.y, z: dst.z - src.z }
    }
}

#[derive(Debug)]
struct Ray {
    orig: Point,
    dir: Vector
}

#[derive(Debug)]
struct Sphere {
    center: Point,
    radius: f64,
}

impl Sphere {
    pub fn new(center: Point, radius: f64) -> Self {
        Self { center: center, radius: radius }
    }
    fn intercept(&mut self, ray: &Ray) -> Option<Vector> {
        let a = ray.dir.scalar(&ray.dir);
        let v0 = Vector::create(&self.center, &ray.orig);
        let b = 2.0 * ray.dir.scalar(&v0);
        let v1 = Vector::create(&ray.orig, &self.center);
        let c = v1.scalar(&v1) - self.radius.powi(2);

        let delta = b * b - 4.0 * a * c;

        if delta < 0.0 {
            return None
        }
        let t1 = (-b + delta.sqrt()) / (2.0 * a);
        let t2 = (-b - delta.sqrt()) / (2.0 * a);
        if t1 < 0.0 && t2 < 0.0 {
            return None
        }
        let scaled_dir = ray.dir.scale(t2);
        let p = ray.orig.add(&scaled_dir);

        let mut normal = Vector::create(&self.center, &p);
        normal.normalize();

        return Some(normal)
    }
}

#[derive(Debug)]
struct RGB {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Debug)]
struct Camera {
    pos: Point,
    dir: Vector,
}

impl Camera {
    fn new(pos: Point, dir: Vector) -> Self {
        Self { pos: pos, dir: dir }
    }
}

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

struct Image {
    res_x: u32,
    res_y: u32,
    content: Vec::<RGB>,
}

impl Image {
    fn new(res_x: u32, res_y: u32) -> Self {
        Self {  res_x: res_x, res_y: res_y,
            content: Vec::<RGB>::with_capacity((res_x * res_y) as usize) }
    }
    fn save_image(&mut self, file: PathBuf) {
        let mut f = File::create(file).expect("File create error");
        let mut content = format!("P3\n{} {}\n255\n", self.res_x, self.res_y);
        f.write_all(content.as_bytes()).expect("Unable to write data");
        let len = self.content.len();
        if len == 0 {
            return;
        }

        println!("vec has len {:?}", len);
        println!("res: {}x{}", self.res_x, self.res_y);

        for i in 0..self.res_y {
            for j in 0..self.res_x {
                let c = &self.content[(i * self.res_x + j) as usize];
                content = format!(" {0} {1} {2} \n", c.r, c.g, c.b);
                f.write_all(content.as_bytes()).expect("Unable to write data");
            }
        }
    }
}

struct RenderJob {
    opt: Options,
    start_time: Instant,
    camera: Camera,
    sphere: Sphere,
    image: Image,
    light: Vector,
}

impl RenderJob {
    pub fn new(opt: Options) -> Self {
        Self {
            start_time : Instant::now(),
            camera: Camera::new(Point  { x: 0.0, y: 0.0, z: 0.0 },
                                Vector { x: 1.0, y: 0.0, z: 0.0 }),
            sphere: Sphere::new(Point  { x: 3.0, y: 0.0, z: 0.0 }, 0.6 ),
            image: Image::new(opt.res_x, opt.res_y),
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

                let mut c = RGB{ r: 0, g: 0, b: 0 };
                if let Some(normal) = self.sphere.intercept(&ray) {
                    let v : f64 = 200.0 * normal.scalar(&self.light).powi(2);
                    let v8 : u8 = v as u8;
                    c = RGB{ r: v8, g: v8, b: v8 };
                    n += 1;
                } 

                self.image.content.push(c);
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
