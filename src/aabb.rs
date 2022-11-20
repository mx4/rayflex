use crate::Ray;
use crate::three_d::Triangle;
use crate::vec3::Point;
use crate::vec3::Vec3;

/*
 * Axis-Aligned Bounding Box
 */

pub struct AABB {
    p_min: Point,
    p_max: Point,
    init: bool, 
}

impl AABB {
    pub fn new() -> AABB {
        AABB { p_min: Point::new(), p_max: Point::new(), init: false }
    }
    fn feed_point(&mut self, point: Point) {
        if ! self.init {
            self.p_min = point;
            self.p_max = point;
            self.init = true;
            return;
        }
        self.p_min.x = self.p_min.x.min(point.x);
        self.p_min.y = self.p_min.y.min(point.y);
        self.p_min.z = self.p_min.z.min(point.z);

        self.p_max.x = self.p_max.x.max(point.x);
        self.p_max.y = self.p_max.y.max(point.y);
        self.p_max.z = self.p_max.z.max(point.z);
    }
    pub fn init_with_triangle(&mut self, triangle: &Triangle) {
        self.feed_point(triangle.points[0]);
        self.feed_point(triangle.points[1]);
        self.feed_point(triangle.points[2]);
    }
    pub fn display(&self) {
        println!("min: {:?} -- max: {:?}", self.p_min, self.p_max);
        println!("size: {:?}", self.p_max - self.p_min);
    }

    // https://tavianator.com/cgit/dimension.git/tree/libdimension/bvh/bvh.c#n194
    //
    // This is actually correct, even though it appears not to handle edge cases
    // (ray.n.{x,y,z} == 0).  It works because the infinities that result from
    // dividing by zero will still behave correctly in the comparisons.  Rays
    // which are parallel to an axis and outside the box will have tmin == inf
    // or tmax == -inf, while rays inside the box will have tmin and tmax
    // unchanged.
    pub fn check_intersect(&self, ray: &Ray, t: f64) -> bool {
        let inv_dir = Vec3 {
            x: 1.0 / ray.dir.x,
            y: 1.0 / ray.dir.y,
            z: 1.0 / ray.dir.z,
        };

        let tx1 = (self.p_min.x - ray.orig.x) * inv_dir.x;
        let tx2 = (self.p_max.x - ray.orig.x) * inv_dir.x;

        let mut tmin = tx1.min(tx2);
        let mut tmax = tx1.max(tx2);

        let ty1 = (self.p_min.y - ray.orig.y) * inv_dir.y;
        let ty2 = (self.p_max.y - ray.orig.y) * inv_dir.y;

        tmin = tmin.max(ty1.min(ty2));
        tmax = tmax.min(ty1.max(ty2));

        let tz1 = (self.p_min.z - ray.orig.z) * inv_dir.z;
        let tz2 = (self.p_max.z - ray.orig.z) * inv_dir.z;

        tmin = tmin.max(tz1.min(tz2));
        tmax = tmax.min(tz1.max(tz2));

        return tmax >= tmin.max(0.0) && tmin < t;
    }
}
