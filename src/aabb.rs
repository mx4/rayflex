use std::sync::Arc;
use std::time::Instant;

use crate::three_d::Object;
use crate::three_d::Plane;
use crate::three_d::Triangle;
use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::Ray;
use crate::RenderStats;

const MAX_NUM_TRIANGLES: usize = 200;
const MAX_DEPTH: u32 = 5;

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
}

impl AABB {
    pub fn new(triangles: Arc<Vec<Triangle>>) -> AABB {
        AABB {
            p_min: Point::new(),
            p_max: Point::new(),
            is_leaf: false,
            triangles: vec![],
            aabbs: None,
            triangles_root: triangles,
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
        for triangle in self.triangles_root.iter() {
            if !init {
                *p_min = triangle.points[0];
                *p_max = triangle.points[0];
                init = true;
            }
            Self::init_with_triangle(p_min, p_max, &triangle);
        }
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
        return self.check_intersect(&ray0, 1.0, &mut t0)
            || self.check_intersect(&ray1, 1.0, &mut t0)
            || self.check_intersect(&ray2, 1.0, &mut t0);
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

        let mut v_min = [Point::new(); 8];
        let mut v_max = [Point::new(); 8];

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
            let mut aabb = AABB::new(self.triangles_root.clone());
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
    pub fn init_aabb(&mut self) {
        let mut p_min = Vec3::new();
        let mut p_max = Vec3::new();
        self.find_bounds(&mut p_min, &mut p_max);

        let start_time = Instant::now();
        self.setup_node(p_min, p_max, &vec![], 0);
        let elapsed = start_time.elapsed();

        if elapsed.as_secs() >= 1 {
            println!(
                "-- aabb generated in {:.2} sec",
                elapsed.as_millis() as f64 / 1000.0
            );
        }
        println!(
            "-- aabb: depth: {}/{} num_leaves={} max_num_triangles={}",
            self.get_depth(),
            MAX_DEPTH,
            self.count_leaves(),
            MAX_NUM_TRIANGLES
        );
        println!("-- aabb: p_min: {:?}", p_min);
        println!("-- aabb: p_max: {:?}", p_max);
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
        return v;
    }

    pub fn intercept(
        &self,
        stats: &mut RenderStats,
        ray: &Ray,
        tmin: f64,
        tmax: &mut f64,
        any: bool,
        oid: &mut usize,
    ) -> bool {
        let mut t_aabb = *tmax;

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
                if self.triangles_root[*triangle_id]
                    .intercept(stats, ray, tmin, tmax, any, &mut oid0)
                {
                    hit = true;
                    *oid = *triangle_id;
                    if any {
                        break;
                    }
                }
            }
            return hit;
        } else {
            let mid = self.p_min + (self.p_max - self.p_min) / 2.0;
            let plane_yz = Plane::new(
                mid,
                Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                0,
            );
            let plane_xz = Plane::new(
                mid,
                Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                0,
            );
            let plane_xy = Plane::new(
                mid,
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                0,
            );
            let mut close_idx = self.nearest_node(ray.orig + ray.dir * t_aabb, mid);
            let mut tmin0 = tmin;
            let mut visited = vec![];

            for _i in 0..4 {
                assert!(close_idx < 8);

                visited.push(close_idx);

                if self.aabbs.as_ref().unwrap()[close_idx]
                    .intercept(stats, ray, tmin, tmax, any, oid)
                {
                    return true;
                }

                let mut t_yz = f64::MAX;
                let mut t_xz = f64::MAX;
                let mut t_xy = f64::MAX;
                let mut planes = [false; 3];

                planes[0] = plane_yz.intercept(stats, ray, tmin0, &mut t_yz, false, &mut oid0);
                planes[1] = plane_xz.intercept(stats, ray, tmin0, &mut t_xz, false, &mut oid0);
                planes[2] = plane_xy.intercept(stats, ray, tmin0, &mut t_xy, false, &mut oid0);

                planes[0] = planes[0] && t_yz > t_aabb;
                planes[1] = planes[1] && t_xz > t_aabb;
                planes[2] = planes[2] && t_xy > t_aabb;
                if t_yz <= t_aabb {
                    t_yz = f64::MAX;
                }
                if t_xy <= t_aabb {
                    t_xy = f64::MAX;
                }
                if t_xz <= t_aabb {
                    t_xz = f64::MAX;
                }

                planes[0] = planes[0] && t_yz <= t_xz && t_yz <= t_xy;
                planes[1] = planes[1] && t_xz <= t_yz && t_xz <= t_xy;
                planes[2] = planes[2] && t_xy <= t_xz && t_yz <= t_yz;

                // planes[0] = planes[0] && self.point_inside(ray.orig + ray.dir * t_yz);
                // planes[1] = planes[1] && self.point_inside(ray.orig + ray.dir * t_xz);
                // planes[2] = planes[2] && self.point_inside(ray.orig + ray.dir * t_xy);

                if !planes.iter().any(|&x| x) {
                    break;
                }

                tmin0 = t_yz.min(t_xy).min(t_xz);
                close_idx = close_idx ^ (1 << planes.iter().position(|&x| x).unwrap());
                assert!(!visited.contains(&close_idx));
            }
        }
        hit
    }

    // https://tavianator.com/cgit/dimension.git/tree/libdimension/bvh/bvh.c#n194
    fn check_intersect(&self, ray: &Ray, tmax: f64, t: &mut f64) -> bool {
        let tx1 = (self.p_min.x - ray.orig.x) * ray.inv_dir.x;
        let tx2 = (self.p_max.x - ray.orig.x) * ray.inv_dir.x;

        let mut t_min = tx1.min(tx2);
        let mut t_max = tx1.max(tx2);

        let ty1 = (self.p_min.y - ray.orig.y) * ray.inv_dir.y;
        let ty2 = (self.p_max.y - ray.orig.y) * ray.inv_dir.y;

        t_min = t_min.max(ty1.min(ty2));
        t_max = t_max.min(ty1.max(ty2));

        let tz1 = (self.p_min.z - ray.orig.z) * ray.inv_dir.z;
        let tz2 = (self.p_max.z - ray.orig.z) * ray.inv_dir.z;

        t_min = t_min.max(tz1.min(tz2));
        t_max = t_max.min(tz1.max(tz2));

        if t_max >= t_min.max(0.0) && t_min < tmax {
            *t = t_min;
            return true;
        }
        false
    }
}
