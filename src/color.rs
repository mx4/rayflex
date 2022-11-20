use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct RGB {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl fmt::Debug for RGB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RGB: {{ r={} g={} b={} }}", self.r, self.g, self.b)
    }
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
        RGB {
            r: self.r + other.r,
            g: self.g + other.g,
            b: self.b + other.b,
        }
    }
}

impl Mul<RGB> for RGB {
    type Output = RGB;
    fn mul(self, rhs: RGB) -> RGB {
        assert!(self.r >= 0.0);
        assert!(self.g >= 0.0);
        assert!(self.b >= 0.0);
        assert!(rhs.r >= 0.0);
        assert!(rhs.g >= 0.0);
        assert!(rhs.b >= 0.0);
        RGB {
            r: self.r * rhs.r,
            g: self.g * rhs.g,
            b: self.b * rhs.b,
        }
    }
}

impl Div<f32> for RGB {
    type Output = RGB;
    fn div(self, rhs: f32) -> RGB {
        assert!(self.r >= 0.0);
        assert!(self.g >= 0.0);
        assert!(self.b >= 0.0);
        assert!(rhs >= 0.0);
        RGB {
            r: self.r / rhs,
            g: self.g / rhs,
            b: self.b / rhs,
        }
    }
}

impl Mul<f32> for RGB {
    type Output = RGB;
    fn mul(self, rhs: f32) -> RGB {
        assert!(self.r >= 0.0);
        assert!(self.g >= 0.0);
        assert!(self.b >= 0.0);
        assert!(rhs >= 0.0);
        RGB {
            r: self.r * rhs,
            g: self.g * rhs,
            b: self.b * rhs,
        }
    }
}

impl AddAssign<RGB> for RGB {
    fn add_assign(&mut self, other: RGB) {
        *self = RGB {
            r: self.r + other.r,
            g: self.g + other.g,
            b: self.b + other.b,
        };
    }
}

impl RGB {
    pub fn new() -> RGB {
        RGB {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    }
    pub fn difference(c00: RGB, c01: RGB, c10: RGB, c11: RGB) -> f32 {
        let avg = (c00 + c01 + c10 + c11) * 0.25;
        avg.distance2(c00) + avg.distance2(c01) + avg.distance2(c10) + avg.distance2(c11)
    }
    pub fn distance2(&self, rhs: RGB) -> f32 {
        let m = f32::max((self.r - rhs.r).abs(), (self.g - rhs.g).abs());
        f32::max((self.b - rhs.b).abs(), m)
    }
    pub fn distance(&self, rhs: RGB) -> f32 {
        ((self.r - rhs.r).powi(2) + (self.g - rhs.g).powi(2) + (self.b - rhs.b).powi(2)).sqrt()
    }
    pub fn add(&mut self, rgb: &RGB) {
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
