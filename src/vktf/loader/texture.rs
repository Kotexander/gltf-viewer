use super::Loader;
use std::sync::Arc;
use vulkano::{
    descriptor_set::WriteDescriptorSet,
    image::{sampler::Sampler, view::ImageView},
};

#[derive(Clone)]
pub struct Texture {
    pub view: Arc<ImageView>,
    pub sampler: Arc<Sampler>,
}
impl Texture {
    pub fn from_loader(texture: gltf::Texture, loader: &mut Loader) -> Texture {
        let sampler = loader.get_sampler(texture.sampler()).clone();
        let view = loader.get_image(texture.source()).clone();

        Self { view, sampler }
    }
    pub fn bind(self, binding: u32) -> WriteDescriptorSet {
        WriteDescriptorSet::image_view_sampler(binding, self.view, self.sampler)
    }
}
