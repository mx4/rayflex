use raymax::color::RGB;
use raymax::vec3::Vec3;
use raymax::vec3::Point;
use raymax::Ray;

#[derive(Debug)]
pub struct Sphere {
    pub name: String,
    pub center: Point,
    pub radius: f64,
    pub rgb: RGB,
}

pub trait Object {
    fn display(&self);
    fn intercept(&self, ray: &Ray, tmin: f64, tmax: f64, t : &mut f64) -> bool;
    fn get_normal(&self, point: Point) -> Vec3;
    fn get_color(&self, point: Point) -> RGB;
    fn get_texture_2d(&self, point: Point) -> (f64, f64);
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
    fn get_normal(&self, point: Point) -> Vec3 {
        let normal = point - self.center;
        normal * (1.0 / self.radius)
    }
    fn get_color(&self, _point: Point) -> RGB {
        self.rgb
    }

    fn get_texture_2d(&self, point: Point) -> (f64, f64) {
        let v = (point - self.center) * (1.0 / self.radius);
        let x = (1.0 + v.y.atan2(v.x) / std::f64::consts::PI) * 0.5;
        let y = v.z.acos() / std::f64::consts::PI;
        ( x, y )
    }

    fn intercept(&self, ray: &Ray, tmin: f64, tmax: f64, t: &mut f64) -> bool {
        let a = ray.dir * ray.dir;
        let v0 = ray.orig - self.center;
        let b = ray.dir * 2.0 * v0;
        let v1 = self.center - ray.orig;
        let c = v1 * v1 - self.radius * self.radius;

        let delta = b * b - 4.0 * a * c;

        if delta < 0.0 {
            return false
        }
        let delta_sqrt = delta.sqrt();
        let t1 = (-b + delta_sqrt) / (2.0 * a);
        let t2 = (-b - delta_sqrt) / (2.0 * a);
        if t1 < tmin {
            return false
        }
        let t0 : f64;
        if t2 < tmin {
            t0 = t1;
        } else {
            t0 = t2;
        }
        *t = t0;

        t0 < tmax
    }
}
