
#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub type Point = Vec3;

impl Vec3 {
    fn norm(&self) -> f64 {
        self.scalar(&self).sqrt()
    }
    pub fn scalar(&self, v: &Vec3) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }
    pub fn scale(&self, r: f64) -> Self {
        Vec3 { x: self.x * r, y: self.y * r, z: self.z * r }
    }
    pub fn normalize(&mut self) {
        let norm = self.norm();
        self.x /= norm;
        self.y /= norm;
        self.z /= norm;
    }
    pub fn create(src: &Point, dst: &Point) -> Self {
        Vec3{ x: dst.x - src.x, y: dst.y - src.y, z: dst.z - src.z }
    }
    pub fn add(&self, v: &Vec3) -> Self {
        Vec3 { x: self.x + v.x, y: self.y + v.y, z: self.z + v.z }
    }
}


