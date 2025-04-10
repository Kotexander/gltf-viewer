use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk,
};
use std::{borrow::Cow, error::Error, ffi::CStr, u64};
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, EventLoop},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowAttributes,
};

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    unsafe {
        let callback_data = *p_callback_data;
        let message_id_number = callback_data.message_id_number;

        let message_id_name = if callback_data.p_message_id_name.is_null() {
            Cow::from("")
        } else {
            std::ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
        };

        let message = if callback_data.p_message.is_null() {
            Cow::from("")
        } else {
            std::ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy()
        };

        let msg = format!("{message_type:?} [{message_id_name} ({message_id_number})] : {message}");
        match message_severity {
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => {
                log::error!("{msg}");
            }
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
                log::warn!("{msg}");
            }
            vk::DebugUtilsMessageSeverityFlagsEXT::INFO => {
                log::info!("{msg}");
            }
            vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
                log::trace!("{msg}");
            }
            _ => {
                println!("{msg}");
            }
        }

        vk::FALSE
    }
}

struct MyInstance {
    debug_messenger: vk::DebugUtilsMessengerEXT,
    surface_loader: surface::Instance,
    debug_loader: debug_utils::Instance,
    instance: ash::Instance,
    entry: ash::Entry,
}
impl Drop for MyInstance {
    fn drop(&mut self) {
        unsafe {
            self.debug_loader
                .destroy_debug_utils_messenger(self.debug_messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}
impl MyInstance {
    fn new(app_info: vk::ApplicationInfo, event_loop: &EventLoop<()>) -> Self {
        unsafe {
            let entry = ash::Entry::load().unwrap();

            let layers = entry.enumerate_instance_layer_properties().unwrap();
            let extensions = entry.enumerate_instance_extension_properties(None).unwrap();

            log::trace!("Instance layer properties:");
            for layer in &layers {
                log::trace!(
                    "\t{:?} ({:?})",
                    layer.layer_name_as_c_str().unwrap(),
                    layer.description_as_c_str().unwrap(),
                );
            }
            log::trace!("Instance extension properties:");
            for extension in &extensions {
                log::trace!("\t{:?}", extension.extension_name_as_c_str().unwrap());
            }

            let mut enabled_extensions = ash_window::enumerate_required_extensions(
                event_loop.display_handle().unwrap().as_raw(),
            )
            .unwrap()
            .to_vec();
            enabled_extensions.push(debug_utils::NAME.as_ptr());

            let enabled_layers = [c"VK_LAYER_KHRONOS_validation".as_ptr()];
            for enabled_layer in &enabled_layers {
                let enabled_layer = CStr::from_ptr(*enabled_layer);
                if !layers
                    .iter()
                    .filter_map(|layer| layer.layer_name_as_c_str().ok())
                    .any(|layer| layer == enabled_layer)
                {
                    panic!("{:?} missing.", enabled_layer);
                }
            }

            log::info!("Instance enabled layers:");
            for enabled_layer in &enabled_layers {
                log::info!("\t{:?}", CStr::from_ptr(*enabled_layer));
            }
            log::info!("Instance enabled extensions:");
            for enabled_extension in &enabled_extensions {
                log::info!("\t{:?}", CStr::from_ptr(*enabled_extension));
            }

            let mut debug_create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                        | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
                    // | vk::DebugUtilsMessageTypeFlagsEXT::GENERAL,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let instance_create_info = vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_extension_names(&enabled_extensions)
                .enabled_layer_names(&enabled_layers)
                .push_next(&mut debug_create_info);

            let instance = entry.create_instance(&instance_create_info, None).unwrap();
            let surface_loader = surface::Instance::new(&entry, &instance);
            let debug_loader = debug_utils::Instance::new(&entry, &instance);

            let debug_messenger = debug_loader
                .create_debug_utils_messenger(&debug_create_info, None)
                .unwrap();

            Self {
                debug_messenger,
                surface_loader,
                debug_loader,
                instance,
                entry,
            }
        }
    }
}

struct MyDevice {
    queue: vk::Queue,
    queue_family: u32,
    device: ash::Device,
    swapchain_loader: swapchain::Device,
    physical_device: vk::PhysicalDevice,
}
impl Drop for MyDevice {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}
impl MyDevice {
    fn new(vk: &MyInstance, surface: vk::SurfaceKHR) -> Self {
        unsafe {
            let physical_devices = vk.instance.enumerate_physical_devices().unwrap();
            for d in &physical_devices {
                let p = vk.instance.get_physical_device_properties(*d);
                log::debug!(
                    "Physical device: {:?} (type {:#?})",
                    p.device_name_as_c_str().unwrap(),
                    p.device_type,
                );
                for q in vk.instance.get_physical_device_queue_family_properties(*d) {
                    log::trace!(
                        "\tQueue Family\n\t\tFlags({:?})\n\t\tcount = {}\n\t\tmin image transfer = {:?}",
                        q.queue_flags,
                        q.queue_count,
                        [
                            q.min_image_transfer_granularity.width,
                            q.min_image_transfer_granularity.height,
                            q.min_image_transfer_granularity.depth
                        ]
                    );
                }
            }

            let required_device_extensions = [ash::khr::swapchain::NAME.as_ptr()];
            log::info!("Required device extensions:");
            for re in &required_device_extensions {
                log::info!("\t{:?}", CStr::from_ptr(*re));
            }
            let (physical_device, physical_device_properties, queue_family) = physical_devices
                .into_iter()
                .map(|d| (d, vk.instance.get_physical_device_properties(d)))
                .filter(|(d, _)| {
                    let de = vk
                        .instance
                        .enumerate_device_extension_properties(*d)
                        .unwrap();

                    required_device_extensions
                        .iter()
                        .map(|re| CStr::from_ptr(*re))
                        .all(|re| {
                            de.iter()
                                .any(|de| de.extension_name_as_c_str().unwrap() == re)
                        })
                })
                .map(|(d, p)| {
                    let q = vk
                        .instance
                        .get_physical_device_queue_family_properties(d)
                        .into_iter()
                        .enumerate()
                        .find_map(|(qi, q)| {
                            let qi = qi as u32;
                            if q.queue_flags
                                .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::TRANSFER)
                                && vk
                                    .surface_loader
                                    .get_physical_device_surface_support(d, qi, surface)
                                    .unwrap()
                            {
                                Some(qi)
                            } else {
                                None
                            }
                        })
                        .unwrap() as u32;

                    (d, p, q)
                })
                .min_by_key(|(_, p, _)| match p.device_type {
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 0,
                    vk::PhysicalDeviceType::DISCRETE_GPU => 1,
                    vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                    vk::PhysicalDeviceType::CPU => 3,
                    vk::PhysicalDeviceType::OTHER => 4,
                    _ => 6,
                })
                .unwrap();

            log::info!(
                "Physical device chosen: {:?} using queue {}",
                physical_device_properties.device_name_as_c_str().unwrap(),
                queue_family,
            );

            let queue_priorities = [1.0];
            let queue_create_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family)
                .queue_priorities(&queue_priorities);

            let queue_create_infos = [queue_create_info];
            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&required_device_extensions);

            let device = vk
                .instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap();
            let queue = device.get_device_queue(queue_family, 0);

            let swapchain_loader = swapchain::Device::new(&vk.instance, &device);

            Self {
                queue,
                queue_family,
                device,
                physical_device,
                swapchain_loader,
            }
        }
    }
    fn create_shader_module(&self, path: &str, kind: shaderc::ShaderKind) -> vk::ShaderModule {
        let compiler = shaderc::Compiler::new().unwrap();
        let source = std::fs::read_to_string(path).unwrap();
        let code = compiler
            .compile_into_spirv(&source, kind, path, "main", None)
            .unwrap();
        if code.get_num_warnings() > 0 {
            log::warn!("{}", code.get_warning_messages());
        }
        let create_info = vk::ShaderModuleCreateInfo::default().code(code.as_binary());

        unsafe {
            self.device
                .create_shader_module(&create_info, None)
                .unwrap()
        }
    }
}

