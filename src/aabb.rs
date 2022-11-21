use std::time::Instant;

use crate::three_d::Object;
use crate::three_d::Triangle;
use crate::vec3::Point;
use crate::vec3::Vec3;
use crate::Ray;
use crate::RenderStats;

const MAX_NUM_TRIANGLES: usize = 300;
const MAX_DEPTH: u32 = 5;

/*
 * Axis-Aligned Bounding Box
 */

pub struct AABB {
    p_min: Point,
    p_max: Point,
    is_leaf: bool,
    aabbs: Option<Vec<AABB>>,
    triangles: Vec<Triangle>,
}

impl AABB {
    pub fn new() -> AABB {
        AABB {
            p_min: Point::new(),
            p_max: Point::new(),
            is_leaf: false,
            triangles: vec![],
            aabbs: None,
        }
    }
    fn init_with_point(p_min: &mut Point, p_max: &mut Point, point: Point) {
        p_min.x = p_min.x.min(point.x);
        p_min.y = p_min.y.min(point.y);
        p_min.z = p_min.z.min(point.z);

        p_max.x = p_max.x.max(point.x);
        p_max.y = p_max.y.max(point.y);
        p_max.z = p_max.z.max(point.z);
    }
    fn init_with_triangle(p_min: &mut Point, p_max: &mut Point, triangle: &Triangle) {
        for p in triangle.points {
            Self::init_with_point(p_min, p_max, p);
        }
    }
    fn find_bounds(&self, p_min: &mut Point, p_max: &mut Point, triangles: &Vec<Triangle>) {
        for triangle in triangles {
            Self::init_with_triangle(p_min, p_max, &triangle);
        }
    }
//   fn point_inside(&self, p: &Point) -> bool {
//       p.x >= self.p_min.x && p.x <= self.p_max.x &&
//       p.y >= self.p_min.y && p.y <= self.p_max.y &&
//       p.z >= self.p_min.z && p.z <= self.p_max.z
//   }
    fn triangle_inside(&self, t: &Triangle) -> bool {
//      if self.point_inside(&t.points[0]) ||
//         self.point_inside(&t.points[1]) ||
//         self.point_inside(&t.points[2]) {
//             return true
//      }
        let ray0 = Ray {
            orig: t.points[0],
            dir: t.points[1] - t.points[0],
        };
        let ray1 = Ray {
            orig: t.points[1],
            dir: t.points[2] - t.points[1],
        };
        let ray2 = Ray {
            orig: t.points[2],
            dir: t.points[0] - t.points[2],
        };
        /*
         * XXX: not correct if the AABB doesn't touch an edge!!
         */
        let mut t = 0.0;
        return self.check_intersect(&ray0, 1.0, &mut t)
            || self.check_intersect(&ray1, 1.0, &mut t)
            || self.check_intersect(&ray2, 1.0, &mut t);
    }
    fn setup_node(&mut self, p_min: Point, p_max: Point, triangles: &Vec<Triangle>, depth: u32) {
        self.p_min = p_min;
        self.p_max = p_max;

        let mut v_triangles = vec![];
        for triangle in triangles {
            if self.triangle_inside(triangle) {
                v_triangles.push(*triangle);
            }
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
        self.is_leaf = false;

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
        self.aabbs = Some(Vec::with_capacity(8));
        for i in 0..8 {
            let mut aabb = AABB::new();
            aabb.setup_node(v_min[i], v_max[i], triangles, depth + 1);
            self.aabbs.as_mut().unwrap().push(aabb);
        }
    }
    pub fn get_depth(&self) -> u32 {
        if self.is_leaf {
            return 0;
        }
        let mut depth = 0;
        for i in 0..8 {
            let d = 1 + self.aabbs.as_ref().unwrap()[i].get_depth();
            depth = depth.max(d);
        }
        depth
    }
    pub fn init_aabb(&mut self, triangles: &Vec<Triangle>) {
        let mut p_min = Vec3::new();
        let mut p_max = Vec3::new();
        self.find_bounds(&mut p_min, &mut p_max, triangles);

        let start_time = Instant::now();
        self.setup_node(p_min, p_max, triangles, 0);
        let elapsed = start_time.elapsed();

        if elapsed.as_secs() >= 1 {
            println!(
                "-- aabb generated in {:.2} sec",
                elapsed.as_millis() as f64 / 1000.0
            );
        }
        println!("-- min: {:?} -- max: {:?}", self.p_min, self.p_max);
        println!(
            "-- max-depth: {} size: {:?}",
            self.get_depth(),
            self.p_max - self.p_min
        );
    }

    fn nearest_node(&self, p: Point) -> usize {
        let op = p - (self.p_min + (self.p_max - self.p_min) / 2.0);
        let x_test = op.x.is_sign_positive();
        let y_test = op.y.is_sign_positive();
        let z_test = op.z.is_sign_positive();

        let mut v : usize = 0;
        if x_test {
            v = 1 << 0;
        }
        if y_test {
            v += 1 << 1;
        }
        if z_test {
            v += 1 << 2;
        }
        return v
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
        let mut t = *tmax;
        if !self.check_intersect(ray, *tmax, &mut t) {
            return false;
        }
        let p = ray.orig + ray.dir * t;
        let close_idx = self.nearest_node(p);
        assert!(close_idx < 8);

        let mut hit = false;

        if self.is_leaf {
            let mut oid0 = 0;
            for triangle in &self.triangles {
                if triangle.intercept(stats, ray, tmin, tmax, any, &mut oid0) {
                    hit = true;
                    *oid = triangle.mesh_id;
                    if any {
                        break;
                    }
                }
            }
        } else {
            if self.aabbs.as_ref().unwrap()[close_idx].intercept(stats, ray, tmin, tmax, any, oid) {
                return true
            }
            for i in 0..8 {
                if i == close_idx {
                    continue;
                }
                if self.aabbs.as_ref().unwrap()[i].intercept(stats, ray, tmin, tmax, any, oid) {
                    hit = true;
                    if any {
                        break;
                    }
                }
            }
        }
        hit
    }

    // https://tavianator.com/cgit/dimension.git/tree/libdimension/bvh/bvh.c#n194
    //
    // This is actually correct, even though it appears not to handle edge cases
    // (ray.n.{x,y,z} == 0).  It works because the infinities that result from
    // dividing by zero will still behave correctly in the comparisons.  Rays
    // which are parallel to an axis and outside the box will have tmin == inf
    // or tmax == -inf, while rays inside the box will have tmin and tmax
    // unchanged.
    fn check_intersect(&self, ray: &Ray, tmax: f64, t: &mut f64) -> bool {
        let inv_dir = Vec3 {
            x: 1.0 / ray.dir.x,
            y: 1.0 / ray.dir.y,
            z: 1.0 / ray.dir.z,
        };

        let tx1 = (self.p_min.x - ray.orig.x) * inv_dir.x;
        let tx2 = (self.p_max.x - ray.orig.x) * inv_dir.x;

        let mut t_min = tx1.min(tx2);
        let mut t_max = tx1.max(tx2);

        let ty1 = (self.p_min.y - ray.orig.y) * inv_dir.y;
        let ty2 = (self.p_max.y - ray.orig.y) * inv_dir.y;

        t_min = t_min.max(ty1.min(ty2));
        t_max = t_max.min(ty1.max(ty2));

        let tz1 = (self.p_min.z - ray.orig.z) * inv_dir.z;
        let tz2 = (self.p_max.z - ray.orig.z) * inv_dir.z;

        t_min = t_min.max(tz1.min(tz2));
        t_max = t_max.min(tz1.max(tz2));

        if t_max >= t_min.max(0.0) && t_min < tmax {
            *t = t_min;
            return true
        }
        false
    }
}
