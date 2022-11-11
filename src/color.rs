use std::ops::{Add, Mul};

#[derive(Debug, Clone, Copy)]
pub struct RGB {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Add for RGB {
    type Output = RGB;

    fn add(self, other: RGB) -> RGB {
        RGB { r: self.r + other.r, g: self.g + other.g, b: self.b + other.b }
    }
}

impl Mul<f64> for RGB {
    type Output = RGB;
    fn mul(self, rhs: f64) -> RGB {
        RGB { r: self.r * rhs, g: self.g * rhs, b: self.b * rhs }
    }
}

impl RGB {
    pub fn distance(&self, rhs: RGB) -> f64 {
        ((self.r - rhs.r).powi(2) + (self.g - rhs.g).powi(2) + (self.b - rhs.b).powi(2)).sqrt()
    }
    pub fn add(&mut self, rgb: &RGB)  {
        self.r += rgb.r;
        self.g += rgb.g;
        self.b += rgb.b;
    }
}
