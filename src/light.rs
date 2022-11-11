use crate::color::RGB;
use crate::vec3::Vec3;
use crate::vec3::Point;

pub struct AmbientLight {
    pub name: String,
    pub rgb: RGB,
    pub intensity: f64,
}

pub struct SpotLight {
    pub name: String,
    pub pos: Point,
    pub rgb: RGB,
    pub intensity: f64,
}

pub struct VectorLight {
    pub name: String,
    pub rgb: RGB,
    pub dir: Vec3,
    pub intensity: f64, // ??
}

pub trait Light {
    fn display(&self);
    fn get_vector(&self, point: &Point) -> Vec3;
    fn get_intensity(&self) -> f64;
    fn get_color(&self) -> RGB;
    fn is_ambient(&self) -> bool;
}

impl Light for SpotLight {
    fn display(&self) {
        println!("{}: {} {:?} {:?}", self.name, self.intensity, self.pos, self.rgb);
    }
    fn get_vector(&self, point: &Point) -> Vec3 {
        let mut v = Vec3::create(&self.pos, point);
        v.normalize();
        v
    }
    fn get_intensity(&self) -> f64 {
        self.intensity
    }
    fn get_color(&self) -> RGB {
        self.rgb
    }
    fn is_ambient(&self) -> bool {
        false
    }
}

impl Light for AmbientLight {
    fn display(&self) {
        println!("{}: {} {:?}", self.name, self.intensity, self.rgb);
    }
    fn get_vector(&self, _point: &Point) -> Vec3 {
        Vec3 { x: 0.0, y: 0.0, z: 0.0 }
    }
    fn get_intensity(&self) -> f64 {
        self.intensity
    }
    fn get_color(&self) -> RGB {
        self.rgb
    }
    fn is_ambient(&self) -> bool {
        true
    }
}

impl Light for VectorLight {
    fn is_ambient(&self) -> bool {
        false
    }
    fn display(&self) {
        println!("{}: {} {:?} {:?}", self.name, self.intensity, self.dir, self.rgb);
    }
    fn get_vector(&self, _point: &Point) -> Vec3 {
        self.dir
    }
    fn get_intensity(&self) -> f64 {
        self.intensity
    }
    fn get_color(&self) -> RGB {
        self.rgb
    }
}

