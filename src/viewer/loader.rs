use crate::{
    Allocators,
    vktf::{GltfRenderInfo, loader::VktfDocument},
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
    ) -> gltf::Result<GltfRenderInfo> {
        let vktf_document = VktfDocument::new(self.allocators.mem.clone(), builder, path)?;

        let info = GltfRenderInfo::new_default(
            self.allocators.mem.clone(),
            self.allocators.set.clone(),
            self.material_set_layout.clone(),
            vktf_document,
        );
        Ok(info)
    }
}
