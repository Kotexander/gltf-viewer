use super::{Loader, texture::Texture};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
        layout::DescriptorSetLayout,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
};

#[repr(C)]
#[derive(Clone, Copy, BufferContents)]
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
    pub set: Arc<DescriptorSet>,
    pub uniform: MaterialUniform,
}
impl Material {
    pub fn from_loader(
        material: gltf::Material,
        images: &mut [Option<::image::RgbaImage>],
        loader: &mut Loader,
    ) -> Self {
        let pbr = material.pbr_metallic_roughness();

        let mut uniform = MaterialUniform {
            bc: pbr.base_color_factor().into(),
            rm: glm::vec2(pbr.roughness_factor(), pbr.metallic_factor()),
            em: material.emissive_factor().into(),
            ..Default::default()
        };

        let base_colour = if let Some(base_color) = pbr.base_color_texture() {
            uniform.bc_set = base_color.tex_coord() as i32;
            loader.get_texture(base_color.texture(), true, images)
        } else {
            loader.get_default_texture()
        };

        let roughness_matallic = if let Some(rougness_metallic) = pbr.metallic_roughness_texture() {
            uniform.rm_set = rougness_metallic.tex_coord() as i32;
            loader.get_texture(rougness_metallic.texture(), false, images)
        } else {
            loader.get_default_texture()
        };

        let occlusion = if let Some(occlusion) = material.occlusion_texture() {
            uniform.ao_set = occlusion.tex_coord() as i32;
            uniform.ao = occlusion.strength();
            loader.get_texture(occlusion.texture(), false, images)
        } else {
            loader.get_default_texture()
        };

        let emissive = if let Some(emissive) = material.emissive_texture() {
            uniform.em_set = emissive.tex_coord() as i32;
            loader.get_texture(emissive.texture(), true, images)
        } else {
            loader.get_default_texture()
        };

        let normal = if let Some(normal) = material.normal_texture() {
            uniform.nm_set = normal.tex_coord() as i32;
            uniform.nm = normal.scale();
            loader.get_texture(normal.texture(), false, images)
        } else {
            loader.get_default_texture()
        };

        let factors_buffer = Self::create_factor_buffer(loader.allocators.mem.clone(), uniform);

        let set = Self::create_set(
            loader.allocators.set.clone(),
            loader.material_set_layout.clone(),
            factors_buffer,
            base_colour,
            roughness_matallic,
            occlusion,
            emissive,
            normal,
        );

        Self { set, uniform }
    }

    pub fn create_factor_buffer(
        allocator: Arc<StandardMemoryAllocator>,
        uniform: MaterialUniform,
    ) -> Subbuffer<MaterialUniform> {
        Buffer::from_data(
            allocator,
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            uniform,
        )
        .unwrap()
    }
    pub fn create_set(
        allocator: Arc<StandardDescriptorSetAllocator>,
        layout: Arc<DescriptorSetLayout>,
        factors: Subbuffer<MaterialUniform>,
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
                WriteDescriptorSet::buffer(0, factors),
                base_colour.bind(1),
                roughness_matallic.bind(2),
                occlusion.bind(3),
                emissive.bind(4),
                normal.bind(5),
            ],
            [],
        )
        .unwrap()
    }
}
