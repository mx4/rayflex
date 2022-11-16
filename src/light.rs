use colored::Colorize;
use crate::color::RGB;
use crate::vec3::Vec3;
use crate::vec3::Point;

pub struct AmbientLight {
    pub name: String,
    pub rgb: RGB,
    pub intensity: f32,
}

pub struct SpotLight {
    pub name: String,
    pub pos: Point,
    pub rgb: RGB,
    pub intensity: f32,
}

pub struct VectorLight {
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
}

impl Light for SpotLight {
    fn display(&self) {
        let s = format!("{:3} {:?} {:?}", self.intensity, self.pos, self.rgb).dimmed();
        println!("{:12}: {s}", self.name.blue());
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
    fn display(&self) {
        let s = format!("{:3} {:?}", self.intensity, self.rgb).dimmed();
        println!("{:12}: {s}", self.name.blue());
    }
    fn get_vector(&self, _point: Point) -> Vec3 {
        Vec3 { x: 0.0, y: 0.0, z: 0.0 }
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
    fn is_ambient(&self) -> bool {
        false
    }
    fn display(&self) {
        let s = format!("{:3} {:?} {:?}", self.intensity, self.dir, self.rgb).dimmed();
        println!("{:12}: {s}", self.name.blue());
    }
    fn get_vector(&self, _point: Point) -> Vec3 {
        self.dir
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

