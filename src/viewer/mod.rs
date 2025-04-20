use crate::{Allocators, gltf::GltfRenderInfo, set_layouts::SetLayouts};
use loader::ViewerLoader;
use renderer::ViewerRenderer;
use std::{path::PathBuf, sync::Arc, thread::JoinHandle};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract},
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    device::Queue,
    image::{
        Image,
        sampler::{Sampler, SamplerCreateInfo},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    pipeline::Pipeline,
    render_pass::Subpass,
    sync::GpuFuture,
};

pub mod loader;
pub mod renderer;

pub struct Viewer {
    pub renderer: ViewerRenderer,
    pub loader: ViewerLoader,
    pub job: Option<JoinHandle<GltfRenderInfo>>,
}
impl Viewer {
    pub fn new(allocators: &Allocators, set_layouts: &SetLayouts, subpass: Subpass) -> Self {
        let renderer = ViewerRenderer::new(allocators, set_layouts, subpass);
        let loader = ViewerLoader {
            allocators: allocators.clone(),
            material_set_layout: set_layouts.material.clone(),
        };

        Self {
            renderer,
            loader,
            job: None,
        }
    }
    pub fn loading(&self) -> bool {
        self.job.is_some()
    }
    pub fn load(&mut self, path: PathBuf, queue: Arc<Queue>) {
        if self.loading() {
            return;
        }
        let loader = self.loader.clone();
        let job = std::thread::spawn(move || {
            let mut builder = AutoCommandBufferBuilder::primary(
                loader.allocators.cmd.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();
            let info = loader.load(path, &mut builder).unwrap();
            let cb = builder.build().unwrap();

            cb.execute(queue)
                .unwrap()
                .then_signal_fence_and_flush()
                .unwrap()
                .wait(None)
                .unwrap();

            info
        });

        self.job = Some(job);
    }
    pub fn update(&mut self) {
        if let Some(info) = self
            .job
            .take_if(|job| job.is_finished())
            .map(|job| job.join().unwrap())
        {
            self.renderer.info = Some(info);
        }
    }
    pub fn new_env(&mut self, diffuse: Arc<Image>, specular: Arc<Image>) {
        let diffuse_view = ImageView::new(
            diffuse.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                ..ImageViewCreateInfo::from_image(&diffuse)
            },
        )
        .unwrap();
        let specular_view = ImageView::new(
            specular.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                ..ImageViewCreateInfo::from_image(&specular)
            },
        )
        .unwrap();
        let env_set = DescriptorSet::new(
            self.loader.allocators.set.clone(),
            self.renderer.pipeline.pipeline.layout().set_layouts()[1].clone(),
            [
                WriteDescriptorSet::image_view_sampler(
                    0,
                    diffuse_view,
                    Sampler::new(
                        self.renderer.pipeline.pipeline.device().clone(),
                        SamplerCreateInfo::simple_repeat_linear(),
                    )
                    .unwrap(),
                ),
                WriteDescriptorSet::image_view_sampler(
                    1,
                    specular_view,
                    Sampler::new(
                        self.renderer.pipeline.pipeline.device().clone(),
                        SamplerCreateInfo::simple_repeat_linear(),
                    )
                    .unwrap(),
                ),
            ],
            [],
        )
        .unwrap();
        self.renderer.env_set = env_set;
    }
}
