use std::ops::{Add, AddAssign, Sub, Mul, Div};
use std::fmt;


#[derive(Clone, Copy)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl fmt::Debug for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vec3: {{ x={:.3} y={:.3} z={:.3} }}", self.x, self.y, self.z)
    }
}

pub type Point = Vec3;

impl Add for Vec3 {
    type Output = Vec3;

    fn add(self, other: Vec3) -> Vec3 {
        Vec3 { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z }
    }
}

impl Sub for Vec3 {
    type Output = Vec3;

    fn sub(self, other: Vec3) -> Vec3 {
        Vec3 { x: self.x - other.x, y: self.y - other.y, z: self.z - other.z }
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, rhs: f64) -> Vec3 {
        Vec3 { x: self.x / rhs, y: self.y / rhs, z: self.z / rhs }
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, rhs: f64) -> Vec3 {
        Vec3 { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs }
    }
}

impl AddAssign<Vec3> for Vec3 {
    fn add_assign(&mut self, other: Vec3)  {
        *self = Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        };
    }
}


impl Vec3 {
    pub fn new() -> Vec3 {
       Vec3{ x: 0.0, y: 0.0, z: 0.0 }
    }
    pub fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn dot(self, rhs: Vec3) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
    pub fn normalize(self) -> Vec3 {
        let norm = self.norm();
        assert!(norm > 0.0);
        self / norm
    }
    pub fn reflect(self, normal: Vec3) -> Vec3 {
	self - normal * self.dot(normal) * 2.0
    }
    pub fn vector_product(self, rhs: Vec3) -> Vec3 {
        let v = Vec3{
            x : self.y * rhs.z - self.z * rhs.y,
            y : self.z * rhs.x - self.x * rhs.z,
            z : self.x * rhs.y - self.y * rhs.x,
        };
        v.normalize()
    }
}
