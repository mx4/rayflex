use std::sync::Arc;

use crate::Ray;
use crate::RenderStats;
use crate::aabb::AABB;
use crate::vec3::EPSILON;
use crate::vec3::Float;
use crate::vec3::Point;
use crate::vec3::Vec2;
use crate::vec3::Vec3;
use serde::{Deserialize, Serialize};

pub trait Object {
    fn display(&self);
    fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        any: bool,
        oid: &mut usize,
    ) -> bool;
    fn get_normal(&self, point: Point, oid: usize) -> Vec3;
    fn get_texture_2d(&self, point: Point) -> Vec2;
    fn get_material_id(&self) -> usize;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sphere {
    pub center: Point,
    pub radius: Float,
    pub material_id: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Plane {
    pub point: Point,
    pub normal: Vec3,
    pub material_id: usize,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Triangle {
    pub points: [Point; 3],
    pub material_id: usize,
    #[serde(skip)]
    pub mesh_id: usize,
}

pub struct Triangles {
    pub point_x: Vec<Float>,
    pub point_y: Vec<Float>,
    pub point_z: Vec<Float>,
    pub material_id: Vec<usize>,
}

impl Triangles {
    pub fn new(n: usize) -> Self {
        Self {
            point_x: Vec::with_capacity(3 * n),
            point_y: Vec::with_capacity(3 * n),
            point_z: Vec::with_capacity(3 * n),
            material_id: Vec::with_capacity(n),
        }
    }
    pub fn get_triangle(&self, idx: usize) -> Triangle {
        let p0 = Point {
            x: self.point_x[3 * idx],
            y: self.point_y[3 * idx],
            z: self.point_z[3 * idx],
        };
        let p1 = Point {
            x: self.point_x[3 * idx + 1],
            y: self.point_y[3 * idx + 1],
            z: self.point_z[3 * idx + 1],
        };
        let p2 = Point {
            x: self.point_x[3 * idx + 2],
            y: self.point_y[3 * idx + 2],
            z: self.point_z[3 * idx + 2],
        };
        Triangle {
            points: [p0, p1, p2],
            material_id: self.material_id[idx],
            mesh_id: 0,
        }
    }
}

pub struct Mesh {
    pub material_id: usize,
    pub triangles: Arc<Vec<Triangle>>,
    pub triangles_soa: Arc<Triangles>,
    pub aabb: AABB,
}

impl Mesh {
    pub fn new(triangles: Vec<Triangle>, mat_id: usize) -> Self {
        let mut triangles_soa = Triangles::new(triangles.len());
        triangles.iter().for_each(|t| {
            triangles_soa.material_id.push(t.material_id);
            t.points.iter().for_each(|p| {
                triangles_soa.point_x.push(p.x);
                triangles_soa.point_y.push(p.y);
                triangles_soa.point_z.push(p.z);
            });
        });
        let arc_triangles = Arc::new(triangles);
        let triangles_soa_arc = Arc::new(triangles_soa);
        let mut m = Mesh {
            triangles: arc_triangles.clone(),
            material_id: mat_id,
            aabb: AABB::new(arc_triangles, triangles_soa_arc.clone()),
            triangles_soa: triangles_soa_arc,
        };
        m.aabb.init();
        m
    }
}

impl Triangle {
    pub fn new(points: [Point; 3], material_id: usize) -> Self {
        Self {
            points,
            material_id,
            mesh_id: 0,
        }
    }
}

impl Plane {
    pub fn new(point: Point, normal: Vec3, material_id: usize) -> Self {
        let n = normal.normalize();
        Self {
            point,
            normal: n,
            material_id,
        }
    }
}
impl Object for Plane {
    fn display(&self) {
        println!("plane: {:?} normal={:?}", self.point, self.normal);
    }
    fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        _any: bool,
        _oid: &mut usize,
    ) -> bool {
        stats.num_intersects_plane += 1;
        let d = ray.dir.dot(self.normal);
        if d.abs() < EPSILON {
            return false;
        }
        let v = self.point - ray.orig;
        let t0 = v.dot(self.normal) / d;
        if t0 <= tmin || t0 >= *tmax {
            return false;
        }
        *tmax = t0;
        true
    }
    fn get_normal(&self, _point: Point, _oid: usize) -> Vec3 {
        self.normal
    }
    fn get_texture_2d(&self, point: Point) -> Vec2 {
        let v = point - self.point;
        let mut v_x = v.dot(Vec3::unity_y());
        let mut v_y = v.dot(Vec3::unity_z());
        if v_x < 0.0 {
            v_x = -v_x + 0.125;
        }
        if v_y < 0.0 {
            v_y = -v_y + 0.125;
        }
        Vec2 { x: v_x, y: v_y }
    }
    fn get_material_id(&self) -> usize {
        self.material_id
    }
}

impl Sphere {
    pub fn new(center: Point, radius: Float, material_id: usize) -> Self {
        Self {
            center,
            radius,
            material_id,
        }
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
        let pi = std::f64::consts::PI as Float;
        let v = (point - self.center) / self.radius;
        let x = (1.0 + v.y.atan2(v.x) / pi) * 0.5;
        let y = v.z.acos() / pi;
        Vec2 { x, y }
    }

    fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        _any: bool,
        _oid: &mut usize,
    ) -> bool {
        stats.num_intersects_sphere += 1;
        let a = ray.dir.dot(ray.dir);
        let v0 = ray.orig - self.center;
        let half_b = ray.dir.dot(v0);
        let v1 = self.center - ray.orig;
        let c = v1.dot(v1) - self.radius * self.radius;

        let delta = half_b * half_b - a * c;

        if delta < 0.0 {
            return false;
        }
        let delta_sqrt = delta.sqrt();
        let t1 = (-half_b - delta_sqrt) / a;
        let t2 = (-half_b + delta_sqrt) / a;

        let t_vals = [t1, t2];
        if let Some(t_opt) = t_vals.iter().find(|&&t| t > tmin && t < *tmax) {
            *tmax = *t_opt;
            return true;
        }

        false
    }
}

