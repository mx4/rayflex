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
    fn intercept(&self, ray: &Ray, t : &mut f64) -> bool;
    fn get_normal(&self, point: &Point) -> Vec3;
    fn get_color(&self, point: &Point) -> RGB;
    fn get_texture_2d(&self, point: &Point) -> (f64, f64);
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
