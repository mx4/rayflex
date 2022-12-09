use crate::vec3::Float;
use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::Ray;
use colored::Colorize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Camera {
    pub pos: Point,
    pub look_at: Point,
    pub up: Vec3,
    pub vfov: Float,
    #[serde(skip)]
    pub dir: Vec3,
    #[serde(skip)]
    pub aspect: Float,
    #[serde(skip)]
    pub screen_u: Vec3,
    #[serde(skip)]
    pub screen_v: Vec3,
}

impl Camera {
    pub fn init(&mut self) {
        self.dir = (self.look_at - self.pos).normalize();
        let theta = self.vfov.to_radians();
        let half_height = (theta / 2.0).tan();
        let half_width = self.aspect * half_height;
        let u = self.up.cross(self.dir).normalize();
        let v = self.dir.cross(u).normalize();

        self.screen_u = u * 2.0 * half_width;
        self.screen_v = v * 2.0 * half_height;
    }

    pub fn new(pos: Point, look_at: Point, up: Vec3, vfov: Float, aspect: Float) -> Self {
        let mut c = Self {
            pos,
            look_at,
            screen_u: Vec3::zero(),
            screen_v: Vec3::zero(),
            dir: Vec3::zero(),
            up,
            vfov,
            aspect,
        };
        c.init();
        c
    }
    // u: -0.5 .. 0.5
    // v: -0.5 .. 0.5
    pub fn get_ray(&self, u: Float, v: Float) -> Ray {
        let pixel = self.pos + self.dir + self.screen_u * u + self.screen_v * v;
        Ray::new(self.pos, pixel - self.pos)
    }
    pub fn display(&self) {
        let s = "camera:".green();
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
