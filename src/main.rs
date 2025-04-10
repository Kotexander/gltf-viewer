mod camera;
mod cubemap;

use camera::OrbitCamera;
use cubemap::CubemapRenderer;
use image::EncodableLayout;
use std::sync::Arc;
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        PrimaryCommandBufferAbstract, RenderPassBeginInfo, SubpassBeginInfo, SubpassContents,
        allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{DescriptorSet, allocator::StandardDescriptorSetAllocator},
    device::{Device, DeviceExtensions, Queue},
    format::Format,
    image::{Image, ImageCreateInfo, ImageType, ImageUsage, view::ImageView},
    instance::{
        InstanceCreateInfo,
        debug::{
            DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessengerCallback,
            DebugUtilsMessengerCreateInfo,
        },
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::graphics::viewport::Viewport,
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    swapchain::Surface,
    sync::GpuFuture,
};
use vulkano_util::{
    context::{VulkanoConfig, VulkanoContext},
    window::VulkanoWindows,
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
};

fn debug_info() -> DebugUtilsMessengerCreateInfo {
    DebugUtilsMessengerCreateInfo {
        message_severity: DebugUtilsMessageSeverity::ERROR
            | DebugUtilsMessageSeverity::WARNING
            | DebugUtilsMessageSeverity::INFO
            | DebugUtilsMessageSeverity::VERBOSE,
        message_type: DebugUtilsMessageType::GENERAL
            | DebugUtilsMessageType::VALIDATION
            | DebugUtilsMessageType::PERFORMANCE,
        ..DebugUtilsMessengerCreateInfo::user_callback(unsafe {
            DebugUtilsMessengerCallback::new(|message_severity, message_type, callback_data| {
                let msg = format!(
                    "[{:?}] {} ({}): {}",
                    message_type,
                    callback_data.message_id_name.unwrap_or("unknown"),
                    callback_data.message_id_number,
                    callback_data.message
                );
                if message_severity.contains(DebugUtilsMessageSeverity::ERROR) {
                    log::error!("{msg}");
                } else if message_severity.contains(DebugUtilsMessageSeverity::WARNING) {
                    log::warn!("{msg}");
                } else if message_severity.contains(DebugUtilsMessageSeverity::INFO) {
                    log::info!("{msg}");
                } else if message_severity.contains(DebugUtilsMessageSeverity::VERBOSE) {
                    log::trace!("{msg}");
                } else {
                    // idk if this is desired
                    panic!("{msg}");
                }
            })
        })
    }
}

#[derive(Clone)]
struct Allocators {
    cmd: Arc<StandardCommandBufferAllocator>,
    mem: Arc<StandardMemoryAllocator>,
    set: Arc<StandardDescriptorSetAllocator>,
}

struct App {
    context: VulkanoContext,
    windows: VulkanoWindows,
    allocators: Allocators,
    triangle: Option<Triangle>,
}
impl App {
    fn new(event_loop: &EventLoop<()>) -> Self {
        let mut required_extensions = Surface::required_extensions(event_loop).unwrap();
        required_extensions.ext_debug_utils = true;
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..Default::default()
        };
        let debug_info = debug_info();
        let context = VulkanoContext::new(VulkanoConfig {
            instance_create_info: InstanceCreateInfo {
                enabled_extensions: required_extensions,
                enabled_layers: vec!["VK_LAYER_KHRONOS_validation".to_owned()],
                debug_utils_messengers: vec![debug_info.clone()],
                ..Default::default()
            },
            debug_create_info: Some(debug_info),
            device_extensions,
            print_device_name: true,
            device_priority_fn: Arc::new(|_| 0),
            ..Default::default()
        });

        let windows = VulkanoWindows::default();

        let cmd_buf_alloc = Arc::new(StandardCommandBufferAllocator::new(
            context.device().clone(),
            Default::default(),
        ));
        let set_alloc = Arc::new(StandardDescriptorSetAllocator::new(
            context.device().clone(),
            Default::default(),
        ));

        let allocators = Allocators {
            cmd: cmd_buf_alloc,
            mem: context.memory_allocator().clone(),
            set: set_alloc,
        };

