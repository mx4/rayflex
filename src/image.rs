use crate::color::RGB;
use colored::Colorize;
use egui::Color32;
use egui::ColorImage;
use image::{Rgb, Rgb32FImage, RgbImage};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

const GAMMA: f32 = 2.2;

pub struct Image {
    use_gamma: bool,
    res_x: u32,
    res_y: u32,
    img_buffer: image::Rgb32FImage,
    img_buffer2: Arc<Mutex<ColorImage>>,
}

fn gamma_encode(linear: f32) -> f32 {
    linear.powf(1.0 / GAMMA)
}

impl Image {
    pub fn set_img(&mut self, img: Arc<Mutex<ColorImage>>) {
        self.img_buffer2 = img;
    }
    pub fn new(use_gamma: bool, res_x: u32, res_y: u32) -> Self {
        Self {
            use_gamma,
            res_x,
            res_y,
            img_buffer: Rgb32FImage::new(res_x, res_y),
            img_buffer2: Arc::new(Mutex::new(ColorImage::new(
                [res_x as usize, res_y as usize],
                Color32::BLACK,
            ))),
        }
    }
    pub fn push_pixel(&mut self, x: u32, y: u32, c: RGB) {
        self.img_buffer.put_pixel(x, y, Rgb([c.r, c.g, c.b]));
        let mut rf = c.r;
        let mut gf = c.g;
        let mut bf = c.b;

        if self.use_gamma {
            rf = gamma_encode(rf);
            gf = gamma_encode(gf);
            bf = gamma_encode(bf);
        }
        let r = (rf * 255.0) as u8;
        let g = (gf * 255.0) as u8;
        let b = (bf * 255.0) as u8;

        self.img_buffer2.lock().unwrap().pixels[(y * self.res_x + x) as usize] =
            Color32::from_rgb(r, g, b);
    }
    pub fn save_image(&mut self, file: PathBuf) -> std::io::Result<()> {
        let start_time = Instant::now();

        let mut img = RgbImage::new(self.res_x, self.res_y);

        for i in 0..self.res_y {
            for j in 0..self.res_x {
                let c = self.img_buffer.get_pixel_mut(j, i);
                let mut rf = c[0];
                let mut gf = c[1];
                let mut bf = c[2];

                if self.use_gamma {
                    rf = gamma_encode(rf);
                    gf = gamma_encode(gf);
                    bf = gamma_encode(bf);
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
