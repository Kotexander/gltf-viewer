use crate::{
    Allocators,
    vktf::{GltfRenderInfo, loader::GltfLoader},
};
use std::{path::Path, sync::Arc};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::layout::DescriptorSetLayout,
};

#[derive(Clone)]
pub struct ViewerLoader {
    pub allocators: Allocators,
    pub material_set_layout: Arc<DescriptorSetLayout>,
}
impl ViewerLoader {
    pub fn load(
        &self,
        path: impl AsRef<Path>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> ::gltf::Result<GltfRenderInfo> {
        let gltf_loader = GltfLoader::new(
            self.allocators.clone(),
            self.material_set_layout.clone(),
            builder,
            path,
        )?;
        let scene = gltf_loader.document.default_scene().unwrap();

        let info =
            GltfRenderInfo::from_scene(self.allocators.mem.clone(), scene, &gltf_loader.meshes);
        Ok(info)
    }
}
