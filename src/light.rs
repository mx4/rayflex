use crate::color::RGB;
use crate::three_d::Material;
use crate::vec3::Point;
use crate::vec3::Vec3;
use colored::Colorize;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AmbientLight {
    pub rgb: RGB,
    pub intensity: f32,
}

#[derive(Serialize, Deserialize)]
pub struct SpotLight {
    #[serde(skip)]
    pub name: String,
    pub pos: Point,
    pub rgb: RGB,
    pub intensity: f32,
}

#[derive(Serialize, Deserialize)]
pub struct VectorLight {
    #[serde(skip)]
    pub name: String,
    pub dir: Vec3,
    pub rgb: RGB,
    pub intensity: f32,
}

pub trait Light {
    fn display(&self);
    fn get_vector(&self, point: Point) -> Vec3;
    fn get_intensity(&self) -> f32;
    fn get_color(&self) -> RGB;
    fn is_ambient(&self) -> bool;
    fn is_vector(&self) -> bool;
    fn is_spot(&self) -> bool;
    fn get_contrib(&self, mat: &Material, obj_point: Point, obj_normal: Vec3) -> RGB;
}

impl Light for SpotLight {
    fn get_contrib(&self, mat: &Material, obj_point: Point, obj_normal: Vec3) -> RGB {
        let c_res = mat.rgb * self.rgb * self.intensity;
        let light_vec = self.pos - obj_point;
        let dist_sq = light_vec.dot(light_vec);
        let light_vec_norm = light_vec / dist_sq.sqrt();
        let v_prod = obj_normal.dot(light_vec_norm).max(0.0) as f32;
        let pi = std::f32::consts::PI;

        c_res * v_prod.powi(4) / (1.0 + 4.0 * pi * dist_sq as f32)
    }
    fn display(&self) {
        let s = format!("{:3} {:?} {:?}", self.intensity, self.pos, self.rgb).dimmed();
        println!("-- {:12}: {s}", self.name.blue());
    }
    fn get_vector(&self, point: Point) -> Vec3 {
        point - self.pos
    }
    fn get_intensity(&self) -> f32 {
        assert!(self.intensity >= 0.0);
        self.intensity
    }
    fn get_color(&self) -> RGB {
        assert!(self.rgb.r >= 0.0);
        assert!(self.rgb.g >= 0.0);
        assert!(self.rgb.b >= 0.0);
        self.rgb
    }
    fn is_ambient(&self) -> bool {
        false
    }
    fn is_vector(&self) -> bool {
        false
    }
    fn is_spot(&self) -> bool {
        true
    }
}

impl Light for AmbientLight {
    fn get_contrib(&self, mat: &Material, _obj_point: Point, _obj_normal: Vec3) -> RGB {
        mat.rgb * self.rgb * self.intensity
    }
    fn display(&self) {
        let s = format!("{:3} {:?}", self.intensity, self.rgb).dimmed();
        println!("-- {:12}: {s}", "ambient".blue());
    }
    fn get_vector(&self, _point: Point) -> Vec3 {
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
    fn get_intensity(&self) -> f32 {
        self.intensity
    }
    fn get_color(&self) -> RGB {
        self.rgb
    }
    fn is_ambient(&self) -> bool {
        true
    }
    fn is_vector(&self) -> bool {
        false
    }
    fn is_spot(&self) -> bool {
        false
    }
}

impl Light for VectorLight {
    fn get_contrib(&self, mat: &Material, obj_point: Point, obj_normal: Vec3) -> RGB {
        let c_res = mat.rgb * self.rgb * self.intensity;
        let light_vec = self.get_vector(obj_point) * -1.0;
        let v_prod = obj_normal.dot(light_vec).min(0.0) as f32;

        c_res * v_prod.powi(4)
    }
    fn is_ambient(&self) -> bool {
        false
    }
    fn display(&self) {
        let s = format!("{:3} {:?} {:?}", self.intensity, self.dir, self.rgb).dimmed();
        println!("-- {:12}: {s}", self.name.blue());
    }
    fn get_vector(&self, _point: Point) -> Vec3 {
        self.dir * -1.0
    }
    fn get_intensity(&self) -> f32 {
        self.intensity
    }
    fn get_color(&self) -> RGB {
        self.rgb
    }
    fn is_vector(&self) -> bool {
        true
    }
    fn is_spot(&self) -> bool {
        false
    }
}
