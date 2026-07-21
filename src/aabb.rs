use std::sync::Arc;
use std::time::Instant;

use crate::Ray;
use crate::RenderStats;
use crate::three_d::Object;
use crate::three_d::Plane;
use crate::three_d::Triangle;
use crate::three_d::Triangles;
use crate::vec3::Float;
use crate::vec3::Point;
use crate::vec3::Vec3;

const MAX_NUM_TRIANGLES: usize = 30;
const MAX_DEPTH: u32 = 8;

/// Exact triangle-vs-AABB overlap test (Akenine-Moller, 13 separating axes:
/// 3 box face normals, 1 triangle normal, 9 edge x box-axis crosses).
/// `c` is the box center, `h` the box half-extents. Complete for convex
/// shapes: no separating axis found <=> the triangle and box overlap.
fn tri_box_overlap(c: Point, h: Vec3, v0: Point, v1: Point, v2: Point) -> bool {
    // Translate so the box is centered at the origin.
    let v0 = v0 - c;
    let v1 = v1 - c;
    let v2 = v2 - c;
    let e0 = v1 - v0;
    let e1 = v2 - v1;
    let e2 = v0 - v2;

    // 9 axes: edge x box axis. For axis a = e x u, vertex projections use
    // the scalar triple product v.(e x u); the box radius projects to
    // h.|a| componentwise.
    for e in [e0, e1, e2] {
        // a = e x X = (0, e.z, -e.y)
        let p0 = v0.y * e.z - v0.z * e.y;
        let p1 = v1.y * e.z - v1.z * e.y;
        let p2 = v2.y * e.z - v2.z * e.y;
        let r = h.y * e.z.abs() + h.z * e.y.abs();
        if p0.min(p1).min(p2) > r || p0.max(p1).max(p2) < -r {
            return false;
        }
        // a = e x Y = (-e.z, 0, e.x)
        let p0 = v0.z * e.x - v0.x * e.z;
        let p1 = v1.z * e.x - v1.x * e.z;
        let p2 = v2.z * e.x - v2.x * e.z;
        let r = h.x * e.z.abs() + h.z * e.x.abs();
        if p0.min(p1).min(p2) > r || p0.max(p1).max(p2) < -r {
            return false;
        }
        // a = e x Z = (e.y, -e.x, 0)
        let p0 = v0.x * e.y - v0.y * e.x;
        let p1 = v1.x * e.y - v1.y * e.x;
        let p2 = v2.x * e.y - v2.y * e.x;
        let r = h.x * e.y.abs() + h.y * e.x.abs();
        if p0.min(p1).min(p2) > r || p0.max(p1).max(p2) < -r {
            return false;
        }
    }

    // 3 box face normals: triangle AABB vs box AABB.
    if v0.x.min(v1.x).min(v2.x) > h.x || v0.x.max(v1.x).max(v2.x) < -h.x {
        return false;
    }
    if v0.y.min(v1.y).min(v2.y) > h.y || v0.y.max(v1.y).max(v2.y) < -h.y {
        return false;
    }
    if v0.z.min(v1.z).min(v2.z) > h.z || v0.z.max(v1.z).max(v2.z) < -h.z {
        return false;
    }

    // 1 triangle normal: plane-box overlap.
    let n = e0.cross(e1);
    let s = n.dot(v0);
    let r = h.x * n.x.abs() + h.y * n.y.abs() + h.z * n.z.abs();
    if s.abs() > r {
        return false;
    }

    true
}

/*
 * Axis-Aligned Bounding Box
 */

type AABBTriangle = usize;

pub struct AABB {
    pub p_min: Point,
    pub p_max: Point,
    pub is_leaf: bool,
    pub aabbs: Option<Vec<AABB>>,
    pub triangles: Vec<AABBTriangle>,
    triangles_root: Arc<Vec<Triangle>>,
    triangles_soa: Arc<Triangles>,
}

