use camera::OrbitCamera;
use egui_file::FileDialog;
use egui_winit_vulkano::CallbackFn;
use nalgebra_glm as glm;
use set_layouts::SetLayouts;
use skybox::Skybox;
use std::{env::current_dir, path::PathBuf, sync::Arc};
use viewer::Viewer;
use vulkano::{
    buffer::{
        Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer,
        allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo},
    },
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, PrimaryCommandBufferAbstract,
        allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
        layout::DescriptorSetLayout,
    },
    device::{DeviceOwned, Queue},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{Pipeline, PipelineBindPoint},
    render_pass::Subpass,
    sync::GpuFuture,
};

mod camera;
mod cubemap;
mod gltf;

mod set_layouts;
mod skybox;
mod viewer;

#[derive(Clone)]
pub struct Allocators {
    pub cmd: Arc<StandardCommandBufferAllocator>,
    pub mem: Arc<StandardMemoryAllocator>,
    pub set: Arc<StandardDescriptorSetAllocator>,
}

#[repr(C)]
#[derive(BufferContents)]
pub struct CameraUniform {
    view: glm::Mat4,
    proj: glm::Mat4,
    view_inv: glm::Mat4,
}
impl CameraUniform {
    pub fn new(camera: &OrbitCamera, aspect: f32) -> Self {
        Self {
            view: camera.look_at(),
            proj: camera.perspective(aspect),
            view_inv: camera.look_at().try_inverse().unwrap(),
        }
    }
}

#[derive(Default)]
pub enum FilePicker {
    Skybox(FileDialog),
    Gltf(FileDialog),
    #[default]
    None,
}
impl FilePicker {
    pub fn skybox(&mut self) {
        let extensions = ["hdr", "exr", "jpg"];
        let mut file_picker = FileDialog::open_file(self.initial_path())
            .show_rename(false)
            .show_new_folder(false)
            .multi_select(false)
            .show_files_filter(Box::new(move |path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| extensions.contains(&ext))
            }));
        file_picker.open();
        *self = Self::Skybox(file_picker)
    }
    pub fn gltf(&mut self) {
        let extensions = ["glb", "gltf"];
        let mut file_picker = FileDialog::open_file(self.initial_path())
            .show_rename(false)
            .show_new_folder(false)
            .multi_select(false)
            .show_files_filter(Box::new(move |path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| extensions.contains(&ext))
            }));
        file_picker.open();
        *self = Self::Gltf(file_picker)
    }
    fn initial_path(&self) -> Option<PathBuf> {
        match self {
            FilePicker::Skybox(file_dialog) => Some(file_dialog.directory().to_owned()),
            FilePicker::Gltf(file_dialog) => Some(file_dialog.directory().to_owned()),
            FilePicker::None => current_dir().ok(),
        }
    }
}

struct CameraResource {
    buffer: Subbuffer<CameraUniform>,
    set: Arc<DescriptorSet>,
}
impl CameraResource {
    pub fn new(
        mem_allocator: Arc<StandardMemoryAllocator>,
        set_allocator: Arc<StandardDescriptorSetAllocator>,
        layout: Arc<DescriptorSetLayout>,
    ) -> Self {
        let buffer = Buffer::new_sized(
            mem_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .unwrap();
        let set = DescriptorSet::new(
            set_allocator.clone(),
            layout,
            [WriteDescriptorSet::buffer(0, buffer.clone())],
            [],
        )
        .unwrap();

        Self { buffer, set }
    }
}

pub struct State {
    queue: Arc<Queue>,
    subbuffer_allocator: SubbufferAllocator,

    camera: OrbitCamera,
    cameras: Vec<CameraResource>,

    aspect: f32,

    skybox: Skybox,
    viewer: Viewer,

