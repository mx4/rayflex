use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::Ray;
use colored::Colorize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Camera {
    pub pos: Point,
    pub dir: Vec3,
    pub up: Vec3,
    pub vfov: f64,
    pub aspect: f64,
    #[serde(skip)]
    pub screen_u: Vec3,
    #[serde(skip)]
    pub screen_v: Vec3,
}

impl Camera {
    pub fn init(&mut self) {
	let theta = self.vfov.to_radians();
	let half_height = (theta / 2.0).tan();
	let half_width = self.aspect * half_height;

	let u = self.up.cross(self.dir).normalize();
	let v = self.dir.cross(u).normalize();

        self.screen_u = u * 2.0 * half_width;
        self.screen_v = v * 2.0 * half_height;
        println!("u: {:?}", self.screen_u);
        println!("v: {:?}", self.screen_v);
    }

    pub fn new(pos: Point, dir: Vec3, up: Vec3, vfov: f64, aspect: f64) -> Self {
        let d = dir.normalize();
        assert!(up.dot(d) != 0.0);

        let mut c = Self {
            pos: pos,
            dir: d,
            screen_u: Vec3::new(),
            screen_v: Vec3::new(),
            up: up,
            vfov: vfov,
            aspect: aspect,
        };
        c.init();
        c
    }
    // u: -0.5 .. 0.5
    // v: -0.5 .. 0.5
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
