#[derive(Debug, Clone, Copy)]
pub struct RGB {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}


impl RGB {
    pub fn distance(&self, rhs: RGB) -> f64 {
        ((self.r - rhs.r).powi(2) + (self.g - rhs.g).powi(2) + (self.b - rhs.b).powi(2)).sqrt()
    }
    pub fn scale(&self, f: f64) -> RGB {
        RGB { r: self.r * f, g: self.g * f, b: self.b * f }
    }
    pub fn add(&mut self, rgb: &RGB)  {
        self.r += rgb.r;
        self.g += rgb.g;
        self.b += rgb.b;
    }
}