impl AABB {
    pub fn new(triangles: Arc<Vec<Triangle>>, triangles_soa: Arc<Triangles>) -> AABB {
        Self {
            p_min: Point::zero(),
            p_max: Point::zero(),
            is_leaf: false,
            triangles: vec![],
            aabbs: None,
            triangles_root: triangles,
            triangles_soa,
        }
    }
    fn init_with_point(p_min: &mut Point, p_max: &mut Point, point: &Point) {
        p_min.x = p_min.x.min(point.x);
        p_min.y = p_min.y.min(point.y);
        p_min.z = p_min.z.min(point.z);

        p_max.x = p_max.x.max(point.x);
        p_max.y = p_max.y.max(point.y);
        p_max.z = p_max.z.max(point.z);
    }
    fn init_with_triangle(p_min: &mut Point, p_max: &mut Point, triangle: &Triangle) {
        triangle.points.iter().for_each(|p| {
            Self::init_with_point(p_min, p_max, p);
        });
    }
    fn find_bounds(&self, p_min: &mut Point, p_max: &mut Point) {
        let mut init = false;
        self.triangles_root.iter().for_each(|triangle| {
            if !init {
                *p_min = triangle.points[0];
                *p_max = triangle.points[0];
                init = true;
            }
            Self::init_with_triangle(p_min, p_max, triangle);
        });
    }
    fn triangle_inside(&self, t: &Triangle) -> bool {
        // The old test (vertex-inside-box || edge-intersects-box) dropped
        // any triangle whose *face* covers a cell without a vertex inside
        // it or an edge touching it -- those triangles were silently
        // missing from the leaf, so rays through the cell hit nothing and
        // rendered as rectangular sky-holes (verified on sponza's
        // lion-head wall: 1369 near-black pixels with the bug vs 71
        // after). SAT is complete for convex shapes, so no triangle that
        // touches the cell is ever dropped.
        let c = (self.p_min + self.p_max) * 0.5;
        let h = (self.p_max - self.p_min) * 0.5;
        tri_box_overlap(c, h, t.points[0], t.points[1], t.points[2])
    }
    fn setup_node(&mut self, p_min: Point, p_max: Point, triangles: &[AABBTriangle], depth: u32) {
        self.p_min = p_min;
        self.p_max = p_max;

        let mut v_triangles = vec![];
        if triangles.is_empty() {
            self.triangles_root
                .iter()
                .filter(|t| self.triangle_inside(t))
                .for_each(|t| v_triangles.push(t.mesh_id));
        } else {
            triangles
                .iter()
                .filter(|&&tid| self.triangle_inside(&self.triangles_root[tid]))
                .for_each(|tid| v_triangles.push(*tid));
        }

        if depth >= MAX_DEPTH || v_triangles.len() < MAX_NUM_TRIANGLES {
            self.is_leaf = true;
            self.triangles = v_triangles;
            return;
        }
        /*
         *      +---+---+
         *     / 6 / 7 /|
         *    +---+---+ +
         *   / 4 / 5 / /
         *  +---+---+ +
         *  |   |   |/
         *  +---+---+
         *
         *      +---+---+    ^ z  ^ y
         *     / 2 / 3 /|    |   /
         *    +---+---+ +    |  /
         *   / 0 / 1 / /     | /
         *  +---+---+ +      |/
         *  |   |   |/       +---------> x
         *  +---+---+
         * orig
         */
        let inc = (p_max - p_min) / 2.0;
        assert!(inc.x != 0.0 && inc.y != 0.0 && inc.z != 0.0);
        let hx = Vec3 {
            x: inc.x,
            y: 0.0,
            z: 0.0,
        };
        let hy = Vec3 {
            x: 0.0,
            y: inc.y,
            z: 0.0,
        };
        let hz = Vec3 {
            x: 0.0,
            y: 0.0,
            z: inc.z,
        };

        let mut v_min = [Point::zero(); 8];
        let mut v_max = [Point::zero(); 8];

        v_min[0] = p_min;
        v_max[0] = p_min + inc;
        v_min[1] = p_min + hx;
        v_max[1] = p_min + hx + inc;
        v_min[2] = p_min + hy;
        v_max[2] = p_min + hy + inc;
        v_min[3] = p_min + hx + hy;
        v_max[3] = p_min + hx + hy + inc;

        for i in 0..4 {
            v_min[4 + i] = v_min[i] + hz;
            v_max[4 + i] = v_max[i] + hz;
        }
        self.is_leaf = false;
        self.aabbs = Some(Vec::with_capacity(8));
        for i in 0..8 {
            let mut aabb = AABB::new(self.triangles_root.clone(), self.triangles_soa.clone());
            aabb.setup_node(v_min[i], v_max[i], &v_triangles, depth + 1);
            self.aabbs.as_mut().unwrap().push(aabb);
        }
    }
    fn count_leaves(&self) -> u32 {
        if self.is_leaf {
            return 1;
        }
        self.aabbs
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| v.count_leaves())
            .sum()
    }
    fn get_depth(&self) -> u32 {
        if self.is_leaf {
            return 0;
        }
        1 + self
            .aabbs
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| v.get_depth())
            .max()
            .unwrap()
    }
    pub fn init(&mut self) {
        let mut p_min = Vec3::zero();
        let mut p_max = Vec3::zero();
        self.find_bounds(&mut p_min, &mut p_max);

        let start_time = Instant::now();
        self.setup_node(p_min, p_max, &[], 0);
        let elapsed = start_time.elapsed();

        if elapsed.as_millis() as Float > 0.1 {
            println!(
                "-- aabb: depth: {}/{} num_leaves={} max_num_triangles={} -- {:.2} sec",
                self.get_depth(),
                MAX_DEPTH,
                self.count_leaves(),
                MAX_NUM_TRIANGLES,
                elapsed.as_millis() as Float / 1000.0
            );
        }
        //println!("-- aabb: p_min: {:?}", p_min);
        //println!("-- aabb: p_max: {:?}", p_max);
    }

    fn nearest_node(&self, p: Point, mid: Point) -> usize {
        let op = p - mid;
        let x_test = op.x.is_sign_positive();
        let y_test = op.y.is_sign_positive();
        let z_test = op.z.is_sign_positive();

        let mut v = 0;
        if x_test {
            v = 1 << 0;
        }
        if y_test {
            v += 1 << 1;
        }
        if z_test {
            v += 1 << 2;
        }
        v
    }

    pub fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: Float,
        tmax: &mut Float,
        any: bool,
        oid: &mut usize,
        exclude: Option<usize>,
    ) -> bool {
        let mut t_aabb = *tmax;

        if self.is_leaf && self.triangles.is_empty() {
            return false;
        }
        stats.num_intersects_aabb += 1;
        if !self.check_intersect(ray, *tmax, &mut t_aabb) {
            return false;
        }

        /*
         * check_intersect reports the ENTRY distance, which is negative when
         * the ray starts inside this node's box (it still returns true there,
         * since it tests t_max >= t_min.max(0)). Such a ray does traverse the
         * node, so clamp the entry to tmin rather than discarding it --
         * returning false here silently skipped the whole subtree for any ray
         * originating inside the box. That hit (a) every ray once meshes are
         * merged, because the root box then encloses the camera, and (b)
         * every secondary/bounce ray leaving a surface inside its own mesh's
         * box, which quietly cost meshes their self-occlusion.
         *
         * t_aabb is used below as the entry point (to pick the first octant
         * via nearest_node, and to reject plane crossings behind the entry),
         * so tmin is exactly the right value when the origin is inside.
         */
        if t_aabb < tmin {
            t_aabb = tmin;
        }

        let mut oid0 = 0;
        let mut hit = false;

        if self.is_leaf {
            for triangle_id in &self.triangles {
                if exclude == Some(*triangle_id) {
                    continue;
                }
                let t = self.triangles_soa.get_triangle(*triangle_id);
                if t.intercept(stats, ray, tmin, tmax, any, &mut oid0, None) {
                    hit = true;
                    *oid = *triangle_id;
                    if any {
                        break;
                    }
                }
            }
            return hit;
        } else {
            let mid = (self.p_max + self.p_min) / 2.0;
            let plane_yz = Plane::new(mid, Vec3::unity_x(), 0);
            let plane_xz = Plane::new(mid, Vec3::unity_y(), 0);
            let plane_xy = Plane::new(mid, Vec3::unity_z(), 0);
            let mut close_idx = self.nearest_node(ray.orig + ray.dir * t_aabb, mid);
            let mut tmin0 = tmin;

            for _i in 0..4 {
                if self.aabbs.as_ref().unwrap()[close_idx]
                    .intercept(stats, ray, tmin, tmax, any, oid, exclude)
                {
                    return true;
                }

                let mut t_yz = Float::MAX;
                let mut t_xz = t_yz;
                let mut t_xy = t_yz;
                let mut p = [false; 3];

                p[0] = plane_yz.intercept(stats, ray, tmin0, &mut t_yz, false, &mut oid0, None);
                p[1] = plane_xz.intercept(stats, ray, tmin0, &mut t_xz, false, &mut oid0, None);
                p[2] = plane_xy.intercept(stats, ray, tmin0, &mut t_xy, false, &mut oid0, None);

                p[0] = p[0] && t_yz > t_aabb;
                p[1] = p[1] && t_xz > t_aabb;
                p[2] = p[2] && t_xy > t_aabb;

                // if the intersection is before the aabb, discard
                if t_yz <= t_aabb {
                    t_yz = Float::MAX;
                }
                if t_xy <= t_aabb {
                    t_xy = Float::MAX;
                }
                if t_xz <= t_aabb {
                    t_xz = Float::MAX;
                }

                p[0] = p[0] && t_yz <= t_xz && t_yz <= t_xy;
                p[1] = p[1] && t_xz <= t_yz && t_xz <= t_xy;
                p[2] = p[2] && t_xy <= t_xz && t_xy <= t_yz;

                if !p.iter().any(|&x| x) {
                    break;
                }

                tmin0 = t_yz.min(t_xy).min(t_xz);
                close_idx ^= 1 << p.iter().position(|&x| x).unwrap();
            }
        }
        hit
    }

    // https://tavianator.com/cgit/dimension.git/tree/libdimension/bvh/bvh.c#n194
    fn check_intersect(&self, ray: &Ray, tmax: Float, t: &mut Float) -> bool {
        let tx1 = (self.p_min.x - ray.orig.x) * ray.inv_dir.x;
        let tx2 = (self.p_max.x - ray.orig.x) * ray.inv_dir.x;

        let ty1 = (self.p_min.y - ray.orig.y) * ray.inv_dir.y;
        let ty2 = (self.p_max.y - ray.orig.y) * ray.inv_dir.y;

        let tz1 = (self.p_min.z - ray.orig.z) * ray.inv_dir.z;
        let tz2 = (self.p_max.z - ray.orig.z) * ray.inv_dir.z;

        let tx_min = tx1.min(tx2);
        let tx_max = tx1.max(tx2);

        let ty_min = ty1.min(ty2);
        let ty_max = ty1.max(ty2);

        let tz_min = tz1.min(tz2);
        let tz_max = tz1.max(tz2);

        let mut t_min = tx_min.max(ty_min);
        let mut t_max = tx_max.min(ty_max);

        t_min = t_min.max(tz_min);
        t_max = t_max.min(tz_max);

        if t_max >= t_min.max(0.0) && t_min < tmax {
            *t = t_min;
            return true;
        }
        false
    }
}
