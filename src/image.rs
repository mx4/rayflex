use crate::color::RGB;
use colored::Colorize;
use image::{Rgb, Rgb32FImage, RgbImage};
use std::path::PathBuf;
use std::time::Instant;

const GAMMA: f32 = 2.2;

pub struct Image {
    res_x: u32,
    res_y: u32,
    img_buffer: image::Rgb32FImage,
}

fn gamma_encode(linear: f32) -> f32 {
    linear.powf(1.0 / GAMMA)
}

impl Image {
    pub fn new(res_x: u32, res_y: u32) -> Self {
        Self {
            res_x: res_x,
            res_y: res_y,
            img_buffer: Rgb32FImage::new(res_x, res_y),
        }
    }
    pub fn push_pixel(&mut self, x: u32, y: u32, c: RGB) {
        self.img_buffer.put_pixel(x, y, Rgb([c.r, c.g, c.b]));
    }
    pub fn save_image(&mut self, file: PathBuf, use_gamma: bool) -> std::io::Result<()> {
        let start_time = Instant::now();

        let mut img = RgbImage::new(self.res_x, self.res_y);

        for i in 0..self.res_y {
            for j in 0..self.res_x {
                let c = self.img_buffer.get_pixel_mut(j, i);
                let mut rf = c[0];
                let mut gf = c[1];
                let mut bf = c[2];

                if use_gamma {
                    rf = gamma_encode(c[0]);
                    gf = gamma_encode(c[1]);
                    bf = gamma_encode(c[2]);
                }
                let r = (255.0 * rf).clamp(0.0, 255.0) as u8;
                let g = (255.0 * gf).clamp(0.0, 255.0) as u8;
                let b = (255.0 * bf).clamp(0.0, 255.0) as u8;

                img.put_pixel(j, i, Rgb([r, g, b]));
            }
        }
        img.save(file.clone()).expect("png write");
        let elapsed = start_time.elapsed();
        let lat_msec = elapsed.as_millis() as f64 / 1000.0;
        println!(
            "writing '{}' took {} sec",
            file.display().to_string().bold(),
            lat_msec
        );
        Ok(())
    }
}