struct SwapChainSupport {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}
impl SwapChainSupport {
    pub fn new(
        surface_loader: &surface::Instance,
        device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
    ) -> Self {
        unsafe {
            let capabilities = surface_loader
                .get_physical_device_surface_capabilities(device, surface)
                .unwrap();

            let formats = surface_loader
                .get_physical_device_surface_formats(device, surface)
                .unwrap();

            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(device, surface)
                .unwrap();

            Self {
                capabilities,
                formats,
                present_modes,
            }
        }
    }
}

struct App {
    window: Option<Window>,
    vk: MyInstance,
}
impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            if let Some(window) = &self.window {
                window.device.device.device_wait_idle().unwrap();
                window
                    .device
                    .device
                    .destroy_semaphore(window.image_available, None);
                window
                    .device
                    .device
                    .destroy_semaphore(window.render_finished, None);
                window.device.device.destroy_fence(window.in_flight, None);
                window
                    .device
                    .device
                    .destroy_command_pool(window.command_pool, None);
                for framebuffer in &window.frame_buffers {
                    window.device.device.destroy_framebuffer(*framebuffer, None);
                }
                window.device.device.destroy_pipeline(window.pipeline, None);
                window
                    .device
                    .device
                    .destroy_render_pass(window.render_pass, None);
                window
                    .device
                    .device
                    .destroy_pipeline_layout(window.pipeline_layout, None);
                for view in &window.swapchain_views {
                    window.device.device.destroy_image_view(*view, None);
                }
                window
                    .device
                    .swapchain_loader
                    .destroy_swapchain(window.swapchain, None);
                self.vk.surface_loader.destroy_surface(window.surface, None);
            }
        }
    }
}

