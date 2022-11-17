pub mod image;
pub mod color;
pub mod vec3;
pub mod light;
pub mod camera;
pub mod three_d;

use vec3::Vec3;
use vec3::Point;

#[derive(Debug)]
pub struct Ray {
    pub orig: Point,
    pub dir: Vec3
}

impl Ray {
    pub fn get_reflection(&self, point: Point, normal: Vec3) -> Ray {
        Ray{orig: point, dir: self.dir.reflect(normal) }
    }
}
