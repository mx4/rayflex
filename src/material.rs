use crate::color::RGB;
use crate::vec3::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub ks: f32,
    pub kd: RGB,
    pub shininess: u32, // 0 --> ~1000
    pub checkered: bool,
}

impl Material {
    pub fn new() -> Material {
        Material {
            kd: RGB::new(),
            ks: 0.0,
            shininess: 0,
            checkered: false,
        }
    }
    pub fn do_checker(&self, c: RGB, text2d: Vec2) -> RGB {
        assert!(self.checkered);
        let pattern = ((text2d.x * 4.0).fract() > 0.5) ^ ((text2d.y * 4.0).fract() > 0.5);
        if pattern {
            c / 3.0
        } else {
            c
        }
    }
}
