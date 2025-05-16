use super::loader::{Vktf, VktfDocument};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::BufferContents,
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::DescriptorSetAllocator,
        layout::DescriptorSetLayout,
    },
    pipeline::{PipelineBindPoint, PipelineLayout},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, BufferContents)]
pub struct MaterialPush {
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
impl MaterialPush {
    pub fn new(material: &gltf::Material) -> Self {
        let pbr = material.pbr_metallic_roughness();
        let mut slf = Self {
            bc: pbr.base_color_factor().into(),
            em: material.emissive_factor().into(),
            rm: glm::vec2(pbr.roughness_factor(), pbr.metallic_factor()),
            ..Default::default()
        };
        if let Some(ao) = material.occlusion_texture() {
            slf.ao = ao.strength();
        }
        if let Some(nm) = material.normal_texture() {
            slf.nm = nm.scale();
        }
        if let Some(bc_set) = pbr.base_color_texture() {
            slf.bc_set = bc_set.tex_coord() as i32;
        }
        if let Some(rm_set) = pbr.metallic_roughness_texture() {
            slf.rm_set = rm_set.tex_coord() as i32;
        }
        if let Some(ao_set) = material.occlusion_texture() {
            slf.ao_set = ao_set.tex_coord() as i32;
        }
        if let Some(em_set) = material.emissive_texture() {
            slf.em_set = em_set.tex_coord() as i32;
        }
        if let Some(nm_set) = material.normal_texture() {
            slf.nm_set = nm_set.tex_coord() as i32;
        }

        slf
    }
}
impl Default for MaterialPush {
    fn default() -> Self {
        Self {
            bc: glm::vec4(1.0, 1.0, 1.0, 1.0),
            em: glm::vec3(0.0, 0.0, 0.0),
            ao: 1.0,
            rm: glm::vec2(1.0, 1.0),
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
    pub push: MaterialPush,
    pub set: Arc<DescriptorSet>,
}
impl Material {
    pub fn new(
        material: &gltf::Material,
        allocator: Arc<dyn DescriptorSetAllocator>,
        layout: Arc<DescriptorSetLayout>,
        vktf: &Vktf,
    ) -> Self {
        let pbr = material.pbr_metallic_roughness();
        let bc = pbr.base_color_texture().map(|bc| bc.texture());
        let rm = pbr.metallic_roughness_texture().map(|bc| bc.texture());
        let ao = material.occlusion_texture().map(|ao| ao.texture());
        let em = material.emissive_texture().map(|em| em.texture());
        let nm = material.normal_texture().map(|nm| nm.texture());
        let set = DescriptorSet::new(
            allocator,
            layout,
            [
                write_descriptor_set(0, bc.as_ref(), vktf),
                write_descriptor_set(1, rm.as_ref(), vktf),
                write_descriptor_set(2, ao.as_ref(), vktf),
                write_descriptor_set(3, em.as_ref(), vktf),
                write_descriptor_set(4, nm.as_ref(), vktf),
            ],
            [],
        )
        .unwrap();
        Self {
            push: MaterialPush::new(material),
            set,
        }
    }

    pub fn set<L>(self, builder: &mut AutoCommandBufferBuilder<L>, layout: Arc<PipelineLayout>) {
        builder
            .bind_descriptor_sets(PipelineBindPoint::Graphics, layout.clone(), 2, self.set)
            .unwrap()
            .push_constants(layout, 0, self.push)
            .unwrap();
    }
}

fn write_descriptor_set(
    binding: u32,
    texture: Option<&gltf::Texture>,
    vktf: &Vktf,
) -> WriteDescriptorSet {
    WriteDescriptorSet::image_view_sampler(
        binding,
        vktf.get_image(texture.map(|t| t.source().index()))
            .unwrap()
            .clone(),
        vktf.get_sampler(texture.and_then(|t| t.sampler().index()))
            .unwrap()
            .clone(),
    )
}

#[derive(Clone)]
pub struct Materials {
    pub index: Vec<Material>,
    // TODO: make default actually an option
    pub default: Material,
}
impl Materials {
    pub fn new(
        allocator: Arc<dyn DescriptorSetAllocator>,
        layout: Arc<DescriptorSetLayout>,
        vktf: &VktfDocument,
    ) -> Self {
        let index = vktf
            .document
            .materials()
            .map(|mat| Material::new(&mat, allocator.clone(), layout.clone(), &vktf.vktf))
            .collect();
        let default = Material {
            push: MaterialPush::default(),
            set: DescriptorSet::new(
                allocator,
                layout,
                [
                    write_descriptor_set(0, None, &vktf.vktf),
                    write_descriptor_set(1, None, &vktf.vktf),
                    write_descriptor_set(2, None, &vktf.vktf),
                    write_descriptor_set(3, None, &vktf.vktf),
                    write_descriptor_set(4, None, &vktf.vktf),
                ],
                [],
            )
            .unwrap(),
        };

        Self { default, index }
    }
    pub fn get(&self, index: Option<usize>) -> Option<&Material> {
        match index {
            Some(i) => self.index.get(i),
            None => Some(&self.default),
        }
    }
}
