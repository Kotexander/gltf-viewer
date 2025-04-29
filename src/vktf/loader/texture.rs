use super::{Loader, image::Image, sampler::Sampler};
use std::sync::Arc;
use vulkano::descriptor_set::WriteDescriptorSet;

#[derive(Clone)]
pub struct Texture {
    pub name: Option<Arc<str>>,
    pub image: Image,
    pub sampler: Sampler,
}
impl Texture {
    pub(super) fn from_loader(texture: gltf::Texture, loader: &mut Loader) -> Texture {
        let sampler = loader.get_sampler(texture.sampler()).clone();
        let image = loader.get_image(texture.source()).clone();

        Self {
            image,
            sampler,
            name: texture.name().map(From::from),
        }
    }
    pub fn bind(self, binding: u32) -> WriteDescriptorSet {
        WriteDescriptorSet::image_view_sampler(binding, self.image.vk, self.sampler.vk)
    }
}
