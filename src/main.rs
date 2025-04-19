use egui_winit_vulkano::{Gui, GuiConfig};
use frameinfo::FrameInfo;
use gltf_viewer::{Allocators, State};
use std::sync::Arc;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, SubpassBeginInfo, SubpassContents,
        allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo},
    },
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    device::DeviceExtensions,
    format::Format,
    image::ImageUsage,
    instance::{
        InstanceCreateInfo,
        debug::{
            DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessengerCallback,
            DebugUtilsMessengerCreateInfo,
        },
    },
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

mod frameinfo;

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

struct Window {
    gui: Gui,
    frame_info: FrameInfo,
    state: State,
    frame: usize,
    num_frames: usize,
}
impl Window {
    pub fn frame_index(&self) -> usize {
        self.frame % self.num_frames
    }
}

struct App {
    context: VulkanoContext,
    windows: VulkanoWindows,
    allocators: Allocators,
    window: Option<Window>,
}
impl App {
    fn new(event_loop: &EventLoop<()>) -> Self {
        let debug_info = if cfg!(debug_assertions) {
            Some(debug_info())
        } else {
            None
        };
        let mut required_extensions = Surface::required_extensions(event_loop).unwrap();
        if debug_info.is_some() {
            required_extensions.ext_debug_utils = true;
        }
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..Default::default()
        };
        let context = VulkanoContext::new(VulkanoConfig {
            instance_create_info: InstanceCreateInfo {
                enabled_extensions: required_extensions,
                enabled_layers: if debug_info.is_some() {
                    vec!["VK_LAYER_KHRONOS_validation".to_owned()]
                } else {
                    vec![]
                },
                debug_utils_messengers: debug_info
                    .clone()
                    .map(|info| vec![info])
                    .unwrap_or_default(),
                ..Default::default()
            },
            debug_create_info: debug_info,
            device_extensions,
            print_device_name: true,
            device_priority_fn: Arc::new(|_| 0),
            ..Default::default()
        });

        let windows = VulkanoWindows::default();

        let cmd_allocator = Arc::new(StandardCommandBufferAllocator::new(
            context.device().clone(),
            StandardCommandBufferAllocatorCreateInfo {
                primary_buffer_count: 16,
                secondary_buffer_count: 16,
                ..Default::default()
            },
        ));
        let set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
            context.device().clone(),
            Default::default(),
        ));

        let allocators = Allocators {
            cmd: cmd_allocator,
            mem: context.memory_allocator().clone(),
            set: set_allocator,
        };

        Self {
            context,
            windows,
            allocators,
            window: None,
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
                swapchain_info.image_format = Format::B8G8R8A8_SRGB;
                // swapchain_info.image_format = Format::B8G8R8A8_UNORM;
                swapchain_info.image_usage |= ImageUsage::TRANSFER_DST;
                swapchain_info.min_image_count += 1;
                // swapchain_info.present_mode = vulkano::swapchain::PresentMode::Mailbox;
            },
        );
        let renderer = self.windows.get_primary_renderer_mut().unwrap();

        let frame_info = FrameInfo::new(
            self.allocators.mem.clone(),
            renderer.swapchain_image_views(),
        );

        let gui = Gui::new_with_subpass(
            event_loop,
            renderer.surface(),
            renderer.graphics_queue(),
            frame_info.subpass().clone(),
            renderer.swapchain_format(),
            GuiConfig {
                allow_srgb_render_target: true,
                ..Default::default()
            },
        );

        let num_frames = renderer.swapchain_image_views().len();

        let state = State::new(
            &self.allocators,
            self.context.graphics_queue().clone(),
            num_frames,
            frame_info.subpass().clone(),
        );

        self.window = Some(Window {
            gui,
            frame_info,
            state,
            frame: 0,
            num_frames,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let renderer = self.windows.get_primary_renderer_mut().unwrap();
        let window = self.window.as_mut().unwrap();

        window.gui.update(&event);
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
                let frame_index = window.frame_index();
                window.frame += 1;

                window.gui.immediate_ui(|gui| {
                    window.state.show(&gui.egui_ctx, frame_index);
                });

                match renderer.acquire(None, |views| {
                    window.frame_info.recreate(views);
                }) {
                    Ok(before_future) => {
                        let mut builder = AutoCommandBufferBuilder::primary(
                            self.allocators.cmd.clone(),
                            renderer.graphics_queue().queue_family_index(),
                            CommandBufferUsage::OneTimeSubmit,
                        )
                        .unwrap();

                        window.state.update(&mut builder, frame_index);

                        builder
                            .begin_render_pass(
                                window
                                    .frame_info
                                    .render_pass_info(renderer.image_index() as usize),
                                SubpassBeginInfo {
                                    contents: SubpassContents::SecondaryCommandBuffers,
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                        let cb = window
                            .gui
                            .draw_on_subpass_image(renderer.swapchain_image_size());
                        builder.execute_commands(cb).unwrap();
                        builder.end_render_pass(Default::default()).unwrap();

                        let command_buffer = builder.build().unwrap();
                        let after_future = before_future
                            .then_execute(renderer.graphics_queue(), command_buffer)
                            .unwrap();

                        renderer.present(after_future.boxed(), false);
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
            self.window
                .as_mut()
                .unwrap()
                .gui
                .egui_winit
                .on_mouse_motion(delta);
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
