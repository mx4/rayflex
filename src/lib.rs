pub mod aabb;
pub mod app;
pub mod camera;
pub mod color;
pub mod image;
pub mod light;
pub mod material;
pub mod scene;
pub mod three_d;
pub mod vec3;

pub mod render;
pub use app::egui_main;

use vec3::Point;
use vec3::Vec3;

pub struct ProgressFunc {
    pub func: Box<dyn Fn(f32) + Send + Sync>,
}

#[derive(Debug)]
pub struct Ray {
    pub orig: Point,
    pub dir: Vec3,
    pub inv_dir: Vec3, // aabb optimization
}

impl Ray {
    pub fn new(point: Point, dir: Vec3) -> Ray {
        let inv_dir = Vec3 {
            x: 1.0 / dir.x,
            y: 1.0 / dir.y,
            z: 1.0 / dir.z,
        };
        Ray {
            orig: point,
            dir,
            inv_dir,
        }
    }
    pub fn get_reflection(&self, point: Point, normal: Vec3) -> Ray {
        Ray::new(point, self.dir.reflect(normal))
    }
}

#[derive(Clone, Copy, Default)]
pub struct RenderStats {
    pub num_rays_sampling: u64,
    pub num_rays_sampling_max: u64,
    pub num_rays_reflection: u64,
    pub num_rays_reflection_max: u64,
    pub num_intersects_plane: u64,
    pub num_intersects_sphere: u64,
    pub num_intersects_triangle: u64,
    pub num_intersects_aabb: u64,
}

impl RenderStats {
    pub fn add(&mut self, other: RenderStats) {
        self.num_rays_sampling += other.num_rays_sampling;
        self.num_rays_sampling_max += other.num_rays_sampling_max;
        self.num_rays_reflection += other.num_rays_reflection;
        self.num_rays_reflection_max += other.num_rays_reflection_max;
        self.num_intersects_sphere += other.num_intersects_sphere;
        self.num_intersects_plane += other.num_intersects_plane;
        self.num_intersects_triangle += other.num_intersects_triangle;
        self.num_intersects_aabb += other.num_intersects_aabb;
    }
}
