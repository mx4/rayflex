use serde::{Deserialize, Serialize};
use colored::Colorize;
use crate::vec3::Vec3;
use crate::vec3::Point;
use crate::Ray;

#[derive(Debug, Serialize, Deserialize)]
pub struct Camera {
    pub pos: Point,
    pub dir: Vec3,
    #[serde(skip)]
    pub screen_u: Vec3,
    #[serde(skip)]
    pub screen_v: Vec3,
}


impl Camera {
    pub fn calc_uv_after_deserialize(&mut self) {
        self.screen_u = Vec3{ x: -self.dir.y, y: self.dir.x, z: 0.0 };
        self.screen_v = self.dir.vector_product(self.screen_u);
    }
    pub fn new(pos: Point, dir: Vec3) -> Self {
        let d = dir.normalize();
        let sc_u = Vec3{ x: -d.y, y: d.x, z: 0.0 };
        let sc_v = dir.vector_product(sc_u);

        Self { pos: pos, dir: d, screen_u: sc_u, screen_v: sc_v }
    }
    // u0: -0.5 .. 0.5
    // v0: -0.5 .. 0.5
    pub fn get_ray(&self, u: f64, v: f64) -> Ray {
        let pixel = self.pos + self.dir + self.screen_u * u + self.screen_v * v;
        Ray{ orig: self.pos, dir: pixel - self.pos }
    }
    pub fn display(&self) {
        let s = format!("camera:").green();
        let s_pos = format!("pos: {:?}", self.pos).dimmed();
        let s_dir = format!("dir: {:?}", self.dir).dimmed();
        let s_u   = format!("  u: {:?}", self.screen_u).dimmed();
        let s_v   = format!("  v: {:?}", self.screen_v).dimmed();
        println!("{s} {s_pos}");
        println!("{s} {s_dir}");
        println!("{s} {s_u}");
        println!("{s} {s_v}");
    }
}