        Self {
            context,
            windows,
            allocators,
            triangle: None,
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.windows.create_window(
            event_loop,
            &self.context,
            &Default::default(),
            |swapchain_info| {
                swapchain_info.image_format = Format::R8G8B8A8_SRGB;
            },
        );

        let triangle = Triangle::new(
            self.context.device().clone(),
            self.context
                .transfer_queue()
                .unwrap_or(self.context.graphics_queue())
                .clone(),
            self.allocators.clone(),
            self.windows
                .get_primary_renderer()
                .unwrap()
                .swapchain_format(),
        );
        self.triangle = Some(triangle);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let renderer = self.windows.get_primary_renderer_mut().unwrap();
        let triangle = self.triangle.as_mut().unwrap();

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                renderer.resize();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                renderer.resize();
            }
            WindowEvent::RedrawRequested => {
                match renderer.acquire(None, |_| {}) {
                    Ok(future) => {
                        let view = renderer.swapchain_image_view();
                        let framebuffer = Framebuffer::new(
                            triangle.render_pass.clone(),
                            FramebufferCreateInfo {
                                attachments: vec![view.clone()],
                                ..Default::default()
                            },
                        )
                        .unwrap();

                        let mut builder = AutoCommandBufferBuilder::primary(
                            self.allocators.cmd.clone(),
                            self.context.graphics_queue().queue_family_index(),
                            CommandBufferUsage::OneTimeSubmit,
                        )
                        .unwrap();

                        builder
                            .begin_render_pass(
                                RenderPassBeginInfo {
                                    clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into())],
                                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                                },
                                SubpassBeginInfo {
                                    contents: SubpassContents::Inline,
                                    ..Default::default()
                                },
                            )
                            .unwrap()
                            .set_viewport(
                                0,
                                [Viewport {
                                    offset: Default::default(),
                                    extent: [
                                        view.image().extent()[0] as f32,
                                        view.image().extent()[1] as f32,
                                    ],
                                    depth_range: 0.0..=1.0,
                                }]
                                .into_iter()
                                .collect(),
                            )
                            .unwrap();

                        triangle
                            .pipeline
                            .render(&mut builder, triangle.sets.to_vec());

                        builder.end_render_pass(Default::default()).unwrap();

                        let command_buffer = builder.build().unwrap();

                        let after_future = future
                            .then_execute(self.context.graphics_queue().clone(), command_buffer)
                            .unwrap();

                        renderer.present(after_future.boxed(), true);
                    }
                    Err(vulkano::VulkanError::OutOfDate) => {
                        renderer.resize();
                    }
                    Err(e) => panic!("Failed to acquire swapchain future: {}", e),
                };

                self.windows.get_primary_window().unwrap().request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let window = self.windows.get_primary_window().unwrap();
        window.request_redraw();
    }
}

struct Triangle {
    render_pass: Arc<RenderPass>,
    pipeline: CubemapRenderer,
    camera: OrbitCamera,
    allocators: Allocators,
    sets: [Arc<DescriptorSet>; 2],
}
impl Triangle {
    fn new(device: Arc<Device>, queue: Arc<Queue>, allocators: Allocators, format: Format) -> Self {
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    format: format,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {},
            }
        )
        .unwrap();
        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

        let camera = OrbitCamera::default();
        let camera_buffer = Buffer::from_data(
            allocators.mem.clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            cubemap::vs::Camera {
                view: camera.look_at().into(),
                proj: camera.perspective(1.0).into(),
            },
        )
        .unwrap();

        let pipeline = CubemapRenderer::new(device, subpass, allocators.mem.clone());
        let image = image::open("assets/skybox.hdr").unwrap().to_rgba32f();

        let view = {
            let stage_buffer = Buffer::new_slice(
                allocators.mem.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                image.as_bytes().len() as DeviceSize,
            )
            .unwrap();
            stage_buffer
                .write()
                .unwrap()
                .copy_from_slice(image.as_bytes());

            let image = Image::new(
                allocators.mem.clone(),
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    format: Format::R32G32B32A32_SFLOAT,
                    extent: [image.width(), image.height(), 1],
                    usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap();

            let mut builder = AutoCommandBufferBuilder::primary(
                allocators.cmd.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();
            builder
                .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                    stage_buffer,
                    image.clone(),
                ))
                .unwrap();
            let command_buffer = builder.build().unwrap();

            let _ = command_buffer.execute(queue.clone()).unwrap();

            ImageView::new_default(image).unwrap()
        };
        let sets = pipeline.create_sets(camera_buffer, view, allocators.set.clone());

        Self {
            render_pass,
            pipeline,
            camera,
            allocators,
            sets,
        }
    }
}

fn main() -> anyhow::Result<()> {
    colog::init();

    let event_loop = EventLoop::new()?;
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app)?;

    Ok(())
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "shaders/shader.vert",
    }
}
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "shaders/shader.frag",
    }
}
