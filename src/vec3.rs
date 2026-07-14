use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub};

pub type Float = f32;
/// Minimum ray distance used to avoid self-intersection ("shadow acne").
/// Kept tiny so it doesn't reject legitimate intersections on small-scale,
/// fine-detail meshes (e.g. models with coordinates on the order of 0.1-1
/// units and many small triangles), where a larger epsilon would be
/// comparable to individual triangle sizes and cull valid hits.
pub const EPSILON: Float = 1e-6;
/// Relative scale factor for the self-intersection bias used by rays that
/// originate from a surface the primary ray just hit: reflection rays and
/// shadow (light occlusion) rays.
///
/// A single fixed absolute epsilon cannot work across scenes of very
/// different scale: floating-point error in the computed hit point grows
/// with the magnitude of its coordinates, so a value tuned for a scene
/// with coordinates around 1-10 units (e.g. cow.json) is too small for a
/// scene with coordinates around 100+ units (e.g. trolley.json), and too
/// large for a scene with coordinates around 0.1-1 units (e.g.
/// buddha.json) where it would be comparable to individual triangle sizes
/// and incorrectly cull legitimate close-range hits.
///
/// Instead, secondary_ray_epsilon() scales this factor by the magnitude
/// of the ray's origin (the hit point it was cast from), so the bias
/// stays proportionally correct regardless of scene scale.
///
/// Without this, self-intersection causes visible artifacts: dark
/// horizontal banding on reflective floors near the horizon (reflection
/// rays), and salt-and-pepper noise on curved surfaces lit by spot lights
/// (shadow rays self-shadowing the surface they left).
pub const SECONDARY_RAY_EPSILON_SCALE: Float = 1e-4;

/// Self-intersection bias for a secondary ray (reflection or shadow ray)
/// cast from `origin`. See SECONDARY_RAY_EPSILON_SCALE for rationale.
pub fn secondary_ray_epsilon(origin: Point) -> Float {
    (origin.x.abs().max(origin.y.abs()).max(origin.z.abs()) * SECONDARY_RAY_EPSILON_SCALE)
        .max(EPSILON)
}

fn u128_fold(v: u128) -> u64 {
    ((v >> 64) ^ v) as u64
}

// wyhash
// tried using the crate nanorand::WyRnd but this resulting in 5% degradation
fn fast_rand(rnd_state: &mut u64) -> u64 {
    *rnd_state = (*rnd_state).wrapping_add(0x60bee2bee120fc15);
    let mut tmp = *rnd_state as u128 * 0xa3b195354a39b70d;
    tmp = u128_fold(tmp) as u128 * 0x1b03738712fad5c9;
    u128_fold(tmp)
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct Vec3 {
    pub x: Float,
    pub y: Float,
    pub z: Float,
}

#[derive(Clone, Copy)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

pub type Point = Vec3;
pub type Point2 = Vec2;

impl fmt::Debug for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "vec3: {{ x={:.3} y={:.3} z={:.3} }}",
            self.x, self.y, self.z
        )
    }
}

impl Add for Vec3 {
    type Output = Vec3;

    fn add(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl Sub for Vec3 {
    type Output = Vec3;

    fn sub(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl Div<Float> for Vec3 {
    type Output = Vec3;
    fn div(self, rhs: Float) -> Vec3 {
        Vec3 {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
        }
    }
}

impl Div<Vec3> for Vec3 {
    type Output = Vec3;
    fn div(self, rhs: Vec3) -> Vec3 {
        Vec3 {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            z: self.z / rhs.z,
        }
    }
}

impl Mul<Float> for Vec3 {
    type Output = Vec3;
    fn mul(self, rhs: Float) -> Vec3 {
        Vec3 {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl AddAssign<Vec3> for Vec3 {
    fn add_assign(&mut self, other: Vec3) {
        *self = Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        };
    }
}

pub struct Matrix3 {
    mat: [Float; 9],
}

impl Vec3 {
    pub fn new(x: Float, y: Float, z: Float) -> Self {
        Self { x, y, z }
    }
    pub fn one() -> Self {
        Vec3::new(1.0, 1.0, 1.0)
    }
    pub fn zero() -> Self {
        Vec3::new(0.0, 0.0, 0.0)
    }
    pub fn unity_x() -> Self {
        Vec3::new(1.0, 0.0, 0.0)
    }
    pub fn unity_y() -> Self {
        Vec3::new(0.0, 1.0, 0.0)
    }
    pub fn unity_z() -> Self {
        Vec3::new(0.0, 0.0, 1.0)
    }
    pub fn norm(self) -> Float {
        self.dot(self).sqrt()
    }
    pub fn dot(self, rhs: Vec3) -> Float {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
    pub fn normalize(self) -> Self {
        let norm = self.norm();
        assert!(norm > 0.0);
        self / norm
    }
    pub fn reflect(self, normal: Vec3) -> Self {
        self - normal * self.dot(normal) * 2.0
    }
    pub fn cross(self, rhs: Vec3) -> Vec3 {
        Vec3 {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }
    pub fn multiply(self, matrix: Matrix3) -> Self {
        let v0 = [self.x, self.y, self.z];
        let mut v = [0.0; 3];
        #[allow(clippy::needless_range_loop)]
        for i in 0..3 {
            for j in 0..3 {
                v[i] += v0[j] * matrix.mat[i + j * 3];
            }
        }
        Vec3::new(v[0], v[1], v[2])
    }
    pub fn rotx(self, alpha: Float) -> Self {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [1.0, 0.0, 0.0, 0.0, cos, -sin, 0.0, sin, cos],
        };
        self.multiply(m)
    }
    pub fn roty(self, alpha: Float) -> Self {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [cos, 0.0, sin, 0.0, 1.0, 0.0, -sin, 0.0, cos],
        };
        self.multiply(m)
    }
    pub fn rotz(self, alpha: Float) -> Self {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0],
        };
        self.multiply(m)
    }
    pub fn gen_rnd_sphere(rnd_state: &mut u64) -> Self {
        let max = u64::MAX as Float;
        loop {
            let v = Vec3 {
                x: fast_rand(rnd_state) as Float / max - 0.5,
                y: fast_rand(rnd_state) as Float / max - 0.5,
                z: fast_rand(rnd_state) as Float / max - 0.5,
            };

            let n = v.norm();
            if n > EPSILON && n <= 1.0 {
                return v.normalize();
            }
        }
    }
}
