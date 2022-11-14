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
        let mut d = dir;
        d.normalize();
        let sc_u = Vec3{ x: -d.y, y: d.x, z: 0.0 };
        let sc_v = dir.vector_product(sc_u);

        println!("camera_u: {:?}", sc_u);
        println!("camera_v: {:?}", sc_v);
        Self { pos: pos, dir: d, screen_u: sc_u, screen_v: sc_v }
    }
}


