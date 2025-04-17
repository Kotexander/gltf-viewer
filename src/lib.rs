use camera::OrbitCamera;
use cubemap::renderer::create_cubemap_image;
use egui_winit_vulkano::CallbackFn;
use image::EncodableLayout;
use nalgebra_glm as glm;
use renderer::Renderer;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    thread::JoinHandle,
};
use viewer::{GltfRenderInfo, loader::GltfLoader};
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract,
        allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
    },
    device::{DeviceOwned, Queue},
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageSubresourceRange, ImageType, ImageUsage,
        sampler::{Sampler, SamplerCreateInfo},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::PipelineBindPoint,
    render_pass::Subpass,
    sync::GpuFuture,
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
    camera_buffer: Subbuffer<[glm::Mat4; 3]>,
    camera_set: Arc<DescriptorSet>,

    renderer: Renderer,

    gltf_job: Option<JoinHandle<(GltfRenderInfo, Arc<PrimaryAutoCommandBuffer>)>>,
    skybox_job: Option<
        JoinHandle<(
            (Arc<DescriptorSet>, Arc<DescriptorSet>),
            Arc<PrimaryAutoCommandBuffer>,
        )>,
    >,

    allocators: Allocators,
}
impl Triangle {
    pub fn new(allocators: Allocators, queue: Arc<Queue>, subpass: Subpass) -> Self {
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
            [
                camera.look_at(),
                camera.perspective(1.0),
                camera.look_at().try_inverse().unwrap(),
            ],
        )
        .unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            allocators.cmd.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        let renderer = Renderer::new(allocators.clone(), &mut builder, subpass);
        let cb = builder.build().unwrap();
        let _ = cb
            .execute(queue)
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None);

        let camera_set = DescriptorSet::new(
            allocators.set.clone(),
            renderer.set_layouts.camera.clone(),
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
        let material_set_layout = self.renderer.set_layouts.material.clone();

        let job = std::thread::spawn(move || {
            let builder = AutoCommandBufferBuilder::primary(
                allocators.cmd.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();
            let loader =
                GltfLoader::new(allocators.clone(), material_set_layout, builder, &path).unwrap();

            let info = GltfRenderInfo::from_scene(
                allocators.mem.clone(),
                loader.document.default_scene().unwrap(),
                loader.meshes,
            );

            (info, loader.builder.build().unwrap())
        });

        self.gltf_job = Some(job);
    }
    pub fn loading_gltf(&self) -> bool {
        self.gltf_job.is_some()
    }

    pub fn load_skybox(&mut self, path: PathBuf, queue: Arc<Queue>) {
        if self.skybox_job.is_some() {
            return;
        }
        let allocators = self.allocators.clone();
        let renderer = self.renderer.equi_renderer.clone();
        let texture_layout = self.renderer.set_layouts.texture.clone();
        let job = std::thread::spawn(move || {
            let mut builder = AutoCommandBufferBuilder::primary(
                allocators.cmd,
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();

            let equi = load_skybox(allocators.mem.clone(), path, &mut builder);
            let equi_view = ImageView::new_default(equi.clone()).unwrap();
            let equi_set = DescriptorSet::new(
                allocators.set.clone(),
                texture_layout.clone(),
                [WriteDescriptorSet::image_view_sampler(
                    0,
                    equi_view,
                    Sampler::new(
                        allocators.mem.device().clone(),
                        SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
                    )
                    .unwrap(),
                )],
                [],
            )
            .unwrap();

            let cube = create_cubemap_image(allocators.mem.clone(), equi.extent()[1]);
            let views: Vec<_> = (0u32..6u32)
                .into_iter()
                .map(|i| {
                    ImageView::new(
                        cube.clone(),
                        ImageViewCreateInfo {
                            view_type: ImageViewType::Dim2d,
                            format: cube.format(),
                            subresource_range: ImageSubresourceRange {
                                aspects: cube.format().aspects(),
                                mip_levels: 0..1,
                                array_layers: i..i + 1,
                            },
                            ..Default::default()
                        },
                    )
                    .unwrap()
                })
                .collect();

            renderer.render(&mut builder, &equi_set, &views);

            let cube_view = ImageView::new(
                cube.clone(),
                ImageViewCreateInfo {
                    view_type: ImageViewType::Cube,
                    ..ImageViewCreateInfo::from_image(&cube)
                },
            )
            .unwrap();
            let cube_set = DescriptorSet::new(
                allocators.set.clone(),
                texture_layout.clone(),
                [WriteDescriptorSet::image_view_sampler(
                    0,
                    cube_view,
                    Sampler::new(
                        allocators.mem.device().clone(),
                        SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
                    )
                    .unwrap(),
                )],
                [],
            )
            .unwrap();

            let cb = builder.build().unwrap();
            ((equi_set, cube_set), cb)
        });
        self.skybox_job = Some(job);
    }
    pub fn loading_skybox(&self) -> bool {
        self.skybox_job.is_some()
    }

    pub fn update_gltf(&mut self) -> Option<Arc<PrimaryAutoCommandBuffer>> {
        if self.gltf_job.as_ref().is_some_and(|job| job.is_finished()) {
            let (info, cb) = self.gltf_job.take().unwrap().join().unwrap();
            self.renderer.gltf_info = Some(info);
            Some(cb)
        } else {
            None
        }
    }
    pub fn update_skybox(&mut self) -> Option<Arc<PrimaryAutoCommandBuffer>> {
        if self
            .skybox_job
            .as_ref()
            .is_some_and(|job| job.is_finished())
        {
            let ((equi, cube), cb) = self.skybox_job.take().unwrap().join().unwrap();

            self.renderer.equi_set = Some(equi);
            self.renderer.cube_set = Some(cube);

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
                    self.camera.look_at().try_inverse().unwrap(),
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

fn load_skybox<L>(
    allocator: Arc<StandardMemoryAllocator>,
    path: impl AsRef<Path>,
    builder: &mut AutoCommandBufferBuilder<L>,
) -> Arc<Image> {
    // let mut reader = BufReader::new(std::fs::File::open(path).unwrap());
    // let mut image_reader = image::ImageReader::new(&mut reader)
    //     .with_guessed_format()
    //     .unwrap();
    // image_reader.no_limits();
    // let image = image_reader.decode().unwrap().to_rgba32f();

    let image = image::open(path).unwrap().to_rgba32f();
    assert_eq!(
        image.width() / 2,
        image.height(),
        "equirectangular image must be 2:1"
    );

    let stage_buffer = Buffer::new_slice(
        allocator.clone(),
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
        allocator,
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
