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

#[allow(clippy::too_many_arguments)] // intercept's params are all load-bearing
pub trait Object {
    fn display(&self);
    /// Test for intersection with `ray`.
    ///
    /// `exclude`, when `Some(sub_id)`, means: the caller wants to ignore
    /// the specific sub-primitive identified by `sub_id` (as previously
    /// reported via `oid`) -- typically because this ray originates from
    /// the surface that primitive was just hit on, and we don't want it
    /// to spuriously re-intersect itself due to floating-point error.
    ///
    /// For composite objects with real sub-primitives (`Mesh`), this
    /// skips just that one triangle, so neighboring triangles remain
    /// testable. For single-primitive objects (`Sphere`, `Plane`,
    /// `Triangle`), there is no finer-grained sub-structure to exclude,
    /// so `Some(_)` means "skip this object entirely" -- the caller is
    /// expected to pass this only when re-testing the exact object that
    /// was just hit.
    fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        any: bool,
        oid: &mut usize,
        exclude: Option<usize>,
    ) -> bool;
    fn get_normal(&self, point: Point, oid: usize) -> Vec3;
    /// UV coordinates at the hit point, for texture sampling. `sub_id` is
    /// the same hit sub-primitive index passed to `get_material_id`.
    fn get_texture_2d(&self, point: Point, sub_id: usize) -> Vec2;
    /// Material for the hit sub-primitive. `sub_id` is the value reported
    /// via `intercept`'s `oid` (a triangle index for `Mesh`; ignored by
    /// single-primitive objects).
    fn get_material_id(&self, sub_id: usize) -> usize;
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
    /// Per-vertex normals (`vn` from an OBJ), one per `points[i]`, for
    /// smooth (Phong-interpolated) shading. `None` for JSON-declared
    /// triangles and OBJ meshes without vertex normals -- those fall back
    /// to the flat geometric face normal.
    #[serde(skip)]
    pub normals: Option<[Vec3; 3]>,
    /// Per-vertex UVs (`vt` from an OBJ), one per `points[i]`, for texture
    /// sampling. `None` for JSON-declared triangles and OBJ meshes without
    /// texture coordinates.
    #[serde(skip)]
    pub uvs: Option<[Vec2; 3]>,
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
            normals: None,
            uvs: None,
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
            normals: None,
            uvs: None,
        }
    }
    /// Barycentric weights (w0, w1, w2) of `p` w.r.t. (points[0], points[1],
    /// points[2]), i.e. `p ~= points[0]*w0 + points[1]*w1 + points[2]*w2`.
    /// `p` is assumed to lie in the triangle's plane (a ray-hit point).
    fn barycentric(&self, p: Point) -> (Float, Float, Float) {
        let v0 = self.points[1] - self.points[0];
        let v1 = self.points[2] - self.points[0];
        let v2 = p - self.points[0];
        let d00 = v0.dot(v0);
        let d01 = v0.dot(v1);
        let d11 = v1.dot(v1);
        let d20 = v2.dot(v0);
        let d21 = v2.dot(v1);
        let denom = d00 * d11 - d01 * d01;
        let w1 = (d11 * d20 - d01 * d21) / denom;
        let w2 = (d00 * d21 - d01 * d20) / denom;
        let w0 = 1.0 - w1 - w2;
        (w0, w1, w2)
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
        exclude: Option<usize>,
    ) -> bool {
        if exclude.is_some() {
            return false;
        }
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
    fn get_texture_2d(&self, point: Point, _sub_id: usize) -> Vec2 {
        // Build an orthonormal tangent frame from the plane's normal so the
        // checkerboard is a proper 2D grid on ANY plane orientation. The old
        // code hardcoded the world (y, z) axes, which degenerated to 1D
        // stripes on horizontal (z-normal) planes: for a floor, v.z == 0
        // everywhere, so the second texture coordinate was constant and
        // do_checker's XOR collapsed to a single-axis test.
        let n = self.normal;
        // Helper axis: the world axis least aligned with the normal (any
        // non-parallel vector works; this choice maximizes numerical
        // stability of the cross product).
        let a = if n.x.abs() <= n.y.abs() && n.x.abs() <= n.z.abs() {
            Vec3::unity_x()
        } else if n.y.abs() <= n.z.abs() {
            Vec3::unity_y()
        } else {
            Vec3::unity_z()
        };
        let u = n.cross(a).normalize();
        let v = n.cross(u); // unit length: n and u are orthonormal
        let d = point - self.point;
        // Raw coordinates, no sign hack -- do_checker wraps negatives
        // periodically via rem_euclid, so there is no phase seam at the
        // axes (the old +0.125 flip left one).
        Vec2 {
            x: d.dot(u),
            y: d.dot(v),
        }
    }
    fn get_material_id(&self, _sub_id: usize) -> usize {
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
    fn get_material_id(&self, _sub_id: usize) -> usize {
        self.material_id
    }
    fn display(&self) {
        println!("sphere: {:?} radius={:?}", self.center, self.radius);
    }
    fn get_normal(&self, point: Point, _oid: usize) -> Vec3 {
        let normal = point - self.center;
        normal / self.radius
    }
    fn get_texture_2d(&self, point: Point, _sub_id: usize) -> Vec2 {
        let pi = std::f32::consts::PI;
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
        exclude: Option<usize>,
    ) -> bool {
        if exclude.is_some() {
            return false;
        }
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
    fn get_material_id(&self, _sub_id: usize) -> usize {
        self.material_id
    }
    fn display(&self) {
        println!(
            "triangle: {:?} {:?} {:?}",
            self.points[0], self.points[1], self.points[2]
        );
    }
    fn get_normal(&self, point: Point, _oid: usize) -> Vec3 {
        let face_normal = || {
            let edge1 = self.points[1] - self.points[0];
            let edge2 = self.points[2] - self.points[0];
            edge1.cross(edge2).normalize()
        };
        match self.normals {
            // Smooth shading: interpolate the three vertex normals
            // (barycentric weights of the hit point) and renormalize --
            // Phong/Gouraud-style. Falls back to the flat face normal for
            // a degenerate (near-zero-area) triangle.
            Some(ns) => {
                let (w0, w1, w2) = self.barycentric(point);
                let n = ns[0] * w0 + ns[1] * w1 + ns[2] * w2;
                if n.norm() > EPSILON {
                    n.normalize()
                } else {
                    face_normal()
                }
            }
            None => face_normal(),
        }
    }
    fn get_texture_2d(&self, point: Point, _sub_id: usize) -> Vec2 {
        // Same barycentric interpolation as get_normal, applied to the
        // per-vertex UVs (`vt` from an OBJ) instead of normals.
        match self.uvs {
            Some(uv) => {
                let (w0, w1, w2) = self.barycentric(point);
                Vec2 {
                    x: uv[0].x * w0 + uv[1].x * w1 + uv[2].x * w2,
                    y: uv[0].y * w0 + uv[1].y * w1 + uv[2].y * w2,
                }
            }
            None => Vec2 { x: 0.0, y: 0.0 },
        }
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
        exclude: Option<usize>,
    ) -> bool {
        if exclude.is_some() {
            return false;
        }
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
    fn get_material_id(&self, sub_id: usize) -> usize {
        // Per-triangle materials: `sub_id` is the hit triangle's index
        // (the same value get_normal uses). Multi-material meshes (e.g. an
        // OBJ with a .mtl) shade each triangle with its own material.
        self.triangles[sub_id].material_id
    }
    fn display(&self) {
        println!("mesh: n={:?}", self.triangles.len());
    }
    fn get_normal(&self, _point: Point, oid: usize) -> Vec3 {
        self.triangles[oid].get_normal(_point, 0)
    }
    fn get_texture_2d(&self, point: Point, sub_id: usize) -> Vec2 {
        self.triangles[sub_id].get_texture_2d(point, 0)
    }

    fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        any: bool,
        oid: &mut usize,
        exclude: Option<usize>,
    ) -> bool {
        self.aabb
            .intercept(stats, ray, tmin, tmax, any, oid, exclude)
    }
}
