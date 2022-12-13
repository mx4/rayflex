use crate::color::RGB;
use colored::Colorize;
use egui::Color32;
use egui::ColorImage;
use image::{Rgb, RgbImage};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

const GAMMA: f32 = 2.2;

pub struct Image {
    use_gamma: bool,
    res_x: u32,
    res_y: u32,
    img_buffer: Arc<Mutex<ColorImage>>,
}

fn gamma_encode(linear: f32) -> f32 {
    linear.powf(1.0 / GAMMA)
}

impl Image {
    pub fn provide_img_buf(&mut self, img: Arc<Mutex<ColorImage>>) {
        self.img_buffer = img;
    }
    pub fn new(use_gamma: bool, res_x: u32, res_y: u32) -> Self {
        Self {
            use_gamma,
            res_x,
            res_y,
            img_buffer: Arc::new(Mutex::new(ColorImage::new(
                [res_x as usize, res_y as usize],
                Color32::BLACK,
            ))),
        }
    }
    pub fn push_pixel(&mut self, x: u32, y: u32, c: RGB) {
        let mut rf = c.r;
        let mut gf = c.g;
        let mut bf = c.b;

        if self.use_gamma {
            rf = gamma_encode(rf);
            gf = gamma_encode(gf);
            bf = gamma_encode(bf);
        }
        let r = (255.0 * rf).clamp(0.0, 255.0) as u8;
        let g = (255.0 * gf).clamp(0.0, 255.0) as u8;
        let b = (255.0 * bf).clamp(0.0, 255.0) as u8;

        self.img_buffer.lock().unwrap().pixels[(y * self.res_x + x) as usize] =
            Color32::from_rgb(r, g, b);
    }
    pub fn save_image(&mut self, file: PathBuf) -> std::io::Result<()> {
        let start_time = Instant::now();

        let mut img = RgbImage::new(self.res_x, self.res_y);

        for y in 0..self.res_y {
            for x in 0..self.res_x {
                let c = self.img_buffer.lock().unwrap().pixels[(y * self.res_x + x) as usize];
                img.put_pixel(x, y, Rgb([c.r(), c.g(), c.b()]));
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
