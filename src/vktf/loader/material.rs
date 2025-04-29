use super::{Loader, texture::Texture};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::BufferContents,
    descriptor_set::{
        DescriptorSet, allocator::StandardDescriptorSetAllocator, layout::DescriptorSetLayout,
    },
};

#[repr(C)]
#[derive(Debug, Clone, Copy, BufferContents)]
pub struct MaterialUniform {
    pub bc: glm::Vec4,
    pub em: glm::Vec3,
    pub ao: f32,
    pub rm: glm::Vec2,
    pub nm: f32,

    pub bc_set: i32,
    pub rm_set: i32,
    pub ao_set: i32,
    pub em_set: i32,
    pub nm_set: i32,
}
impl Default for MaterialUniform {
    fn default() -> Self {
        Self {
            bc: glm::vec4(1.0, 1.0, 1.0, 1.0),
            rm: glm::vec2(1.0, 1.0),
            ao: 1.0,
            em: glm::vec3(0.0, 0.0, 0.0),
            nm: 1.0,
            bc_set: -1,
            rm_set: -1,
            ao_set: -1,
            em_set: -1,
            nm_set: -1,
        }
    }
}

#[derive(Clone)]
pub struct Material {
    pub name: Option<Arc<str>>,
    pub set: Arc<DescriptorSet>,
    pub uniform: MaterialUniform,
}
impl Material {
    pub(super) fn from_loader(material: gltf::Material, loader: &mut Loader) -> Self {
        let pbr = material.pbr_metallic_roughness();

        let mut uniform = MaterialUniform {
            bc: pbr.base_color_factor().into(),
            rm: glm::vec2(pbr.roughness_factor(), pbr.metallic_factor()),
            em: material.emissive_factor().into(),
            ..Default::default()
        };

        let base_colour = if let Some(base_color) = pbr.base_color_texture() {
            uniform.bc_set = base_color.tex_coord() as i32;
            loader.get_texture(base_color.texture())
        } else {
            loader.get_default_texture()
        };

        let roughness_matallic = if let Some(rougness_metallic) = pbr.metallic_roughness_texture() {
            uniform.rm_set = rougness_metallic.tex_coord() as i32;
            loader.get_texture(rougness_metallic.texture())
        } else {
            loader.get_default_texture()
        };

        let occlusion = if let Some(occlusion) = material.occlusion_texture() {
            uniform.ao_set = occlusion.tex_coord() as i32;
            uniform.ao = occlusion.strength();
            loader.get_texture(occlusion.texture())
        } else {
            loader.get_default_texture()
        };

        let emissive = if let Some(emissive) = material.emissive_texture() {
            uniform.em_set = emissive.tex_coord() as i32;
            loader.get_texture(emissive.texture())
        } else {
            loader.get_default_texture()
        };

        let normal = if let Some(normal) = material.normal_texture() {
            uniform.nm_set = normal.tex_coord() as i32;
            uniform.nm = normal.scale();
            loader.get_texture(normal.texture())
        } else {
            loader.get_default_texture()
        };

        let set = Self::create_set(
            loader.allocators.set.clone(),
            loader.material_set_layout.clone(),
            base_colour,
            roughness_matallic,
            occlusion,
            emissive,
            normal,
        );

        Self {
            set,
            uniform,
            name: material.name().map(From::from),
        }
    }

    pub fn create_set(
        allocator: Arc<StandardDescriptorSetAllocator>,
        layout: Arc<DescriptorSetLayout>,
        base_colour: Texture,
        roughness_matallic: Texture,
        occlusion: Texture,
        emissive: Texture,
        normal: Texture,
    ) -> Arc<DescriptorSet> {
        DescriptorSet::new(
            allocator,
            layout,
            [
                base_colour.bind(0),
                roughness_matallic.bind(1),
                occlusion.bind(2),
                emissive.bind(3),
                normal.bind(4),
            ],
            [],
        )
        .unwrap()
    }
}
