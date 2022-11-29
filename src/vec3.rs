use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub};
use rand::Rng;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

pub type Point = Vec3;
pub type Point2 = Vec2;

impl Default for Vec3 {
    fn default() -> Self {
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}


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

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, rhs: f64) -> Vec3 {
        Vec3 {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
        }
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, rhs: f64) -> Vec3 {
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

impl Vec2 {
    pub fn new() -> Vec2 {
        Vec2 { x: 0.0, y: 0.0 }
    }
}

pub struct Matrix3 {
    mat: [f64; 9],
}

impl Matrix3 {
    pub fn new() -> Matrix3 {
        Matrix3 {
            mat: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl Vec3 {
    pub fn zero() -> Vec3 {
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x: x, y: y, z: z }
    }
    pub fn unity_x() -> Self {
        Self {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }
    }
    pub fn unity_y() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    }
    pub fn unity_z() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }
    }
    pub fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn dot(self, rhs: Vec3) -> f64 {
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
        for i in 0..3 {
            for j in 0..3 {
                v[i] += v0[j] * matrix.mat[i + j * 3];
            }
        }
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
        }
    }
    pub fn rotx(self, alpha: f64) -> Self {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [1.0, 0.0, 0.0, 0.0, cos, -sin, 0.0, sin, cos],
        };
        self.multiply(m)
    }
    pub fn roty(self, alpha: f64) -> Self {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [cos, 0.0, sin, 0.0, 1.0, 0.0, -sin, 0.0, cos],
        };
        self.multiply(m)
    }
    pub fn rotz(self, alpha: f64) -> Self {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0],
        };
        self.multiply(m)
    }
    pub fn gen_rnd_sphere() -> Self {
        let mut rng = rand::thread_rng();
        loop {
            let v = Vec3 {
                x: rng.gen_range(-1.0..1.0),
                y: rng.gen_range(-1.0..1.0),
                z: rng.gen_range(-1.0..1.0),
            };
            if v.norm() <= 1.0 {
                return v.normalize();
            }
        };
    }
}
