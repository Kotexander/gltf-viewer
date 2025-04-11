use camera::OrbitCamera;
use cubemap::{cubemap::CubemapRenderer, equirectangular::EquirectangularRenderer};
use egui_winit_vulkano::CallbackFn;
use image::EncodableLayout;
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        PrimaryCommandBufferAbstract, allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{DescriptorSet, allocator::StandardDescriptorSetAllocator},
    device::{Device, Queue},
    format::Format,
    image::{
        Image, ImageCreateFlags, ImageCreateInfo, ImageType, ImageUsage,
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    render_pass::Subpass,
};

mod camera;
mod cubemap;

#[derive(Clone)]
pub struct Allocators {
    pub cmd: Arc<StandardCommandBufferAllocator>,
    pub mem: Arc<StandardMemoryAllocator>,
    pub set: Arc<StandardDescriptorSetAllocator>,
}

#[derive(Clone)]
struct EquiRenderer {
    pipeline: EquirectangularRenderer,
    sets: Vec<Arc<DescriptorSet>>,
}
impl EquiRenderer {
    fn callback(
        self,
        rect: egui::Rect,
        camera: OrbitCamera,
        camera_buffer: Subbuffer<[glm::Mat4; 2]>,
    ) -> egui::PaintCallback {
        egui::PaintCallback {
            rect,
            callback: Arc::new(CallbackFn::new(move |info, context| {
                let mut buffer = camera_buffer.write().unwrap();
                *buffer = [
                    camera.look_at(),
                    camera.perspective(info.viewport.aspect_ratio()),
                ];

                self.pipeline.render(context.builder, self.sets.to_vec());
            })),
        }
    }
}

#[derive(Clone)]
struct CubeRenderer {
    pipeline: CubemapRenderer,
    sets: Vec<Arc<DescriptorSet>>,
}
impl CubeRenderer {
    fn callback(
        self,
        rect: egui::Rect,
        camera: OrbitCamera,
        camera_buffer: Subbuffer<[glm::Mat4; 2]>,
    ) -> egui::PaintCallback {
        egui::PaintCallback {
            rect,
            callback: Arc::new(CallbackFn::new(move |info, context| {
                let mut buffer = camera_buffer.write().unwrap();
                *buffer = [
                    camera.look_at(),
                    camera.perspective(info.viewport.aspect_ratio()),
                ];

                self.pipeline.render(context.builder, self.sets.to_vec());
            })),
        }
    }
}

pub struct Triangle {
    pub camera: OrbitCamera,
    pub mode: bool,

    camera_buffer: Subbuffer<[glm::Mat4; 2]>,
    equi_renderer: EquiRenderer,
    cube_renderer: CubeRenderer,
    allocators: Allocators,
}
impl Triangle {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        allocators: Allocators,
        subpass: Subpass,
    ) -> Self {
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
            [camera.look_at(), camera.perspective(1.0)],
        )
        .unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            allocators.cmd.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let equi_image = load_skybox(allocators.mem.clone(), &mut builder);
        let cube_image = load_cubemap(allocators.mem.clone(), &mut builder);

        let command_buffer = builder.build().unwrap();
        let _ = command_buffer.execute(queue.clone()).unwrap();

        let equi_view = ImageView::new_default(equi_image).unwrap();
        let cube_view = ImageView::new(
            cube_image.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                format: cube_image.format(),
                subresource_range: cube_image.subresource_range(),
                ..Default::default()
            },
        )
        .unwrap();

        let equi_pipeline =
            EquirectangularRenderer::new(device.clone(), subpass.clone(), allocators.mem.clone());
        let cube_pipeline = CubemapRenderer::new(device.clone(), subpass, allocators.mem.clone());

        let equi_sets = equi_pipeline
            .create_sets(camera_buffer.clone(), equi_view, allocators.set.clone())
            .to_vec();
        let cube_sets = cube_pipeline
            .create_sets(camera_buffer.clone(), cube_view, allocators.set.clone())
            .to_vec();

        Self {
            camera,
            allocators,
            camera_buffer,
            equi_renderer: EquiRenderer {
                pipeline: equi_pipeline,
                sets: equi_sets,
            },
            cube_renderer: CubeRenderer {
                pipeline: cube_pipeline,
                sets: cube_sets,
            },
            mode: false,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.toggle_value(&mut self.mode, "text");

        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::all());

            let drag_delta = response.drag_motion() * 0.001 * self.camera.zoom;
            self.camera.pitch += drag_delta.y;
            self.camera.yaw += drag_delta.x;
            self.camera.wrap();

            if self.mode {
                let paint_callback = self.equi_renderer.clone().callback(
                    rect,
                    self.camera,
                    self.camera_buffer.clone(),
                );
                ui.painter().add(paint_callback);
            } else {
                let paint_callback = self.cube_renderer.clone().callback(
                    rect,
                    self.camera,
                    self.camera_buffer.clone(),
                );
                ui.painter().add(paint_callback);
            }
        });
    }
}

fn load_cubemap<L>(
    mem_alloc: Arc<StandardMemoryAllocator>,
    builder: &mut AutoCommandBufferBuilder<L>,
) -> Arc<Image> {
    let paths = [
        "assets/Yokohama/posx.jpg",
        "assets/Yokohama/negx.jpg",
        "assets/Yokohama/posy.jpg",
        "assets/Yokohama/negy.jpg",
        "assets/Yokohama/posz.jpg",
        "assets/Yokohama/negz.jpg",
    ];
    let images: Vec<_> = paths
        .iter()
        .map(|path| image::open(path).unwrap().to_rgba8())
        .collect();

    let dimensions = images[0].dimensions();
    for image in &images {
        if image.dimensions() != dimensions {
            panic!();
        }
    }

    let one_image_size = dimensions.0 * dimensions.1 * 4;
    let staging_buffer = Buffer::new_slice(
        mem_alloc.clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        (one_image_size * 6) as DeviceSize,
    )
    .unwrap();

    let mut write = staging_buffer.write().unwrap();
    for (i, image) in images.iter().enumerate() {
        write[(i * one_image_size as usize)..((i + 1) * one_image_size as usize)]
            .copy_from_slice(image.as_bytes());
    }
    drop(write);

    let image = Image::new(
        mem_alloc,
        ImageCreateInfo {
            flags: ImageCreateFlags::CUBE_COMPATIBLE,
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            image_type: ImageType::Dim2d,
            array_layers: 6,
            extent: [dimensions.0, dimensions.1, 1],
            format: Format::R8G8B8A8_SRGB,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap();

    builder
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            staging_buffer,
            image.clone(),
        ))
        .unwrap();

    image
}

fn load_skybox<L>(
    mem_alloc: Arc<StandardMemoryAllocator>,
    builder: &mut AutoCommandBufferBuilder<L>,
) -> Arc<Image> {
    let path = "assets/skybox8k.hdr";
    // let mut reader = BufReader::new(std::fs::File::open(path).unwrap());
    // let mut image_reader = image::ImageReader::new(&mut reader)
    //     .with_guessed_format()
    //     .unwrap();
    // image_reader.no_limits();
    // let image = image_reader.decode().unwrap().to_rgba32f();

    let image = image::open(path).unwrap().to_rgba32f();

    let stage_buffer = Buffer::new_slice(
        mem_alloc.clone(),
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
        mem_alloc,
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

    builder
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            stage_buffer,
            image.clone(),
        ))
        .unwrap();

    image
}