    file_picker: FilePicker,
}
impl State {
    pub fn new(
        allocators: &Allocators,
        queue: Arc<Queue>,
        num_frames: usize,
        subpass: Subpass,
    ) -> Self {
        let camera = OrbitCamera::default();

        let subbuffer_allocator = SubbufferAllocator::new(
            allocators.mem.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::TRANSFER_SRC,
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        let set_layouts = SetLayouts::new(queue.device().clone());

        let cameras = (0..num_frames)
            .map(|_| {
                CameraResource::new(
                    allocators.mem.clone(),
                    allocators.set.clone(),
                    set_layouts.camera.clone(),
                )
            })
            .collect();

        let mut builder = AutoCommandBufferBuilder::primary(
            allocators.cmd.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        let skybox = Skybox::new(allocators, &mut builder, &set_layouts, subpass.clone());

        builder
            .build()
            .unwrap()
            .execute(queue.clone())
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None)
            .unwrap();

        let viewer = Viewer::new(allocators, &set_layouts, subpass);

        Self {
            camera,
            subbuffer_allocator,
            aspect: 1.0,
            skybox,
            file_picker: FilePicker::default(),
            queue,
            cameras,
            viewer,
        }
    }
    pub fn update<L>(&mut self, builder: &mut AutoCommandBufferBuilder<L>, index: usize) {
        if let Some((conv, filt)) = self.skybox.update() {
            self.viewer.new_env(conv, filt);
        }
        self.viewer.update();

        if self.aspect.is_normal() {
            let data = CameraUniform::new(&self.camera, self.aspect);
            let buffer = self.subbuffer_allocator.allocate_sized().unwrap();
            *buffer.write().unwrap() = data;
            builder
                .copy_buffer(CopyBufferInfo::buffers(
                    buffer,
                    self.cameras[index].buffer.clone(),
                ))
                .unwrap();
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, index: usize) {
        match &mut self.file_picker {
            FilePicker::Skybox(file_dialog) => {
                if file_dialog.show(ctx).selected() {
                    let file = file_dialog.path().unwrap();
                    self.skybox.load(file.into(), self.queue.clone());
                }
            }
            FilePicker::Gltf(file_dialog) => {
                if file_dialog.show(ctx).selected() {
                    let file = file_dialog.path().unwrap();
                    self.viewer.load(file.into(), self.queue.clone());
                }
            }
            FilePicker::None => {}
        }

        egui::SidePanel::right("state_right_panel").show(ctx, |ui| {
            ui.heading("Settings");

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.skybox.loading(), egui::Button::new("Open Skybox"))
                    .clicked()
                {
                    self.file_picker.skybox();
                }
                if self.skybox.loading() {
                    ui.spinner();
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.viewer.loading(), egui::Button::new("Open glTF"))
                    .clicked()
                {
                    self.file_picker.gltf();
                }
                if self.viewer.loading() {
                    ui.spinner();
                }
            });

            ui.separator();

            ui.collapsing("Camera", |ui| {
                self.camera.ui(ui);
            });

            ui.separator();
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::all());
                self.aspect = rect.aspect_ratio();

                let modifiers = response.ctx.input(|i| i.modifiers);

                // pan
                if modifiers.shift {
                    let cam = self.camera.look_at().try_inverse().unwrap();
                    let right = cam.transform_vector(&glm::Vec3::x());
                    let up = cam.transform_vector(&glm::Vec3::y());
                    let delta = response.drag_motion() * 0.002 * self.camera.zoom;
                    self.camera.target -= right * delta.x;
                    self.camera.target -= up * delta.y;
                }
                // rotate
                else {
                    let drag_delta = response.drag_motion() * 0.005;
                    self.camera.yaw -= drag_delta.x;
                    self.camera.pitch += drag_delta.y;
                    self.camera.wrap();
                }

                let smooth_scroll = response.ctx.input(|i| i.smooth_scroll_delta);
                self.camera.zoom += self.camera.zoom * -smooth_scroll.y * 0.003;
                self.camera.clamp();

                let skybox = self.skybox.renderer.clone();
                let viewer = self.viewer.renderer.clone();
                let camera_set = self.cameras[index].set.clone();
                let callback = egui::PaintCallback {
                    rect,
                    callback: Arc::new(CallbackFn::new(move |_info, context| {
                        context
                            .builder
                            .bind_descriptor_sets(
                                PipelineBindPoint::Graphics,
                                viewer.pipeline.pipeline.layout().clone(),
                                0,
                                camera_set.clone(),
                            )
                            .unwrap();
                        viewer.render(context.builder);
                        skybox.render(context.builder);
                    })),
                };
                ui.painter().add(callback);
            });
    }
}
