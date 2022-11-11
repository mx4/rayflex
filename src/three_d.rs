use raymax::color::RGB;
use raymax::vec3::Vec3;
use raymax::vec3::Point;

#[derive(Debug)]
pub struct Ray {
    pub orig: Point,
    pub dir: Vec3
}

#[derive(Debug)]
pub struct Sphere {
    pub name: String,
    pub center: Point,
    pub radius: f64,
    pub rgb: RGB,
}

#[derive(Debug)]
pub struct Camera {
    pub pos: Point,
    pub dir: Vec3,
}

pub struct AmbientLight {
    pub name: String,
    pub rgb: RGB,
    pub intensity: f64,
}

pub struct PointLight {
    pub name: String,
    pub pos: Point,
    pub rgb: RGB,
    pub intensity: f64,
}

pub struct VectorLight {
    pub name: String,
    pub rgb: RGB,
    pub dir: Vec3,
    pub intensity: f64, // ??
}

pub trait Light {
    fn display(&self);
    fn get_vector(&self, point: &Point) -> Vec3;
    fn get_intensity(&self) -> f64;
    fn get_color(&self) -> RGB;
    fn is_ambient(&self) -> bool;
}

impl Light for AmbientLight {
    fn display(&self) {
        println!("{}: {} {:?}", self.name, self.intensity, self.rgb);
    }
    fn get_vector(&self, _point: &Point) -> Vec3 {
        Vec3 { x: 0.0, y: 0.0, z: 0.0 }
    }
    fn get_intensity(&self) -> f64 {
        self.intensity
    }
    fn get_color(&self) -> RGB {
        self.rgb
    }
    fn is_ambient(&self) -> bool {
        true
    }
}

impl Light for VectorLight {
    fn is_ambient(&self) -> bool {
        false
    }
    fn display(&self) {
        println!("{}: {} {:?} {:?}", self.name, self.intensity, self.dir, self.rgb);
    }
    fn get_vector(&self, _point: &Point) -> Vec3 {
        self.dir
    }
    fn get_intensity(&self) -> f64 {
        self.intensity
    }
    fn get_color(&self) -> RGB {
        self.rgb
    }
}

pub trait Object {
    fn display(&self);
    fn intercept(&self, ray: &Ray, t : &mut f64) -> bool;
    fn get_normal(&self, point: &Point) -> Vec3;
    fn get_color(&self, point: &Point) -> RGB;
    fn get_texture_2d(&self, point: &Point) -> (f64, f64);
}


impl Camera {
    pub fn new(pos: Point, dir: Vec3) -> Self {
        Self { pos: pos, dir: dir }
    }
}


impl Sphere {
    pub fn new(name: String, center: Point, radius: f64, rgb: RGB) -> Self {
        Self { name: name, center: center, radius: radius, rgb: rgb }
    }
}

impl Object for Sphere {
    fn display(&self) {
        println!("{}: {:?} radius={:?}", self.name, self.center, self.radius);
    }
    fn get_normal(&self, point: &Point) -> Vec3 {
        let mut normal = Vec3::create(&self.center, point);
        normal.normalize();
        normal
    }
    fn get_color(&self, _point: &Point) -> RGB {
        self.rgb
    }

    fn get_texture_2d(&self, point: &Point) -> (f64, f64) {
        let mut v = Vec3::create(&self.center, point);
        v.normalize();
        let x = (1.0 + v.y.atan2(v.x) / std::f64::consts::PI) * 0.5;
        let y = v.z.acos() / std::f64::consts::PI;
        ( x, y )
    }

    fn intercept(&self, ray: &Ray, t: &mut f64) -> bool {
        let a = ray.dir.scalar(&ray.dir);
        let v0 = Vec3::create(&self.center, &ray.orig);
        let b = 2.0 * ray.dir.scalar(&v0);
        let v1 = Vec3::create(&ray.orig, &self.center);
        let c = v1.scalar(&v1) - self.radius * self.radius;

        let delta = b * b - 4.0 * a * c;

        if delta < 0.0 {
            return false
        }
        let delta_sqrt = delta.sqrt();
        let t1 = (-b + delta_sqrt) / (2.0 * a);
        let t2 = (-b - delta_sqrt) / (2.0 * a);
        if t1 < 1.0 {
            return false
        }
        let t0 : f64;
        if t2 < 1.0 {
            t0 = t1;
        } else {
            t0 = t2;
        }
        *t = t0;

        return true
    }
}
