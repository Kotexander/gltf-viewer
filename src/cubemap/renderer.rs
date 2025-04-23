use super::CubeMesh;
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, RenderPassBeginInfo, SubpassBeginInfo, SubpassEndInfo,
    },
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
        layout::DescriptorSetLayout,
    },
    device::DeviceOwned,
    format::Format,
    image::{
        Image, ImageCreateFlags, ImageCreateInfo, ImageSubresourceRange, ImageType, ImageUsage,
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{
        GraphicsPipeline, Pipeline, PipelineBindPoint,
        graphics::viewport::{Scissor, Viewport},
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, Subpass},
};

fn create_cubemap_cameras(
    mem_allocator: Arc<StandardMemoryAllocator>,
    set_allocator: Arc<StandardDescriptorSetAllocator>,
    camera_set_layout: Arc<DescriptorSetLayout>,
) -> Vec<Arc<DescriptorSet>> {
    let proj = glm::perspective_rh_zo(1.0, std::f32::consts::FRAC_PI_2, 0.1, 10.0);
    let eye = glm::Vec3::zeros();
    #[rustfmt::skip]
    let views = [
        [glm::look_at_rh(&eye, &glm::vec3( 1.0,  0.0,  0.0), &glm::vec3( 0.0, -1.0,  0.0)), proj],
        [glm::look_at_rh(&eye, &glm::vec3(-1.0,  0.0,  0.0), &glm::vec3( 0.0, -1.0,  0.0)), proj],
        [glm::look_at_rh(&eye, &glm::vec3( 0.0,  1.0,  0.0), &glm::vec3( 0.0,  0.0,  1.0)), proj],
        [glm::look_at_rh(&eye, &glm::vec3( 0.0, -1.0,  0.0), &glm::vec3( 0.0,  0.0, -1.0)), proj],
        [glm::look_at_rh(&eye, &glm::vec3( 0.0,  0.0,  1.0), &glm::vec3( 0.0, -1.0,  0.0)), proj],
        [glm::look_at_rh(&eye, &glm::vec3( 0.0,  0.0, -1.0), &glm::vec3( 0.0, -1.0,  0.0)), proj],
    ];

    views
        .into_iter()
        .map(|view| {
            let buffer = Buffer::from_data(
                mem_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::UNIFORM_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                view,
            )
            .unwrap();
            DescriptorSet::new(
                set_allocator.clone(),
                camera_set_layout.clone(),
                [WriteDescriptorSet::buffer(0, buffer)],
                [],
            )
            .unwrap()
        })
        .collect()
}

pub struct CubemapRenderPass {
    pub subpass: Subpass,
    pub cameras: Vec<Arc<DescriptorSet>>,
}
impl CubemapRenderPass {
    pub fn new(
        mem_allocator: Arc<StandardMemoryAllocator>,
        set_allocator: Arc<StandardDescriptorSetAllocator>,
        camera_set_layout: Arc<DescriptorSetLayout>,
    ) -> Self {
        let device = mem_allocator.device();
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    format: Format::R16G16B16A16_SFLOAT,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {},
            }
        )
        .unwrap();
        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

        let cameras = create_cubemap_cameras(mem_allocator, set_allocator, camera_set_layout);

        Self { subpass, cameras }
    }
}

#[derive(Clone)]
pub struct CubemapRenderPipeline {
    pub pipeline: Arc<GraphicsPipeline>,
    pub renderer: Arc<CubemapRenderPass>,
    pub cube: Arc<CubeMesh>,
}
impl CubemapRenderPipeline {
    pub fn render<L>(
        &self,
        builder: &mut AutoCommandBufferBuilder<L>,
        equi_set: &Arc<DescriptorSet>,
        image: &Arc<Image>,
        mip: u32,
    ) {
        let extent = image.extent();
        let mip_width = (extent[0] >> mip).max(1);
        let mip_height = (extent[1] >> mip).max(1);
        builder
            .set_viewport(
                0,
                vec![Viewport {
                    extent: [mip_width as f32, mip_height as f32],
                    ..Default::default()
                }]
                .into(),
            )
            .unwrap()
            .set_scissor(0, vec![Scissor::default()].into())
            .unwrap();

        let views = (0..6).map(|i| {
            ImageView::new(
                image.clone(),
                ImageViewCreateInfo {
                    view_type: ImageViewType::Dim2d,
                    format: image.format(),
                    subresource_range: ImageSubresourceRange {
                        aspects: image.format().aspects(),
                        mip_levels: mip..mip + 1,
                        array_layers: i..i + 1,
                    },
                    ..Default::default()
                },
            )
            .unwrap()
        });

        for (view, cam_set) in views.zip(self.renderer.cameras.iter()) {
            let framebuffer = Framebuffer::new(
                self.renderer.subpass.render_pass().clone(),
                FramebufferCreateInfo {
                    attachments: vec![view.clone()],
                    extent: [mip_width, mip_height],
                    ..Default::default()
                },
            )
            .unwrap();

            builder
                .begin_render_pass(
                    RenderPassBeginInfo {
                        clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into())],
                        ..RenderPassBeginInfo::framebuffer(framebuffer)
                    },
                    SubpassBeginInfo::default(),
                )
                .unwrap();

            builder
                .bind_pipeline_graphics(self.pipeline.clone())
                .unwrap();
            builder
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline.layout().clone(),
                    0,
                    vec![cam_set.clone(), equi_set.clone()],
                )
                .unwrap();
            self.cube.clone().render(builder);
            builder.end_render_pass(SubpassEndInfo::default()).unwrap();
        }
    }
}

pub fn create_cubemap_image(
    allocator: Arc<StandardMemoryAllocator>,
    size: u32,
    mips: u32,
) -> Arc<Image> {
    Image::new(
        allocator,
        ImageCreateInfo {
            flags: ImageCreateFlags::CUBE_COMPATIBLE,
            usage: ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::SAMPLED
                | if mips > 0 {
                    ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST
                } else {
                    ImageUsage::empty()
                },
            image_type: ImageType::Dim2d,
            array_layers: 6,
            extent: [size, size, 1],
            mip_levels: mips,
            format: Format::R16G16B16A16_SFLOAT,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap()
}
