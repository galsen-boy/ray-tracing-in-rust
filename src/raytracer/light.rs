use super::color::Color;
use super::geometry::HitInfo;
use super::ray::Ray;
use super::scene::Scene;


pub trait Light: Sync + Send {
    fn compute_light(&self, scene: &Scene, hit_info: &HitInfo, pixel_color: &mut Color, ray: &Ray);
}
