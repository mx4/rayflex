
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug)]
pub struct Vector {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug)]
pub struct Ray {
    pub orig: Point,
    pub dir: Vector
}

#[derive(Debug)]
pub struct Sphere {
    pub name: String,
    pub center: Point,
    pub radius: f64,
}

#[derive(Debug)]
pub struct Camera {
    pub pos: Point,
    pub dir: Vector,
}

pub trait Object {
    fn display(&self);
    fn intercept(&self, ray: &Ray, t : &mut f64) -> bool;
    fn get_normal(&self, point: &Point) -> Vector;
}


impl Point {
    pub fn add(&self, v: &Vector) -> Self {
        Point { x: self.x + v.x, y: self.y + v.y, z: self.z + v.z }
    }
}

impl Vector {
    fn norm(&self) -> f64 {
        self.scalar(&self).sqrt()
    }
    pub fn scalar(&self, v: &Vector) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }
    pub fn scale(&self, r: f64) -> Self {
        Vector { x: self.x * r, y: self.y * r, z: self.z * r }
    }
    pub fn normalize(&mut self) {
        let norm = self.norm();
        self.x /= norm;
        self.y /= norm;
        self.z /= norm;
    }
    pub fn create(src: &Point, dst: &Point) -> Self {
        Vector{ x: dst.x - src.x, y: dst.y - src.y, z: dst.z - src.z }
    }
}

impl Camera {
    pub fn new(pos: Point, dir: Vector) -> Self {
        Self { pos: pos, dir: dir }
    }
}


impl Sphere {
    pub fn new(name: String, center: Point, radius: f64) -> Self {
        Self { name: name, center: center, radius: radius }
    }
}

impl Object for Sphere {
    fn display(&self) {
        println!("{}: {:?} radius={:?}", self.name, self.center, self.radius);
    }
    fn get_normal(&self, point: &Point) -> Vector {
        let mut normal = Vector::create(&self.center, point);
        normal.normalize();
        normal
    }
    fn intercept(&self, ray: &Ray, t: &mut f64) -> bool {
        let a = ray.dir.scalar(&ray.dir);
        let v0 = Vector::create(&self.center, &ray.orig);
        let b = 2.0 * ray.dir.scalar(&v0);
        let v1 = Vector::create(&ray.orig, &self.center);
        let c = v1.scalar(&v1) - self.radius * self.radius;

        let delta = b * b - 4.0 * a * c;

        if delta < 0.0 {
            return false
        }
        let delta_sqrt = delta.sqrt();
        let t1 = (-b + delta_sqrt) / (2.0 * a);
        let t2 = (-b - delta_sqrt) / (2.0 * a);
        if t1 < 0.0 {
            return false
        }
        let t0 : f64;
        if t2 < 0.0 {
            t0 = t1;
        } else {
            t0 = t2;
        }
        *t = t0;

        return true
    }
}
