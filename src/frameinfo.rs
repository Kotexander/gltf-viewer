use std::sync::Arc;
use vulkano::{
    command_buffer::RenderPassBeginInfo,
    device::DeviceOwned,
    format::Format,
    image::{Image, ImageCreateInfo, ImageType, ImageUsage, SampleCount, view::ImageView},
    memory::allocator::{AllocationCreateInfo, StandardMemoryAllocator},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
};

pub struct FrameInfo {
    depth_buffer: Arc<ImageView>,
    msaa_buffer: Arc<ImageView>,
    frame_buffers: Vec<Arc<Framebuffer>>,
    render_pass: Arc<RenderPass>,
    subpass: Subpass,
    mem_alloc: Arc<StandardMemoryAllocator>,
}
impl FrameInfo {
    const DEPTH_FORMAT: Format = Format::D32_SFLOAT;

    pub fn new(mem_alloc: Arc<StandardMemoryAllocator>, views: Vec<Arc<ImageView>>) -> Self {
        let format = views[0].image().format();

        let render_pass = vulkano::single_pass_renderpass!(
            mem_alloc.device().clone(),
            attachments: {
                intermediary: {
                  format: format,
                  samples: 4,
                  load_op: Clear,
                  store_op: DontCare,
                },
                color: {
                    format: format,
                    samples: 1,
                    load_op: DontCare,
                    store_op: Store,
                },
                depth_stencil: {
                    format: Self::DEPTH_FORMAT,
                    samples: 4,
                    load_op: Clear,
                    store_op: DontCare,
                },
            },
            pass: {
                color: [intermediary],
                color_resolve: [color],
                depth_stencil: {depth_stencil}
            },
        )
        .unwrap();
        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

        let extent = views[0].image().extent();
        let depth_buffer = Self::create_depth_buffer(mem_alloc.clone(), extent);
        let msaa_buffer = Self::create_mssa_buffer(mem_alloc.clone(), format, extent);
        let frame_buffers =
            Self::create_frame_buffers(&render_pass, &msaa_buffer, &depth_buffer, views);

        Self {
            depth_buffer,
            msaa_buffer,
            frame_buffers,
            render_pass,
            subpass,
            mem_alloc,
        }
    }
    pub fn recreate(&mut self, views: Vec<Arc<ImageView>>) {
        let extent = views[0].image().extent();
        let format = views[0].image().format();
        self.depth_buffer = Self::create_depth_buffer(self.mem_alloc.clone(), extent);
        self.msaa_buffer = Self::create_mssa_buffer(self.mem_alloc.clone(), format, extent);
        self.frame_buffers = Self::create_frame_buffers(
            &self.render_pass,
            &self.msaa_buffer,
            &self.depth_buffer,
            views,
        );
    }
    pub fn render_pass_info(&self, index: usize) -> RenderPassBeginInfo {
        RenderPassBeginInfo {
            clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into()), None, Some(1f32.into())],
            ..RenderPassBeginInfo::framebuffer(self.frame_buffers[index].clone())
        }
    }
    pub fn subpass(&self) -> &Subpass {
        &self.subpass
    }

    fn create_depth_buffer(
        allocator: Arc<StandardMemoryAllocator>,
        extent: [u32; 3],
    ) -> Arc<ImageView> {
        ImageView::new_default(
            Image::new(
                allocator,
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    format: Self::DEPTH_FORMAT,
                    extent,
                    samples: SampleCount::Sample4,
                    usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn create_mssa_buffer(
        allocator: Arc<StandardMemoryAllocator>,
        format: Format,
        extent: [u32; 3],
    ) -> Arc<ImageView> {
        ImageView::new_default(
            Image::new(
                allocator,
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    format,
                    extent,
                    samples: SampleCount::Sample4,
                    usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn create_frame_buffers(
        render_pass: &Arc<RenderPass>,
        msaa_buffer: &Arc<ImageView>,
        depth_buffer: &Arc<ImageView>,
        views: Vec<Arc<ImageView>>,
    ) -> Vec<Arc<Framebuffer>> {
        views
            .into_iter()
            .map(|view| {
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![msaa_buffer.clone(), view, depth_buffer.clone()],
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect()
    }
}
