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

#[derive(Default, Clone)]
pub struct TextureSets {
    pub bc: Option<u32>,
    pub rm: Option<u32>,
    pub ao: Option<u32>,
    pub em: Option<u32>,
    pub nm: Option<u32>,
}

#[repr(C)]
#[derive(BufferContents, Clone)]
pub struct Factors {
    pub bc: glm::Vec4,
    pub em: glm::Vec3,
    pub ao: f32,
    pub rm: glm::Vec2,
}
impl Default for Factors {
    fn default() -> Self {
        Self {
            bc: glm::vec4(1.0, 1.0, 1.0, 1.0),
            rm: glm::vec2(1.0, 1.0),
            ao: 1.0,
            em: glm::vec3(0.0, 0.0, 0.0),
        }
    }
}

#[derive(Clone)]
pub struct Material {
    pub set: Arc<DescriptorSet>,
    pub tex_sets: TextureSets,
}
impl Material {
    pub fn from_loader(
        material: gltf::Material,
        images: &mut [Option<::image::RgbaImage>],
        loader: &mut Loader,
    ) -> Self {
        let pbr = material.pbr_metallic_roughness();

        let mut tex_sets = TextureSets::default();
        let mut factors = Factors {
            bc: pbr.base_color_factor().into(),
            rm: glm::vec2(pbr.roughness_factor(), pbr.metallic_factor()),
            em: material.emissive_factor().into(),
            ..Default::default()
        };

        let base_colour = if let Some(base_color) = pbr.base_color_texture() {
            tex_sets.bc = Some(base_color.tex_coord());
            loader.get_texture(base_color.texture(), true, images)
        } else {
            loader.get_default_texture()
        };

        let roughness_matallic = if let Some(rougness_metallic) = pbr.metallic_roughness_texture() {
            tex_sets.rm = Some(rougness_metallic.tex_coord());
            loader.get_texture(rougness_metallic.texture(), false, images)
        } else {
            loader.get_default_texture()
        };

        let occlusion = if let Some(occlusion) = material.occlusion_texture() {
            tex_sets.ao = Some(occlusion.tex_coord());
            factors.ao = occlusion.strength();
            loader.get_texture(occlusion.texture(), false, images)
        } else {
            loader.get_default_texture()
        };

        let emissive = if let Some(emissive) = material.emissive_texture() {
            tex_sets.em = Some(emissive.tex_coord());
            loader.get_texture(emissive.texture(), true, images)
        } else {
            loader.get_default_texture()
        };

        let normal = if let Some(normal) = material.normal_texture() {
            tex_sets.nm = Some(normal.tex_coord());
            loader.get_texture(normal.texture(), true, images)
        } else {
            loader.get_default_texture()
        };

        let factors_buffer = Self::create_factor_buffer(loader.allocators.mem.clone(), factors);

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

        Self { set, tex_sets }
    }

    pub fn create_factor_buffer(
        allocator: Arc<StandardMemoryAllocator>,
        factors: Factors,
    ) -> Subbuffer<Factors> {
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
            factors,
        )
        .unwrap()
    }
    pub fn create_set(
        allocator: Arc<StandardDescriptorSetAllocator>,
        layout: Arc<DescriptorSetLayout>,
        factors: Subbuffer<Factors>,
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
