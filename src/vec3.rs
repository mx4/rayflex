use std::ops::{Add, AddAssign, Sub, Mul};


#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
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

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, rhs: f64) -> Vec3 {
        Vec3 { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs }
    }
}

impl Mul<Vec3> for Vec3 {
    type Output = f64;
    fn mul(self, rhs: Vec3) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
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
        (self * self).sqrt()
    }
    pub fn normalize(&mut self) {
        let norm = self.norm();
        self.x /= norm;
        self.y /= norm;
        self.z /= norm;
    }
    pub fn reflect(self, normal: Vec3) -> Vec3 {
	self - normal * (self * normal) * 2.0
    }
    pub fn vector_product(self, rhs: Vec3) -> Vec3 {
        let mut v = Vec3{
            x : self.y * rhs.z - self.z * rhs.y,
            y : self.z * rhs.x - self.x * rhs.z,
            z : self.x * rhs.y - self.y * rhs.x,
        };
        v.normalize();
        v
    }
}
