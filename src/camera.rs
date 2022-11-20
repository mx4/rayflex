use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::Ray;
use colored::Colorize;
use serde::{Deserialize, Serialize};

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
    pub fn calc_uv(dir: Vec3, u: &mut Vec3, v: &mut Vec3) {
        *u = Vec3 {
            x: -dir.y,
            y: dir.x,
            z: 0.0,
        };
        let v0 = dir.cross(*u);
        *v = v0.normalize();
    }
    pub fn calc_uv_after_deserialize(&mut self) {
        self.dir = self.dir.normalize();
        Self::calc_uv(self.dir, &mut self.screen_u, &mut self.screen_v);
    }
    pub fn new(pos: Point, dir: Vec3) -> Self {
        let d = dir.normalize();

        let mut sc_u = Vec3::new();
        let mut sc_v = Vec3::new();

        Self::calc_uv(d, &mut sc_u, &mut sc_v);

        Self {
            pos: pos,
            dir: d,
            screen_u: sc_u,
            screen_v: sc_v,
        }
    }
    // u0: -0.5 .. 0.5
    // v0: -0.5 .. 0.5
    pub fn get_ray(&self, u: f64, v: f64) -> Ray {
        let pixel = self.pos + self.dir + self.screen_u * u + self.screen_v * v;
        Ray {
            orig: self.pos,
            dir: pixel - self.pos,
        }
    }
    pub fn display(&self) {
        let s = format!("camera:").green();
        let s_pos = format!("pos: {:?}", self.pos).dimmed();
        let s_dir = format!("dir: {:?}", self.dir).dimmed();
        let s_u = format!("  u: {:?}", self.screen_u).dimmed();
        let s_v = format!("  v: {:?}", self.screen_v).dimmed();
        println!("-- {s} {s_pos}");
        println!("-- {s} {s_dir}");
        println!("-- {s} {s_u}");
        println!("-- {s} {s_v}");
    }
}
