use crate::{
    Allocators,
    cubemap::{CubeMesh, CubemapPipelineLayout, CubemapShaders},
    set_layouts::SetLayouts,
};
use loader::SkyboxLoader;
use renderer::SkyboxRenderer;
use std::{path::PathBuf, sync::Arc, thread::JoinHandle};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract},
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    device::{DeviceOwned, Queue},
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

pub struct Skybox {
    pub renderer: SkyboxRenderer,
    pub loader: SkyboxLoader,
    pub job: Option<JoinHandle<(Arc<Image>, Arc<Image>)>>,
}
impl Skybox {
    pub fn new<L>(
        allocators: &Allocators,
        builder: &mut AutoCommandBufferBuilder<L>,
        set_layouts: &SetLayouts,
        subpass: Subpass,
    ) -> Self {
        let device = allocators.mem.device();

        let cube = Arc::new(CubeMesh::new(allocators.mem.clone(), builder));

        let cubemap_pipeline_layout =
            CubemapPipelineLayout::new(set_layouts.camera.clone(), set_layouts.texture.clone());
        let cubemap_shaders = CubemapShaders::new(device.clone());

        let skybox_pipeline = cubemap_pipeline_layout.clone().create_pipeline(
            cubemap_shaders.vs.clone(),
            cubemap_shaders.cube_fs.clone(),
            cubemap_shaders.vertex_input_state.clone(),
            subpass.clone(),
        );

        let loader = SkyboxLoader::new(
            allocators.clone(),
            &cubemap_pipeline_layout,
            &cubemap_shaders,
            set_layouts,
            cube.clone(),
        );
        let renderer = SkyboxRenderer {
            pipeline: skybox_pipeline,
            cube,
            skybox: None,
        };

        Self {
            renderer,
            loader,
            job: None,
        }
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
            let image = loader.load(path, &mut builder).unwrap();
            let cb = builder.build().unwrap();

            cb.execute(queue)
                .unwrap()
                .then_signal_fence_and_flush()
                .unwrap()
                .wait(None)
                .unwrap();

            image
        });
        self.job = Some(job)
    }
    pub fn loading(&self) -> bool {
        self.job.is_some()
    }
    pub fn update(&mut self) -> Option<Arc<Image>> {
        if let Some((cube, conv)) = self
            .job
            .take_if(|job| job.is_finished())
            .map(|job| job.join().unwrap())
        {
            let cube_view = ImageView::new(
                cube.clone(),
                ImageViewCreateInfo {
                    view_type: ImageViewType::Cube,
                    ..ImageViewCreateInfo::from_image(&cube)
                },
            )
            .unwrap();
            let cube_set = DescriptorSet::new(
                self.loader.allocators.set.clone(),
                self.renderer.pipeline.layout().set_layouts()[1].clone(),
                [WriteDescriptorSet::image_view_sampler(
                    0,
                    cube_view.clone(),
                    Sampler::new(
                        self.loader.allocators.mem.device().clone(),
                        SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
                    )
                    .unwrap(),
                )],
                [],
            )
            .unwrap();
            self.renderer.skybox = Some(cube_set);
            Some(conv)
        } else {
            None
        }
    }
}
