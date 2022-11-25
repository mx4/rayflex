use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub};

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
    pub fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x: x, y: y, z: z }
    }
    pub fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn dot(self, rhs: Vec3) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
    pub fn normalize(self) -> Vec3 {
        let norm = self.norm();
        assert!(norm > 0.0);
        self / norm
    }
    pub fn reflect(self, normal: Vec3) -> Vec3 {
        self - normal * self.dot(normal) * 2.0
    }
    pub fn cross(self, rhs: Vec3) -> Vec3 {
        Vec3 {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }
    pub fn multiply(self, matrix: Matrix3) -> Vec3 {
        let v0 = [self.x, self.y, self.z];
        let mut v = [0.0, 0.0, 0.0];
        for i in 0..3 {
            for j in 0..3 {
                v[i] += v0[j] * matrix.mat[i + j * 3];
            }
        }
        Vec3 {
            x: v[0],
            y: v[1],
            z: v[2],
        }
    }
    pub fn rotx(self, alpha: f64) -> Vec3 {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [1.0, 0.0, 0.0, 0.0, cos, -sin, 0.0, sin, cos],
        };
        self.multiply(m)
    }
    pub fn roty(self, alpha: f64) -> Vec3 {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [cos, 0.0, sin, 0.0, 1.0, 0.0, -sin, 0.0, cos],
        };
        self.multiply(m)
    }
    pub fn rotz(self, alpha: f64) -> Vec3 {
        let cos = alpha.cos();
        let sin = alpha.sin();
        let m = Matrix3 {
            mat: [cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0],
        };
        self.multiply(m)
    }
}