struct Window {
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    in_flight: vk::Fence,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    pipeline: vk::Pipeline,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_views: Vec<vk::ImageView>,
    frame_buffers: Vec<vk::Framebuffer>,
    swapchain_extent: vk::Extent2D,
    swapchain_format: vk::Format,
    surface: vk::SurfaceKHR,
    device: MyDevice,
    window: winit::window::Window,
}
impl Window {
    pub fn new(vk: &MyInstance, event_loop: &ActiveEventLoop) -> Self {
        unsafe {
            let window = event_loop
                .create_window(WindowAttributes::default().with_title("glTF Viewer"))
                .unwrap();
            let size = window.inner_size();

            let surface = ash_window::create_surface(
                &vk.entry,
                &vk.instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )
            .unwrap();

            let device = MyDevice::new(&vk, surface);

            let swapchain_support =
                SwapChainSupport::new(&vk.surface_loader, device.physical_device, surface);

            log::debug!("{:#?}", swapchain_support.capabilities);
            log::debug!("Surface formats: {:#?}", swapchain_support.formats);
            log::debug!("Present modes: {:#?}", swapchain_support.present_modes);

            let swapchain_format = swapchain_support
                .formats
                .iter()
                .find(|format| {
                    [vk::Format::B8G8R8A8_SRGB, vk::Format::R8G8B8A8_SRGB].contains(&format.format)
                        && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                })
                .copied()
                .unwrap_or(swapchain_support.formats[0]);
            log::info!("Using format: {swapchain_format:?}");
            let present_mode = swapchain_support.present_modes[0];
            log::info!("Using present mode: {present_mode:?}");

            let swapchain_extent = vk::Extent2D {
                width: size.width.clamp(
                    swapchain_support.capabilities.min_image_extent.width,
                    swapchain_support.capabilities.max_image_extent.width,
                ),
                height: size.height.clamp(
                    swapchain_support.capabilities.min_image_extent.height,
                    swapchain_support.capabilities.max_image_extent.height,
                ),
            };

            let mut image_count = swapchain_support.capabilities.min_image_count + 1;
            if swapchain_support.capabilities.max_image_count > 0 {
                image_count = image_count.max(swapchain_support.capabilities.max_image_count)
            }

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(surface)
                .image_format(swapchain_format.format)
                .image_color_space(swapchain_format.color_space)
                .present_mode(present_mode)
                .image_extent(swapchain_extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .min_image_count(image_count)
                .pre_transform(swapchain_support.capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .clipped(true);

            let swapchain = device
                .swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let swapchain_images = device
                .swapchain_loader
                .get_swapchain_images(swapchain)
                .unwrap();

            let swapchain_views: Vec<_> = swapchain_images
                .iter()
                .map(|image| {
                    let view_create_info = vk::ImageViewCreateInfo::default()
                        .image(*image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(swapchain_format.format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        );
                    device
                        .device
                        .create_image_view(&view_create_info, None)
                        .unwrap()
                })
                .collect();

            let vert_shader =
                device.create_shader_module("shaders/shader.vert", shaderc::ShaderKind::Vertex);
            let frag_shader =
                device.create_shader_module("shaders/shader.frag", shaderc::ShaderKind::Fragment);

            let vert_shader_stage_info = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .name(c"main")
                .module(vert_shader);
            let frag_shader_stage_info = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .name(c"main")
                .module(frag_shader);

            let shader_stages = [vert_shader_stage_info, frag_shader_stage_info];

            let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let viewport = [vk::Viewport::default()
                .width(swapchain_extent.width as f32)
                .height(swapchain_extent.height as f32)
                .max_depth(1.0)];
            let scissor = [vk::Rect2D::default().extent(swapchain_extent)];
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewports(&viewport)
                .scissors(&scissor);
            let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .cull_mode(vk::CullModeFlags::NONE)
                .line_width(1.0);
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA)];
            let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(&color_blend_attachment);

            let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default();
            let pipeline_layout = device
                .device
                .create_pipeline_layout(&pipeline_layout_create_info, None)
                .unwrap();

            let color_attachment = [vk::AttachmentDescription::default()
                .format(swapchain_format.format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

            let attachment_reference = [vk::AttachmentReference::default()
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

            let subpass_desc =
                [vk::SubpassDescription::default().color_attachments(&attachment_reference)];

            let subpass_dep = [vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
            let render_pass_info = vk::RenderPassCreateInfo::default()
                .attachments(&color_attachment)
                .subpasses(&subpass_desc);
            // .dependencies(&subpass_dep);

            let render_pass = device
                .device
                .create_render_pass(&render_pass_info, None)
                .unwrap();

            let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&shader_stages)
                .vertex_input_state(&vertex_input_info)
                .input_assembly_state(&input_assembly_info)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterizer)
                .multisample_state(&multisampling)
                .color_blend_state(&color_blending)
                .layout(pipeline_layout)
                .render_pass(render_pass);

            let pipeline = device
                .device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .unwrap()[0];

            device.device.destroy_shader_module(vert_shader, None);
            device.device.destroy_shader_module(frag_shader, None);

            let frame_buffers = swapchain_views
                .iter()
                .map(|view| {
                    let view = [*view];
                    let frame_buffer_info = vk::FramebufferCreateInfo::default()
                        .render_pass(render_pass)
                        .attachments(&view)
                        .width(swapchain_extent.width)
                        .height(swapchain_extent.height)
                        .layers(1);
                    device
                        .device
                        .create_framebuffer(&frame_buffer_info, None)
                        .unwrap()
                })
                .collect();

            let command_pool_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(device.queue_family);

            let command_pool = device
                .device
                .create_command_pool(&command_pool_info, None)
                .unwrap();

            let command_buffer_alloc_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let command_buffer = device
                .device
                .allocate_command_buffers(&command_buffer_alloc_info)
                .unwrap()[0];

            let semaphore_info = vk::SemaphoreCreateInfo::default();
            let image_available = device
                .device
                .create_semaphore(&semaphore_info, None)
                .unwrap();
            let render_finished = device
                .device
                .create_semaphore(&semaphore_info, None)
                .unwrap();
            let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            let in_flight = device.device.create_fence(&fence_info, None).unwrap();

            Self {
                swapchain,
                swapchain_images,
                swapchain_views,
                swapchain_extent,
                swapchain_format: swapchain_format.format,
                surface,
                device,
                window,
                render_pass,
                pipeline_layout,
                pipeline,
                frame_buffers,
                command_pool,
                command_buffer,
                image_available,
                render_finished,
                in_flight,
            }
        }
    }
    fn render(&self) {
        unsafe {
            self.device
                .device
                .wait_for_fences(&[self.in_flight], true, u64::MAX)
                .unwrap();
            self.device.device.reset_fences(&[self.in_flight]).unwrap();
            let (index, _) = self
                .device
                .swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    self.image_available,
                    vk::Fence::null(),
                )
                .unwrap();
            self.device
                .device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();

            let begin_info = vk::CommandBufferBeginInfo::default();
            self.device
                .device
                .begin_command_buffer(self.command_buffer, &begin_info)
                .unwrap();

            let mut clear_color = vk::ClearValue::default();
            clear_color.color.float32 = [0.0, 0.0, 0.0, 1.0];
            let clear_color = [clear_color];

            let render_pass_info = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.frame_buffers[index as usize])
                .render_area(vk::Rect2D::default().extent(self.swapchain_extent))
                .clear_values(&clear_color);
            self.device.device.cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );
            self.device.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );
            // let viewport = [vk::Viewport {
            //     x: 0.0,
            //     y: 0.0,
            //     width: self.swapchain_extent.width as f32,
            //     height: self.swapchain_extent.height as f32,
            //     min_depth: 0.0,
            //     max_depth: 1.0,
            // }];
            // let scissor = [vk::Rect2D {
            //     offset: vk::Offset2D::default(),
            //     extent: self.swapchain_extent,
            // }];
            // self.device
            //     .device
            //     .cmd_set_viewport(self.command_buffer, 0, &viewport);
            // self.device
            //     .device
            //     .cmd_set_scissor(self.command_buffer, 0, &scissor);

            self.device.device.cmd_draw(self.command_buffer, 3, 1, 0, 0);

            self.device.device.cmd_end_render_pass(self.command_buffer);
            self.device
                .device
                .end_command_buffer(self.command_buffer)
                .unwrap();
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let wait_semaphores = [self.image_available];
            let buffers = [self.command_buffer];
            let signal_semaphores = [self.render_finished];
            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&buffers)
                .signal_semaphores(&signal_semaphores);
            self.device
                .device
                .queue_submit(self.device.queue, &[submit_info], self.in_flight)
                .unwrap();

            let swapchains = [self.swapchain];
            let index = [index];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&index);
            self.device
                .swapchain_loader
                .queue_present(self.device.queue, &present_info)
                .unwrap();
        }
    }
}

impl App {
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        let app_info = vk::ApplicationInfo::default()
            .api_version(vk::make_api_version(1, 0, 0, 0))
            .application_name(c"glTF Viewer");

        Self {
            vk: MyInstance::new(app_info, event_loop),
            window: None,
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.window = Some(Window::new(&self.vk, event_loop));
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let window = self.window.as_mut().unwrap();
        match event {
            winit::event::WindowEvent::RedrawRequested => {
                window.render();
                window.window.request_redraw();
            }
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    colog::init();

    let event_loop = EventLoop::new()?;
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app)?;

    Ok(())
}
