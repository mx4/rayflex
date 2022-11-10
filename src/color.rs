#[derive(Debug, Clone, Copy)]
pub struct RGB {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}


impl RGB {
    pub fn scale(&self, f: f64) -> RGB {
        RGB { r: self.r * f, g: self.g * f, b: self.b * f }
    }
    pub fn add(&mut self, rgb: &RGB)  {
        self.r += rgb.r;
        self.g += rgb.g;
        self.b += rgb.b;
    }
}
