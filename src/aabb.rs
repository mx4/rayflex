use std::sync::Arc;
use std::time::Instant;

use crate::three_d::Object;
use crate::three_d::Plane;
use crate::three_d::Triangle;
use crate::three_d::Triangles;
use crate::vec3::Float;
use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::Ray;
use crate::RenderStats;

const MAX_NUM_TRIANGLES: usize = 30;
const MAX_DEPTH: u32 = 8;

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
    fn point_inside(&self, p: Point) -> bool {
        p.x >= self.p_min.x
            && p.x <= self.p_max.x
            && p.y >= self.p_min.y
            && p.y <= self.p_max.y
            && p.z >= self.p_min.z
            && p.z <= self.p_max.z
    }
    fn triangle_inside(&self, t: &Triangle) -> bool {
        if self.point_inside(t.points[0])
            || self.point_inside(t.points[1])
            || self.point_inside(t.points[2])
        {
            return true;
        }
        let ray0 = Ray::new(t.points[0], t.points[1] - t.points[0]);
        let ray1 = Ray::new(t.points[1], t.points[2] - t.points[1]);
        let ray2 = Ray::new(t.points[2], t.points[0] - t.points[2]);
        /*
         * XXX: not correct if the AABB doesn't touch an edge!!
         */
        let mut t0 = 0.0;
        self.check_intersect(&ray0, 1.0, &mut t0)
            || self.check_intersect(&ray1, 1.0, &mut t0)
            || self.check_intersect(&ray2, 1.0, &mut t0)
    }
    fn setup_node(
        &mut self,
        p_min: Point,
        p_max: Point,
        triangles: &Vec<AABBTriangle>,
        depth: u32,
    ) {
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
        self.setup_node(p_min, p_max, &vec![], 0);
        let elapsed = start_time.elapsed();

        println!(
            "-- aabb: depth: {}/{} num_leaves={} max_num_triangles={} -- {:.2} sec",
            self.get_depth(),
            MAX_DEPTH,
            self.count_leaves(),
            MAX_NUM_TRIANGLES,
            elapsed.as_millis() as Float / 1000.0
        );
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
         * If any interception exists and it's closer to the entry point into
         * this node, we're done.
         */
        if t_aabb < tmin {
            return false;
        }

        let mut oid0 = 0;
        let mut hit = false;

        if self.is_leaf {
            for triangle_id in &self.triangles {
                let t = self.triangles_soa.get_triangle(*triangle_id);
                if t.intercept(stats, ray, tmin, tmax, any, &mut oid0) {
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
                    .intercept(stats, ray, tmin, tmax, any, oid)
                {
                    return true;
                }

                let mut t_yz = Float::MAX;
                let mut t_xz = t_yz;
                let mut t_xy = t_yz;
                let mut p = [false; 3];

                p[0] = plane_yz.intercept(stats, ray, tmin0, &mut t_yz, false, &mut oid0);
                p[1] = plane_xz.intercept(stats, ray, tmin0, &mut t_xz, false, &mut oid0);
                p[2] = plane_xy.intercept(stats, ray, tmin0, &mut t_xy, false, &mut oid0);

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
