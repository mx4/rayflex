use crate::vec3::Vec3;
use crate::vec3::Point;

#[derive(Debug)]
pub struct Camera {
    pub pos: Point,
    pub dir: Vec3,
}


impl Camera {
    pub fn new(pos: Point, dir: Vec3) -> Self {
        Self { pos: pos, dir: dir }
    }
}


