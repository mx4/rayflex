use crate::color::RGB;
use crate::vec3::{Float, Vec2};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

/// A decoded, pre-linearized 2D image, sampled as a diffuse albedo map.
///
/// Texture images are sRGB-encoded, but the renderer shades in linear light
/// (the output stage gamma-encodes at the very end -- see image.rs). Every
/// texel is converted sRGB -> linear once at load time so `sample` can be
/// used directly wherever `kd` was, with no separate linearization step at
/// shading time.
pub struct Texture {
    width: usize,
    height: usize,
    linear: Vec<RGB>,
}

impl fmt::Debug for Texture {
    // Manual impl: a derived one would dump every texel (a Vec<RGB> of up
    // to millions of entries) whenever a Material is Debug-printed.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Texture {{ {}x{} }}", self.width, self.height)
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl Texture {
    pub fn load(path: &Path) -> Option<Arc<Texture>> {
        let img = image::open(path)
            .map_err(|e| {
                eprintln!("-- warning: failed to load texture {}: {e}", path.display());
                e
            })
            .ok()?
            .to_rgb8();
        let (width, height) = (img.width() as usize, img.height() as usize);
        let linear = img
            .pixels()
            .map(|p| {
                RGB::new(
                    srgb_to_linear(p[0] as f32 / 255.0),
                    srgb_to_linear(p[1] as f32 / 255.0),
                    srgb_to_linear(p[2] as f32 / 255.0),
                )
            })
            .collect();
        Some(Arc::new(Texture {
            width,
            height,
            linear,
        }))
    }

    /// Sample at `(u, v)` in the OBJ convention: both in `[0,1]` (tiling via
    /// wraparound outside that range), origin at the bottom-left -- the
    /// opposite of image row order, hence the `1.0 - v` flip below.
    pub fn sample(&self, u: Float, v: Float) -> RGB {
        let u = u.rem_euclid(1.0);
        let v = v.rem_euclid(1.0);
        let col = ((u * self.width as Float) as usize).min(self.width - 1);
        let row = (((1.0 - v) * self.height as Float) as usize).min(self.height - 1);
        self.linear[row * self.width + col]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    #[serde(default)]
    pub ks: RGB,
    #[serde(default)]
    pub kd: RGB,
    #[serde(default)]
    pub ke: RGB,
    #[serde(default)]
    pub shininess: f32, // 0 --> ~1000
    #[serde(default)]
    pub checkered: bool,
    /// Diffuse albedo texture (`map_Kd` from a `.mtl`), sampled in place of
    /// `kd`. `None` for JSON-declared materials and meshes without a
    /// texture -- always the case for scene-file materials, since JSON
    /// scenes have no way to reference an image file.
    #[serde(skip, default)]
    pub map_kd: Option<Arc<Texture>>,
}

impl Material {
    pub fn do_checker(&self, c: RGB, text2d: Vec2) -> RGB {
        assert!(self.checkered);
        // rem_euclid wraps negative UVs periodically -- the old .fract()
        // mapped all negatives into (-1, 0], which never passes the > 0.5
        // test, so the negative half of any plane rendered as one solid
        // block (and planes needed the +0.125 phase-shift hack, which
        // left a seam at the axis).
        let pattern = ((text2d.x * 4.0).rem_euclid(1.0) > 0.5)
            ^ ((text2d.y * 4.0).rem_euclid(1.0) > 0.5);
        if pattern { c / 3.0 } else { c }
    }

    /// Albedo at a UV: the texture if present, else the flat `kd`.
    pub fn albedo(&self, uv: Vec2) -> RGB {
        match &self.map_kd {
            Some(t) => t.sample(uv.x, uv.y),
            None => self.kd,
        }
    }
}
