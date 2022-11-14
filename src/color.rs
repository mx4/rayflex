use std::ops::{Add, Mul};

#[derive(Debug, Clone, Copy)]
pub struct RGB {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Add for RGB {
    type Output = RGB;

    fn add(self, other: RGB) -> RGB {
        assert!(self.r >= 0.0);
        assert!(self.g >= 0.0);
        assert!(self.b >= 0.0);
        assert!(other.r >= 0.0);
        assert!(other.g >= 0.0);
        assert!(other.b >= 0.0);
        RGB { r: self.r + other.r, g: self.g + other.g, b: self.b + other.b }
    }
}

impl Mul<f32> for RGB {
    type Output = RGB;
    fn mul(self, rhs: f32) -> RGB {
        assert!(self.r >= 0.0);
        assert!(self.g >= 0.0);
        assert!(self.b >= 0.0);
        assert!(rhs >= 0.0);
        RGB { r: self.r * rhs, g: self.g * rhs, b: self.b * rhs }
    }
}

impl RGB {
    pub fn new() -> RGB {
	RGB{ r: 0.0, g: 0.0, b: 0.0 }
    }
    pub fn distance2(&self, rhs: RGB) -> f32 {
        let m = f32::max((self.r - rhs.r).abs(), (self.g - rhs.g).abs());
        let m = f32::max((self.b - rhs.b).abs(), m);

        m
    }
    pub fn distance(&self, rhs: RGB) -> f32 {
        ((self.r - rhs.r).powi(2) + (self.g - rhs.g).powi(2) + (self.b - rhs.b).powi(2)).sqrt()
    }
    pub fn add(&mut self, rgb: &RGB)  {
        assert!(self.r >= 0.0);
        assert!(self.g >= 0.0);
        assert!(self.b >= 0.0);
        assert!(rgb.r >= 0.0);
        assert!(rgb.g >= 0.0);
        assert!(rgb.b >= 0.0);
        self.r += rgb.r;
        self.g += rgb.g;
        self.b += rgb.b;
    }
}
