pub mod image;
pub mod color;
pub mod vec3;
pub mod light;
pub mod camera;

#[derive(Debug)]
pub struct Ray {
    pub orig: vec3::Point,
    pub dir: vec3::Vec3
}
