use raymax::color::RGB;
use raymax::vec3::Vec3;
use raymax::vec3::Point;
use raymax::Ray;

#[derive(Debug, Clone)]
pub struct Material {
    pub albedo: f32,
    pub reflectivity: f32,
    pub rgb: RGB,
    pub checkered: bool,
}

impl Material {
    pub fn new() -> Material {
        Material{ albedo: 0.0, rgb: RGB::new(), checkered : false, reflectivity: 0.0 }
    }
}

pub trait Object {
    fn display(&self);
    fn intercept(&self, ray: &Ray, tmin: f64, tmax: &mut f64) -> bool;
    fn get_normal(&self, point: Point) -> Vec3;
    fn get_texture_2d(&self, point: Point) -> (f64, f64);
    fn get_material(&self) -> Material;
}

#[derive(Debug)]
pub struct Sphere {
    pub name: String,
    pub center: Point,
    pub radius: f64,
    pub material: Material,
}

#[derive(Debug)]
pub struct Plane {
    pub name: String,
    pub point: Point,
    pub normal: Vec3,
    pub material: Material,
}

impl Plane {
    pub fn new(name: String, point: Point, normal: Vec3, material: Material) -> Self {
        let n = normal.normalize();
        Self { name: name, point: point, normal: n, material: material }
    }
}
impl Object for Plane {
    fn display(&self) {
        println!("{}: {:?} normal={:?}", self.name, self.point, self.normal);
    }
    fn intercept(&self, ray: &Ray, tmin: f64, tmax: &mut f64) -> bool {
        let d = ray.dir.dot(self.normal);
        if d.abs() < 0.001 {
            return false
        }
        let v = self.point - ray.orig;
        let t0 = v.dot(self.normal) / d;
        if t0 <= tmin || t0 >= *tmax {
            return false
        }
        *tmax = t0;
        true
    }
    fn get_normal(&self, _point: Point) -> Vec3 {
        self.normal
    }
    fn get_texture_2d(&self, _point: Point) -> (f64, f64) {
        (0.0, 0.0)
    }
    fn get_material(&self) -> Material {
        self.material.clone()
    }
}

impl Sphere {
    pub fn new(name: String, center: Point, radius: f64, material: Material) -> Self {
        Self { name: name, center: center, radius: radius, material: material }
    }
}

impl Object for Sphere {
    fn get_material(&self) -> Material {
        self.material.clone()
    }
    fn display(&self) {
        println!("{}: {:?} radius={:?}", self.name, self.center, self.radius);
    }
    fn get_normal(&self, point: Point) -> Vec3 {
        let normal = point - self.center;
        normal / self.radius
    }
    fn get_texture_2d(&self, point: Point) -> (f64, f64) {
        let v = (point - self.center) / self.radius;
        let x = (1.0 + v.y.atan2(v.x) / std::f64::consts::PI) * 0.5;
        let y = v.z.acos() / std::f64::consts::PI;
        ( x, y )
    }

    fn intercept(&self, ray: &Ray, tmin: f64, tmax: &mut f64) -> bool {
        let a = ray.dir.dot(ray.dir);
        let v0 = ray.orig - self.center;
        let b = 2.0 * ray.dir.dot(v0);
        let v1 = self.center - ray.orig;
        let c = v1.dot(v1) - self.radius * self.radius;

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
        if t0 >= *tmax {
            return false
        }

        *tmax = t0;
        true
    }
}
