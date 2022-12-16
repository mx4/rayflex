use crate::color::RGB;
use crate::vec3::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    #[serde(default)]
    pub ks: f32,
    #[serde(default)]
    pub kd: RGB,
    #[serde(default)]
    pub ke: RGB,
    #[serde(default)]
    pub shininess: f32, // 0 --> ~1000
    #[serde(default)]
    pub checkered: bool,
}

impl Material {
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
