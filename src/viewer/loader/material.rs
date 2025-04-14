use super::Loader;
use std::sync::Arc;
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};

pub struct Material {
    pub set: Arc<DescriptorSet>,
    pub bc_tex: Option<u32>,
}
impl Material {
    pub fn from_loader(material: gltf::Material, loader: &mut Loader) -> Self {
        let pbr = material.pbr_metallic_roughness();
        let bc = pbr
            .base_color_texture()
            .map(|base_color| base_color.tex_coord());
        // // bc.texture().i
        let base_texture = &loader.textures[pbr.base_color_texture().unwrap().texture().index()];
        let set = DescriptorSet::new(
            loader.allocators.set.clone(),
            loader.material_set_layout.clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                base_texture.view.clone(),
                base_texture.sampler.clone(),
            )],
            [],
        )
        .unwrap();

        Self { bc_tex: bc, set }
    }
}
