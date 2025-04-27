use crate::{Allocators, set_layouts::SetLayouts, vktf::GltfRenderInfo};
use loader::ViewerLoader;
use renderer::ViewerRenderer;
use std::{path::PathBuf, sync::Arc, thread::JoinHandle};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract},
    device::Queue,
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
    pub fn new<L>(
        allocators: &Allocators,
        builder: &mut AutoCommandBufferBuilder<L>,
        set_layouts: &SetLayouts,
        subpass: Subpass,
    ) -> Self {
        let renderer = ViewerRenderer::new(allocators, builder, set_layouts, subpass);
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
    pub fn update(&mut self) -> bool {
        if let Some(info) = self
            .job
            .take_if(|job| job.is_finished())
            .map(|job| job.join().unwrap())
        {
            self.renderer.info = Some(info);
            true
        } else {
            false
        }
    }
}
