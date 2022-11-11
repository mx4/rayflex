use std::path::PathBuf;
use std::fs::File;
use std::io::Write;

use crate::color::RGB;

pub struct Image {
    res_x: u32,
    res_y: u32,
    content: Vec::<RGB>,
}


impl Image {
    pub fn new(res_x: u32, res_y: u32) -> Self {
        Self {  res_x: res_x, res_y: res_y,
	content: Vec::<RGB>::with_capacity((res_x * res_y) as usize) }
    }
    pub fn push_pixel(&mut self, c: RGB) {
        self.content.push(c);
    }
    pub fn save_image(&mut self, file: PathBuf) -> std::io::Result<()> {
        println!("saving result to {:?}", file);
	let mut f = File::create(file)?;
	let mut content = format!("P3\n{} {}\n255\n", self.res_x, self.res_y);
	f.write_all(content.as_bytes())?;
	let len = self.content.len();
        assert!(len > 0);
	if len == 0 {
	    return Ok(())
	}

	println!("res: {}x{}", self.res_x, self.res_y);

	for i in 0..self.res_y {
	    for j in 0..self.res_x {
		let c = &self.content[(i * self.res_x + j) as usize];
                let rf = (255.0 * c.r).clamp(0.0, 255.0);
                let gf = (255.0 * c.g).clamp(0.0, 255.0);
                let bf = (255.0 * c.b).clamp(0.0, 255.0);
                let r = rf as u8;
                let g = gf as u8;
                let b = bf as u8;
		content = format!(" {0} {1} {2} \n", r, g, b);
		f.write_all(content.as_bytes()).expect("Unable to write data");
	    }
	}
        Ok(())
    }
}

