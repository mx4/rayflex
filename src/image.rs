use crate::color::RGB;
use colored::Colorize;
use egui::Color32;
use egui::ColorImage;
use image::{Rgb, RgbImage};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

const GAMMA: f32 = 2.2;

/// Knee of the highlight-rolloff tone-map: linear radiance below this is
/// passed through unchanged (shadows/midtones keep their contrast); above
/// it, highlights compress smoothly toward 1.0. Pure Reinhard was rejected
/// here because its `L/(1+L)` term darkens midtones globally, muting an
/// otherwise well-exposed scene; this only touches the highlights.
const TONEMAP_KNEE: f32 = 0.75;

pub struct Image {
    use_gamma: bool,
    tone_map: bool,
    res_x: u32,
    res_y: u32,
    img_buffer: Arc<Mutex<ColorImage>>,
}

fn gamma_encode(linear: f32) -> f32 {
    linear.powf(1.0 / GAMMA)
}

/// Highlight-rolloff tone-map, per channel: maps linear radiance
/// [0, inf) -> [0, 1). Below the knee it is the identity, so shadows and
/// midtones are untouched; above the knee it rolls off exponentially toward
/// 1.0. Value and slope are continuous at the knee (slope 1 on both sides),
/// so there is no visible seam. Operates in linear space (composes cleanly
/// with the separate gamma-encode step). A directly-visible emitter maps to
/// ~1.0 (white) without the hard clip's flat, detail-free look.
fn tonemap_rolloff(l: f32) -> f32 {
    let k = TONEMAP_KNEE;
    if l <= k {
        l
    } else {
        k + (1.0 - k) * (1.0 - (-(l - k) / (1.0 - k)).exp())
    }
}

impl Image {
    pub fn get_img(&mut self) -> Arc<Mutex<ColorImage>> {
        self.img_buffer.clone()
    }
    pub fn new(use_gamma: bool, tone_map: bool, res_x: u32, res_y: u32) -> Self {
        Self {
            use_gamma,
            tone_map,
            res_x,
            res_y,
            img_buffer: Arc::new(Mutex::new(ColorImage::filled(
                [res_x as usize, res_y as usize],
                Color32::BLACK,
            ))),
        }
    }
    pub fn push_pixel(&mut self, x: u32, y: u32, c: RGB) {
        let mut rf = c.r;
        let mut gf = c.g;
        let mut bf = c.b;

        // Tone-map HDR radiance to [0,1) before gamma so bright emitters and
        // highlights roll off instead of hard-clipping to white. Gated to
        // path tracing (set at alloc time); classic ray-traced scenes keep
        // their original hard-clamp look.
        if self.tone_map {
            rf = tonemap_rolloff(rf);
            gf = tonemap_rolloff(gf);
            bf = tonemap_rolloff(bf);
        }

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
    pub fn save_image(&mut self, file: &Path) -> std::io::Result<()> {
        let start_time = Instant::now();

        let mut img = RgbImage::new(self.res_x, self.res_y);

        let pixels = self.img_buffer.lock().unwrap().pixels.clone();

        for y in 0..self.res_y {
            for x in 0..self.res_x {
                let c = pixels[(y * self.res_x + x) as usize];
                img.put_pixel(x, y, Rgb([c.r(), c.g(), c.b()]));
            }
        }

        img.save(file).expect("png write");
        let elapsed = start_time.elapsed();
        let lat_sec = elapsed.as_secs_f64();
        println!(
            "writing '{}' took {} sec",
            file.display().to_string().bold(),
            lat_sec
        );
        Ok(())
    }
}
