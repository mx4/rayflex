use crate::vec3::Vec3;
use crate::vec3::Point;

#[derive(Debug)]
pub struct Camera {
    pub pos: Point,
    pub dir: Vec3,
    pub screen_u: Vec3,
    pub screen_v: Vec3,
}


impl Camera {
    pub fn new(pos: Point, dir: Vec3) -> Self {
        let d = dir.normalize();
        let sc_u = Vec3{ x: -d.y, y: d.x, z: 0.0 };
        let sc_v = dir.vector_product(sc_u);

        Self { pos: pos, dir: d, screen_u: sc_u, screen_v: sc_v }
    }
    pub fn display(&self) {
        println!("camera: pos: {:?}", self.pos);
        println!("camera: dir: {:?}", self.dir);
        println!("camera:   u: {:?}", self.screen_u);
        println!("camera:   v: {:?}", self.screen_v);
    }
}


