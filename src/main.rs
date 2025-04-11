mod camera;
mod cubemap;

use camera::OrbitCamera;
use cubemap::CubemapRenderer;
use egui_winit_vulkano::{CallbackFn, Gui, GuiConfig};
use image::EncodableLayout;
use std::sync::Arc;
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        PrimaryCommandBufferAbstract, allocator::StandardCommandBufferAllocator,
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
    render_pass::Subpass,
    swapchain::Surface,
};
use vulkano_util::{
    context::{VulkanoConfig, VulkanoContext},
    window::VulkanoWindows,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, WindowEvent},
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
    gui: Option<Gui>,
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
            gui: None,
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
                // swapchain_info.present_mode = PresentMode::Mailbox;
                // swapchain_info.min_image_count = 5;
            },
        );
        let renderer = self.windows.get_primary_renderer_mut().unwrap();

        let gui = Gui::new(
            event_loop,
            renderer.surface(),
            renderer.graphics_queue(),
            renderer.swapchain_format(),
            GuiConfig {
                allow_srgb_render_target: true,
                ..Default::default()
            },
        );
        self.gui = Some(gui);

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
        let gui = self.gui.as_mut().unwrap();

        gui.update(&event);
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
                gui.immediate_ui(|gui| {
                    let ctx = gui.context();

                    egui::CentralPanel::default().show(&ctx, |ui| {
                        triangle.ui(ui);
                    });
                });

                match renderer.acquire(None, |_| {}) {
                    Ok(before_future) => {
                        let after_future =
                            gui.draw_on_image(before_future, renderer.swapchain_image_view());
                        renderer.present(after_future, true);
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

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.gui.as_mut().unwrap().egui_winit.on_mouse_motion(delta);
        }
    }
}

#[derive(Clone)]
struct Renderer {
    pipeline: CubemapRenderer,
    sets: Vec<Arc<DescriptorSet>>,
}
impl Renderer {
    fn callback(
        self,
        rect: egui::Rect,
        camera: OrbitCamera,
        camera_buffer: Subbuffer<cubemap::vs::Camera>,
    ) -> egui::PaintCallback {
        egui::PaintCallback {
            rect,
            callback: Arc::new(CallbackFn::new(move |info, context| {
                let camera = cubemap::vs::Camera {
                    view: camera.look_at().into(),
                    proj: camera.perspective(info.viewport.aspect_ratio()).into(),
                };

                let mut buffer = camera_buffer.write().unwrap();
                *buffer = camera;

                self.pipeline.render(context.builder, self.sets.to_vec());
            })),
        }
    }
}

struct Triangle {
    camera: OrbitCamera,
    camera_buffer: Subbuffer<cubemap::vs::Camera>,
    allocators: Allocators,
    renderer: Renderer,
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
        let sets = pipeline
            .create_sets(camera_buffer.clone(), view, allocators.set.clone())
            .to_vec();

        Self {
            camera,
            allocators,
            camera_buffer,
            renderer: Renderer { pipeline, sets },
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::all());

            let drag_delta = response.drag_motion() * 0.001 * self.camera.zoom;
            self.camera.pitch += drag_delta.y;
            self.camera.yaw += drag_delta.x;
            self.camera.wrap();

            let paint_callback =
                self.renderer
                    .clone()
                    .callback(rect, self.camera, self.camera_buffer.clone());
            ui.painter().add(paint_callback);
        });
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
