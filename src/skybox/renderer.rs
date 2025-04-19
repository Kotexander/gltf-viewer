use crate::cubemap::CubeMesh;
use std::sync::Arc;
use vulkano::{
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::DescriptorSet,
    pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint},
};

#[derive(Clone)]
pub struct SkyboxRenderer {
    pub pipeline: Arc<GraphicsPipeline>,
    pub skybox: Option<Arc<DescriptorSet>>,
    pub cube: Arc<CubeMesh>,
}
impl SkyboxRenderer {
    pub fn render<L>(&self, builder: &mut AutoCommandBufferBuilder<L>) {
        if let Some(skybox) = self.skybox.clone() {
            builder
                .bind_pipeline_graphics(self.pipeline.clone())
                .unwrap();
            builder
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline.layout().clone(),
                    1,
                    skybox,
                )
                .unwrap();
            self.cube.render(builder);
        }
    }
}
