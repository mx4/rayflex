use serde::{Deserialize, Serialize};
use crate::color::RGB;
use crate::vec3::Vec3;
use crate::vec3::Vec2;
use crate::vec3::Point;
use crate::Ray;
use crate::RenderStats;


pub const EPSILON : f64 = 0.000001;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub albedo: f32,
    pub reflectivity: f32,
    pub rgb: RGB,
    pub checkered: bool,
}

impl Material {
    pub fn new() -> Material {
        Material{ albedo: 0.0, rgb: RGB::new(), checkered : false, reflectivity: 0.0 }
    }
    pub fn do_checker(&self, c: RGB, text2d: Vec2) -> RGB {
        assert!(self.checkered);
        let pattern = ((text2d.x * 4.0).fract() > 0.5) ^ ((text2d.y * 4.0).fract() > 0.5);
        if pattern {
            c / 3.0
        } else {
            c
        }
    }
}

pub trait Object {
    fn display(&self);
    fn intercept(&self, stats: &mut RenderStats, ray: &Ray, tmin: f64, tmax: &mut f64, oid: &mut usize) -> bool;
    fn get_normal(&self, point: Point, oid: usize) -> Vec3;
    fn get_texture_2d(&self, point: Point) -> Vec2;
    fn get_material_id(&self) -> usize;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sphere {
    pub center: Point,
    pub radius: f64,
    pub material_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plane {
    pub point: Point,
    pub normal: Vec3,
    pub material_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triangle {
    pub points: [Point; 3],
    pub material_id: usize,
    #[serde(skip)]
    pub normal: Vec3,
    #[serde(skip)]
    pub has_normal: bool,
}

pub struct Mesh {
    pub material_id: usize,
    pub triangles: Vec<Triangle>,
}


impl Triangle {
    pub fn new(points: [Point; 3], material_id: usize) -> Self {
        Self { points: points, normal: Vec3::new(), material_id: material_id, has_normal: false }
    }
    pub fn calc_normal(&mut self) {
        self.normal = self.get_normal(Point::new(), 0);
        self.has_normal = true;
    }
}

impl Plane {
    pub fn new(point: Point, normal: Vec3, material_id: usize) -> Self {
        let n = normal.normalize();
        Self { point: point, normal: n, material_id: material_id }
    }
}
impl Object for Plane {
    fn display(&self) {
        println!("plane: {:?} normal={:?}", self.point, self.normal);
    }
    fn intercept(&self, stats: &mut RenderStats, ray: &Ray, tmin: f64, tmax: &mut f64, _oid: &mut usize) -> bool {
        stats.num_intersects_plane += 1;
        let d = ray.dir.dot(self.normal);
        if d.abs() < 0.001 {
            return false
        }
        let v = self.point - ray.orig;
        let t0 = v.dot(self.normal) / d;
        if t0 <= tmin || t0 >= *tmax {
            return false
        }
        *tmax = t0;
        true
    }
    fn get_normal(&self, _point: Point, _oid: usize) -> Vec3 {
        self.normal
    }
    fn get_texture_2d(&self, _point: Point) -> Vec2 {
        Vec2{ x: 0.0, y: 0.0 }
    }
    fn get_material_id(&self) -> usize {
        self.material_id
    }
}

impl Sphere {
    pub fn new(center: Point, radius: f64, material_id: usize) -> Self {
        Self { center: center, radius: radius, material_id: material_id }
    }
}

impl Object for Sphere {
    fn get_material_id(&self) -> usize {
        self.material_id
    }
    fn display(&self) {
        println!("sphere: {:?} radius={:?}", self.center, self.radius);
    }
    fn get_normal(&self, point: Point, _oid: usize) -> Vec3 {
        let normal = point - self.center;
        normal / self.radius
    }
    fn get_texture_2d(&self, point: Point) -> Vec2 {
        let v = (point - self.center) / self.radius;
        let x = (1.0 + v.y.atan2(v.x) / std::f64::consts::PI) * 0.5;
        let y = v.z.acos() / std::f64::consts::PI;
        Vec2{
            x: x as f32,
            y: y as f32,
        }
    }

    fn intercept(&self, stats: &mut RenderStats, ray: &Ray, tmin: f64, tmax: &mut f64, _oid: &mut usize) -> bool {
        stats.num_intersects_sphere += 1;
        let a = ray.dir.dot(ray.dir);
        let v0 = ray.orig - self.center;
        let b = 2.0 * ray.dir.dot(v0);
        let v1 = self.center - ray.orig;
        let c = v1.dot(v1) - self.radius * self.radius;

        let delta = b * b - 4.0 * a * c;

        if delta < 0.0 {
            return false
        }
        let delta_sqrt = delta.sqrt();
        let t1 = (-b + delta_sqrt) / (2.0 * a);
        let t2 = (-b - delta_sqrt) / (2.0 * a);
        if t1 < tmin {
            return false
        }
        let t0 : f64;
        if t2 < tmin {
            t0 = t1;
        } else {
            t0 = t2;
        }
        if t0 >= *tmax {
            return false
        }

        *tmax = t0;
        true
    }
}

impl Object for Triangle {
    fn get_material_id(&self) -> usize {
        self.material_id
    } 
    fn display(&self) {
        println!("triangle: {:?} {:?} {:?}", self.points[0], self.points[1], self.points[2]);
    }
    fn get_normal(&self, _point: Point, _oid: usize) -> Vec3 {
        if self.has_normal {
            return self.normal
        }
        let edge1 = self.points[1] - self.points[0];
        let edge2 = self.points[2] - self.points[0];
        edge1.cross(edge2).normalize()
    }
    fn get_texture_2d(&self, _point: Point) -> Vec2 {
        Vec2{ x: 0.0, y: 0.0 }
    }

    // cf wikipedia
    fn intercept(&self, stats: &mut RenderStats, ray: &Ray, tmin: f64, tmax: &mut f64, _oid: &mut usize) -> bool {
        stats.num_intersects_triangle += 1;
        let edge1 = self.points[1] - self.points[0];
        let edge2 = self.points[2] - self.points[0];
        let h = ray.dir.cross(edge2);
        let a = edge1.dot(h);
        if a.abs() < EPSILON {
            return false
        }

        let f = 1.0 / a;
        let s = ray.orig - self.points[0];
        let u = f * s.dot(h);
        if u < 0.0 || u > 1.0 {
            return false
        }

        let q = s.cross(edge1);
        let v = f * ray.dir.dot(q);
        if v < 0.0 || u + v > 1.0 {
            return false
        }

        let t = f * edge2.dot(q);
        if t < EPSILON {
            return false
        }
        if t <= tmin || t >= *tmax {
            return false
        }
        *tmax = t;
        true
    }
}

impl Object for Mesh {
    fn get_material_id(&self) -> usize {
        self.material_id
    } 
    fn display(&self) {
        println!("mesh: n={:?}", self.triangles.len());
    }
    fn get_normal(&self, _point: Point, oid: usize) -> Vec3 {
        self.triangles[oid].get_normal(_point, 0)
    }
    fn get_texture_2d(&self, _point: Point) -> Vec2 {
        Vec2{ x: 0.0, y: 0.0 }
    }

    fn intercept(&self, stats: &mut RenderStats, ray: &Ray, tmin: f64, tmax: &mut f64, oid: &mut usize) -> bool {
        let mut n = 0;
        let mut oid0 : usize = 0;

        let hit_triangle = self.triangles.iter().filter(|triangle| {
            stats.num_intersects_triangle += 1;
            let res = triangle.intercept(stats, &ray, tmin, tmax, &mut oid0);
            if res { *oid = n; }
            n += 1;
            res
        }).fold(None, |_acc, triangle| Some(triangle));

        hit_triangle.is_some()
    }
}