impl Object for Triangle {
    fn get_material_id(&self) -> usize {
        self.material_id
    }
    fn display(&self) {
        println!(
            "triangle: {:?} {:?} {:?}",
            self.points[0], self.points[1], self.points[2]
        );
    }
    fn get_normal(&self, _point: Point, _oid: usize) -> Vec3 {
        let edge1 = self.points[1] - self.points[0];
        let edge2 = self.points[2] - self.points[0];
        edge1.cross(edge2).normalize()
    }
    fn get_texture_2d(&self, _point: Point) -> Vec2 {
        Vec2 { x: 0.0, y: 0.0 }
    }

    // cf wikipedia
    fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        _any: bool,
        _oid: &mut usize,
    ) -> bool {
        stats.num_intersects_triangle += 1;
        let edge1 = self.points[1] - self.points[0];
        let edge2 = self.points[2] - self.points[0];
        let h = ray.dir.cross(edge2);
        let a = edge1.dot(h);
        if a.abs() < EPSILON {
            return false;
        }

        let f = 1.0 / a;
        let s = ray.orig - self.points[0];
        let u = f * s.dot(h);
        if !(0.0..=1.0).contains(&u) {
            return false;
        }

        let q = s.cross(edge1);
        let v = f * ray.dir.dot(q);
        if v < 0.0 || u + v > 1.0 {
            return false;
        }

        let t = f * edge2.dot(q);
        if t < EPSILON {
            return false;
        }
        if t <= tmin || t >= *tmax {
            return false;
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
        Vec2 { x: 0.0, y: 0.0 }
    }

    fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        any: bool,
        oid: &mut usize,
    ) -> bool {
        self.aabb.intercept(stats, ray, tmin, tmax, any, oid)
    }
}
