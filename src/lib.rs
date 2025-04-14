use camera::OrbitCamera;
use egui_winit_vulkano::CallbackFn;
use image::EncodableLayout;
use nalgebra_glm as glm;
use renderer::Renderer;
use std::{path::PathBuf, sync::Arc, thread::JoinHandle};
use viewer::{GltfRenderInfo, loader::GltfLoader};
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer,
        allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
    },
    device::Queue,
    format::Format,
    image::{Image, ImageCreateFlags, ImageCreateInfo, ImageType, ImageUsage},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::PipelineBindPoint,
    render_pass::Subpass,
};

mod camera;
mod cubemap;
mod renderer;
mod viewer;

#[derive(Clone)]
pub struct Allocators {
    pub cmd: Arc<StandardCommandBufferAllocator>,
    pub mem: Arc<StandardMemoryAllocator>,
    pub set: Arc<StandardDescriptorSetAllocator>,
}

pub struct Triangle {
    camera: OrbitCamera,
    camera_buffer: Subbuffer<[glm::Mat4; 2]>,
    camera_set: Arc<DescriptorSet>,

    renderer: Renderer,

    gltf_job: Option<JoinHandle<(GltfRenderInfo, Arc<PrimaryAutoCommandBuffer>)>>,
    skybox_job: Option<JoinHandle<(Arc<Image>, Arc<PrimaryAutoCommandBuffer>)>>,

    allocators: Allocators,
}
impl Triangle {
    pub fn new(allocators: Allocators, subpass: Subpass) -> Self {
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

        let renderer = Renderer::new(allocators.mem.clone(), subpass);

        let camera_set = DescriptorSet::new(
            allocators.set.clone(),
            renderer.camera_set_layout.clone(),
            [WriteDescriptorSet::buffer(0, camera_buffer.clone())],
            [],
        )
        .unwrap();

        Self {
            camera,
            allocators,
            camera_buffer,
            renderer,
            camera_set,
            gltf_job: None,
            skybox_job: None,
        }
    }

    pub fn load_gltf(&mut self, path: PathBuf, queue: Arc<Queue>) {
        if self.gltf_job.is_some() {
            return;
        }
        let allocators = self.allocators.clone();
        let material_set_layout = self.renderer.material_set_layout.clone();
        let job = std::thread::spawn(move || {
            let loader = GltfLoader::new(
                allocators.clone(),
                material_set_layout,
                queue.queue_family_index(),
                &path,
            )
            .unwrap();

            let info = GltfRenderInfo::from_scene(
                allocators.mem.clone(),
                loader.document.default_scene().unwrap(),
                &loader.meshes,
            );
            (info, loader.cb)
        });
        self.gltf_job = Some(job);
    }
    pub fn loading(&self) -> bool {
        self.gltf_job.is_some()
    }
    pub fn update(&mut self) -> Option<Arc<PrimaryAutoCommandBuffer>> {
        if self.gltf_job.as_ref().is_some_and(|job| job.is_finished()) {
            let (info, cb) = self.gltf_job.take().unwrap().join().unwrap();
            self.renderer.gltf_info = Some(info);
            Some(cb)
        } else {
            None
        }
    }

    pub fn side(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Camera", |ui| {
            ui.label("Pitch");
            ui.drag_angle(&mut self.camera.pitch);
            ui.label("Yaw");
            ui.drag_angle(&mut self.camera.yaw);

            ui.separator();

            ui.label("Near");
            let diff = 0.01;
            let old_near = self.camera.near;
            ui.add(
                egui::DragValue::new(&mut self.camera.near)
                    .range(diff..=self.camera.far - diff)
                    .speed(0.1),
            );
            ui.label("Far");
            ui.add(
                egui::DragValue::new(&mut self.camera.far)
                    .range(old_near + diff..=f32::INFINITY)
                    .speed(0.1),
            );

            ui.separator();

            ui.label("Target");
            ui.add(
                egui::DragValue::new(&mut self.camera.target.x)
                    .prefix("x: ")
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.camera.target.y)
                    .prefix("y: ")
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.camera.target.z)
                    .prefix("z: ")
                    .speed(0.1),
            );
        });

        ui.collapsing("Skybox", |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!self.renderer.cube_mode, "Equirectangular")
                    .clicked()
                {
                    self.renderer.cube_mode = false;
                }
                if ui
                    .selectable_label(self.renderer.cube_mode, "Cubemap")
                    .clicked()
                {
                    self.renderer.cube_mode = true;
                }
            })
        });
    }
    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::all());

                let drag_delta = response.drag_motion() * 0.005;
                self.camera.pitch += drag_delta.y;
                self.camera.yaw -= drag_delta.x;
                self.camera.wrap();

                let smooth_scroll = response.ctx.input(|i| i.smooth_scroll_delta);
                self.camera.zoom += self.camera.zoom * -smooth_scroll.y * 0.003;
                self.camera.clamp();

                let mut buffer = self.camera_buffer.write().unwrap();
                *buffer = [
                    self.camera.look_at(),
                    self.camera.perspective(rect.aspect_ratio()),
                ];

                let renderer = self.renderer.clone();
                let camera_set = self.camera_set.clone();
                let callback = egui::PaintCallback {
                    rect,
                    callback: Arc::new(CallbackFn::new(move |_info, context| {
                        context
                            .builder
                            .bind_descriptor_sets(
                                PipelineBindPoint::Graphics,
                                renderer.gltf_pipeline.layout().clone(),
                                0,
                                camera_set.clone(),
                            )
                            .unwrap();
                        renderer.clone().render(context.builder);
                    })),
                };
                ui.painter().add(callback);
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
