pub mod image;
pub mod color;
pub mod vec3;
pub mod light;
pub mod camera;
pub mod three_d;
pub mod aabb;

use vec3::Vec3;
use vec3::Point;

#[derive(Debug)]
pub struct Ray {
    pub orig: Point,
    pub dir: Vec3
}

impl Ray {
    pub fn get_reflection(&self, point: Point, normal: Vec3) -> Ray {
        Ray{orig: point, dir: self.dir.reflect(normal) }
    }
}

#[derive(Clone, Copy)]
pub struct RenderStats {
    pub num_rays_sampling: u64,
    pub num_rays_reflection: u64,
    pub num_rays_hit_max_level: u64,
    pub num_intersects_plane: u64,
    pub num_intersects_sphere: u64,
    pub num_intersects_triangle: u64,
}

impl RenderStats {
    pub fn new() -> RenderStats {
        RenderStats {
            num_rays_sampling: 0,
            num_rays_reflection: 0,
            num_rays_hit_max_level: 0,
            num_intersects_plane: 0,
            num_intersects_sphere: 0,
            num_intersects_triangle: 0,
        }
    }
    pub fn add(&mut self, other: RenderStats) {
        self.num_rays_sampling       += other.num_rays_sampling;
        self.num_rays_reflection     += other.num_rays_reflection;
        self.num_rays_hit_max_level  += other.num_rays_hit_max_level;
        self.num_intersects_sphere   += other.num_intersects_sphere;
        self.num_intersects_plane    += other.num_intersects_plane;
        self.num_intersects_triangle += other.num_intersects_triangle;
    }
}

