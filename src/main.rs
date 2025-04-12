use egui_winit_vulkano::{Gui, GuiConfig};
use gltf_viewer::{Allocators, Triangle};
use std::sync::Arc;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassBeginInfo,
        SubpassContents, allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    device::DeviceExtensions,
    format::Format,
    instance::{
        InstanceCreateInfo,
        debug::{
            DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessengerCallback,
            DebugUtilsMessengerCreateInfo,
        },
    },
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

struct App {
    context: VulkanoContext,
    windows: VulkanoWindows,
    allocators: Allocators,
    gui: Option<Gui>,
    triangle: Option<Triangle>,
    render_pass: Option<Arc<RenderPass>>,
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
            render_pass: None,
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

        let render_pass = vulkano::single_pass_renderpass!(
            self.context.device().clone(),
            attachments: {
                color: {
                    format: renderer.swapchain_format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                depth_stencil: {
                    format: Format::D32_SFLOAT,
                    samples: 1,
                    load_op: Clear,
                    store_op: DontCare,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {depth_stencil}
            },
        )
        .unwrap();
        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

        let gui = Gui::new_with_subpass(
            event_loop,
            renderer.surface(),
            renderer.graphics_queue(),
            subpass,
            renderer.swapchain_format(),
            GuiConfig {
                allow_srgb_render_target: true,
                ..Default::default()
            },
        );

        let triangle = Triangle::new(
            self.context.device().clone(),
            self.context
                .transfer_queue()
                .unwrap_or(self.context.graphics_queue())
                .clone(),
            self.allocators.clone(),
            gui.render_resources().subpass,
            renderer.swapchain_image_size(),
        );

        self.gui = Some(gui);
        self.triangle = Some(triangle);
        self.render_pass = Some(render_pass);
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
        let render_pass = self.render_pass.as_mut().unwrap();

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

                match renderer.acquire(None, |view| {
                    let extent = view[0].image().extent();
                    triangle.resize([extent[0], extent[1]]);
                }) {
                    Ok(before_future) => {
                        let image = renderer.swapchain_image_view();

                        let dimensions = image.image().extent();
                        let framebuffer = Framebuffer::new(
                            render_pass.clone(),
                            FramebufferCreateInfo {
                                attachments: vec![image, triangle.depth_buffer.clone()],
                                ..Default::default()
                            },
                        )
                        .unwrap();

                        let mut builder = AutoCommandBufferBuilder::primary(
                            self.allocators.cmd.clone(),
                            renderer.graphics_queue().queue_family_index(),
                            CommandBufferUsage::OneTimeSubmit,
                        )
                        .unwrap();
                        builder
                            .begin_render_pass(
                                RenderPassBeginInfo {
                                    clear_values: vec![
                                        Some([0.0, 0.0, 0.0, 1.0].into()),
                                        Some(1f32.into()),
                                    ],
                                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                                },
                                SubpassBeginInfo {
                                    contents: SubpassContents::SecondaryCommandBuffers,
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                        let cb = gui.draw_on_subpass_image([dimensions[0], dimensions[1]]);
                        builder.execute_commands(cb).unwrap();
                        builder.end_render_pass(Default::default()).unwrap();

                        let command_buffer = builder.build().unwrap();
                        let after_future = before_future
                            .then_execute(renderer.graphics_queue(), command_buffer)
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

fn main() -> anyhow::Result<()> {
    colog::init();

    let event_loop = EventLoop::new()?;
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app)?;

    Ok(())
}
