use crate::{
    Allocators,
    camera::OrbitCamera,
    gltf::{GltfRenderInfo, loader::mesh::PrimitiveVertex},
};
use std::sync::Arc;
use vulkano::{
    acceleration_structure::{
        AccelerationStructure, AccelerationStructureBuildGeometryInfo,
        AccelerationStructureBuildRangeInfo, AccelerationStructureBuildType,
        AccelerationStructureCreateInfo, AccelerationStructureGeometries,
        AccelerationStructureGeometryInstancesData, AccelerationStructureGeometryInstancesDataType,
        AccelerationStructureGeometryTrianglesData, AccelerationStructureInstance,
        AccelerationStructureType, BuildAccelerationStructureFlags, BuildAccelerationStructureMode,
    },
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
        allocator::CommandBufferAllocator,
    },
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    device::{Device, Queue},
    format::Format,
    image::{Image, ImageCreateInfo, ImageUsage, view::ImageView},
    memory::{
        DeviceAlignment,
        allocator::{AllocationCreateInfo, DeviceLayout, MemoryAllocator, MemoryTypeFilter},
    },
    pipeline::{
        Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
        layout::PipelineDescriptorSetLayoutCreateInfo,
        ray_tracing::{
            RayTracingPipeline, RayTracingPipelineCreateInfo, RayTracingShaderGroupCreateInfo,
            ShaderBindingTable,
        },
    },
    sync::GpuFuture,
};

#[derive(Clone)]
pub struct Raytracer {
    pipeline: Arc<RayTracingPipeline>,
    shader_binding_table: ShaderBindingTable,
    tlas: Option<Arc<AccelerationStructure>>,
    allocators: Allocators,
    pub view: Arc<ImageView>,

    _blas: Vec<Arc<AccelerationStructure>>,
}
impl Raytracer {
    pub fn new(device: &Arc<Device>, allocators: Allocators) -> Self {
        let raygen = raygen::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let closest_hit = closest_hit::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let miss = miss::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();

        let stages = [
            PipelineShaderStageCreateInfo::new(raygen),
            PipelineShaderStageCreateInfo::new(miss),
            PipelineShaderStageCreateInfo::new(closest_hit),
        ];

        let groups = [
            RayTracingShaderGroupCreateInfo::General { general_shader: 0 },
            RayTracingShaderGroupCreateInfo::General { general_shader: 1 },
            RayTracingShaderGroupCreateInfo::TrianglesHit {
                closest_hit_shader: Some(2),
                any_hit_shader: None,
            },
        ];

        let layout = PipelineLayout::new(
            device.clone(),
            PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                .into_pipeline_layout_create_info(device.clone())
                .unwrap(),
        )
        .unwrap();

        let pipeline = RayTracingPipeline::new(
            device.clone(),
            None,
            RayTracingPipelineCreateInfo {
                stages: stages.into_iter().collect(),
                groups: groups.into_iter().collect(),
                max_pipeline_ray_recursion_depth: 1,
                ..RayTracingPipelineCreateInfo::layout(layout)
            },
        )
        .unwrap();

        let shader_binding_table =
            ShaderBindingTable::new(allocators.mem.clone(), &pipeline).unwrap();

        let view = Self::new_view(allocators.mem.clone(), [1, 1]);

        Self {
            pipeline,
            shader_binding_table,
            tlas: None,
            allocators,
            view,
            _blas: vec![],
        }
    }
    pub fn build(&mut self, queue: Arc<Queue>, info: &GltfRenderInfo) {
        let (blas, other): (Vec<_>, Vec<Vec<_>>) = info
            .meshes
            .iter()
            .flat_map(|instances| {
                instances.primatives().iter().map(|primitive| unsafe {
                    let blas = build_acceleration_structure_triangles(
                        primitive.vbuf().clone(),
                        primitive.ibuf().clone(),
                        self.allocators.mem.clone(),
                        self.allocators.cmd.clone(),
                        queue.device().clone(),
                        queue.clone(),
                    );
                    (
                        blas.clone(),
                        instances
                            .instances()
                            .iter()
                            .map(move |transform| AccelerationStructureInstance {
                                acceleration_structure_reference: blas.device_address().into(),
                                transform: transform.remove_row(3).transpose().into(),
                                ..Default::default()
                            })
                            .collect(),
                    )
                })
            })
            .collect();

        let tlas = unsafe {
            build_top_level_acceleration_structure(
                other.concat(),
                self.allocators.mem.clone(),
                self.allocators.cmd.clone(),
                queue.device().clone(),
                queue.clone(),
            )
        };

        self.tlas = Some(tlas);
        self._blas = blas;
    }
    pub fn render(&self, orbit_camera: OrbitCamera, aspect: f32, queue: Arc<Queue>) {
        if let Some(tlas) = self.tlas.clone() {
            let mut builder = AutoCommandBufferBuilder::primary(
                self.allocators.cmd.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();

            let camera = [
                orbit_camera.look_at().try_inverse().unwrap(),
                orbit_camera.perspective(aspect),
            ];
            let camera = Buffer::from_data(
                self.allocators.mem.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::UNIFORM_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                camera,
            )
            .unwrap();
            let scene_set = DescriptorSet::new(
                self.allocators.set.clone(),
                self.pipeline.layout().set_layouts()[0].clone(),
                [
                    WriteDescriptorSet::acceleration_structure(0, tlas),
                    WriteDescriptorSet::buffer(1, camera),
                ],
                [],
            )
            .unwrap();
            let image_set = DescriptorSet::new(
                self.allocators.set.clone(),
                self.pipeline.layout().set_layouts()[1].clone(),
                [WriteDescriptorSet::image_view(0, self.view.clone())],
                [],
            )
            .unwrap();
            builder
                .bind_descriptor_sets(
                    PipelineBindPoint::RayTracing,
                    self.pipeline.layout().clone(),
                    0,
                    vec![scene_set, image_set],
                )
                .unwrap()
                .bind_pipeline_ray_tracing(self.pipeline.clone())
                .unwrap();

            unsafe {
                builder.trace_rays(
                    self.shader_binding_table.addresses().clone(),
                    self.view.image().extent(),
                )
            }
            .unwrap();

            let cb = builder.build().unwrap();
            cb.execute(queue)
                .unwrap()
                .then_signal_fence_and_flush()
                .unwrap()
                .wait(None)
                .unwrap();
        }
        // builder.bind
    }
    pub fn resize(&mut self, size: [u32; 2]) {
        if self.view.image().extent()[..2] != size[..] {
            self.view = Self::new_view(self.allocators.mem.clone(), size);
        }
    }
    fn new_view(mem_allocator: Arc<dyn MemoryAllocator>, size: [u32; 2]) -> Arc<ImageView> {
        let image = Image::new(
            mem_allocator,
            ImageCreateInfo {
                usage: ImageUsage::STORAGE | ImageUsage::SAMPLED,
                format: Format::R32G32B32A32_SFLOAT,
                extent: [size[0], size[1], 1],
                ..Default::default()
            },
            Default::default(),
        )
        .unwrap();
        ImageView::new_default(image).unwrap()
    }
}

/// A helper function to build a acceleration structure and wait for its completion.
///
/// # Safety
///
/// - If you are referencing a bottom-level acceleration structure in a top-level acceleration
///   structure, you must ensure that the bottom-level acceleration structure is kept alive.
unsafe fn build_acceleration_structure_common(
    geometries: AccelerationStructureGeometries,
    primitive_count: u32,
    ty: AccelerationStructureType,
    memory_allocator: Arc<dyn MemoryAllocator>,
    command_buffer_allocator: Arc<dyn CommandBufferAllocator>,
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> Arc<AccelerationStructure> {
    let mut as_build_geometry_info = AccelerationStructureBuildGeometryInfo {
        mode: BuildAccelerationStructureMode::Build,
        flags: BuildAccelerationStructureFlags::PREFER_FAST_TRACE,
        ..AccelerationStructureBuildGeometryInfo::new(geometries)
    };

    let as_build_sizes_info = device
        .acceleration_structure_build_sizes(
            AccelerationStructureBuildType::Device,
            &as_build_geometry_info,
            &[primitive_count],
        )
        .unwrap();

    // We create a new scratch buffer for each acceleration structure for simplicity. You may want
    // to reuse scratch buffers if you need to build many acceleration structures.
    // let scratch_buffer = Buffer::new_slice::<u8>(
    //     memory_allocator.clone(),
    //     BufferCreateInfo {
    //         usage: BufferUsage::SHADER_DEVICE_ADDRESS | BufferUsage::STORAGE_BUFFER,
    //         ..Default::default()
    //     },
    //     AllocationCreateInfo::default(),
    //     as_build_sizes_info.build_scratch_size,
    // )
    // .unwrap()
    // .align_to(
    //     DeviceLayout::new(
    //         as_build_sizes_info.build_scratch_size.try_into().unwrap(),
    //         DeviceAlignment::new(
    //             device
    //                 .physical_device()
    //                 .properties()
    //                 .min_acceleration_structure_scratch_offset_alignment
    //                 .unwrap()
    //                 .try_into()
    //                 .unwrap(),
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap(),
    // );

    let scratch_buffer = Buffer::new(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::SHADER_DEVICE_ADDRESS | BufferUsage::STORAGE_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
        DeviceLayout::new(
            as_build_sizes_info.build_scratch_size.try_into().unwrap(),
            DeviceAlignment::new(
                device
                    .physical_device()
                    .properties()
                    .min_acceleration_structure_scratch_offset_alignment
                    .unwrap()
                    .into(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let as_create_info = AccelerationStructureCreateInfo {
        ty,
        ..AccelerationStructureCreateInfo::new(
            Buffer::new_slice::<u8>(
                memory_allocator,
                BufferCreateInfo {
                    usage: BufferUsage::ACCELERATION_STRUCTURE_STORAGE
                        | BufferUsage::SHADER_DEVICE_ADDRESS,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
                as_build_sizes_info.acceleration_structure_size,
            )
            .unwrap(),
        )
    };

    let acceleration = unsafe { AccelerationStructure::new(device, as_create_info) }.unwrap();

    as_build_geometry_info.dst_acceleration_structure = Some(acceleration.clone());
    as_build_geometry_info.scratch_data = Some(scratch_buffer.into());

    let as_build_range_info = AccelerationStructureBuildRangeInfo {
        primitive_count,
        ..Default::default()
    };

    // For simplicity, we build a single command buffer that builds the acceleration structure,
    // then waits for its execution to complete.
    let mut builder = AutoCommandBufferBuilder::primary(
        command_buffer_allocator,
        queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    unsafe {
        builder
            .build_acceleration_structure(
                as_build_geometry_info,
                std::iter::once(as_build_range_info).collect(),
            )
            .unwrap()
    };

    builder
        .build()
        .unwrap()
        .execute(queue)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap()
        .wait(None)
        .unwrap();

    acceleration
}

unsafe fn build_acceleration_structure_triangles(
    vertex_buffer: Subbuffer<[PrimitiveVertex]>,
    index_buffer: Subbuffer<[u32]>,
    memory_allocator: Arc<dyn MemoryAllocator>,
    command_buffer_allocator: Arc<dyn CommandBufferAllocator>,
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> Arc<AccelerationStructure> {
    let primitive_count = (index_buffer.len() / 3) as u32;
    let as_geometry_triangles_data = AccelerationStructureGeometryTrianglesData {
        max_vertex: vertex_buffer.len() as _,
        vertex_data: Some(vertex_buffer.into_bytes()),
        vertex_stride: size_of::<PrimitiveVertex>() as _,
        index_data: Some(index_buffer.into()),
        ..AccelerationStructureGeometryTrianglesData::new(Format::R32G32B32_SFLOAT)
    };

    let geometries = AccelerationStructureGeometries::Triangles(vec![as_geometry_triangles_data]);

    unsafe {
        build_acceleration_structure_common(
            geometries,
            primitive_count,
            AccelerationStructureType::BottomLevel,
            memory_allocator,
            command_buffer_allocator,
            device,
            queue,
        )
    }
}

unsafe fn build_top_level_acceleration_structure(
    as_instances: Vec<AccelerationStructureInstance>,
    allocator: Arc<dyn MemoryAllocator>,
    command_buffer_allocator: Arc<dyn CommandBufferAllocator>,
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> Arc<AccelerationStructure> {
    let primitive_count = as_instances.len() as u32;

    let instance_buffer = Buffer::from_iter(
        allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::SHADER_DEVICE_ADDRESS
                | BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        as_instances,
    )
    .unwrap();

    let as_geometry_instances_data = AccelerationStructureGeometryInstancesData::new(
        AccelerationStructureGeometryInstancesDataType::Values(Some(instance_buffer)),
    );

    let geometries = AccelerationStructureGeometries::Instances(as_geometry_instances_data);

    unsafe {
        build_acceleration_structure_common(
            geometries,
            primitive_count,
            AccelerationStructureType::TopLevel,
            allocator,
            command_buffer_allocator,
            device,
            queue,
        )
    }
}

mod raygen {
    vulkano_shaders::shader! {
        ty: "raygen",
        vulkan_version: "1.2",
        src: r#"
#version 460
#extension GL_EXT_ray_tracing : require

layout(location = 0) rayPayloadEXT vec3 hit_value;

layout(set = 0, binding = 0) uniform accelerationStructureEXT top_level_as;
layout(set = 0, binding = 1) uniform Camera {
    mat4 view_inverse; // Camera inverse view matrix
    mat4 proj_inverse; // Camera inverse projection matrix
} camera;
layout(set = 1, binding = 0, rgba32f) uniform image2D image;

void main() {
    const vec2 pixel_center = vec2(gl_LaunchIDEXT.xy) + vec2(0.5);
    const vec2 in_uv = pixel_center / vec2(gl_LaunchSizeEXT.xy);
    vec2 d = in_uv * 2.0 - 1.0;

    vec4 origin = camera.view_inverse * vec4(0, 0, 0, 1);
    vec4 target = camera.proj_inverse * vec4(d.x, d.y, 1, 1);
    vec4 direction = camera.view_inverse * vec4(normalize(target.xyz), 0);

    uint ray_flags = gl_RayFlagsOpaqueEXT;
    float t_min = 0.001;
    float t_max = 10000.0;

    traceRayEXT(
        top_level_as,  // acceleration structure
        ray_flags,     // rayFlags
        0xFF,          // cullMask
        0,             // sbtRecordOffset
        0,             // sbtRecordStride
        0,             // missIndex
        origin.xyz,    // ray origin
        t_min,         // ray min range
        direction.xyz, // ray direction
        t_max,         // ray max range
        0);            // payload (location = 0)

    imageStore(image, ivec2(gl_LaunchIDEXT.xy), vec4(hit_value, 1.0));
}
        "#
    }
}

mod closest_hit {
    vulkano_shaders::shader! {
        ty: "closesthit",
        vulkan_version: "1.2",
        src: r#"
#version 460
#extension GL_EXT_ray_tracing : require

layout(location = 0) rayPayloadInEXT vec3 hit_value;
hitAttributeEXT vec2 attribs;

void main() {
    vec3 barycentrics = vec3(1.0 - attribs.x - attribs.y, attribs.x, attribs.y);
    hit_value = barycentrics;
}
        "#,
    }
}

mod miss {
    vulkano_shaders::shader! {
        ty: "miss",
        vulkan_version: "1.2",
        src: r#"
#version 460
#extension GL_EXT_ray_tracing : require

layout(location = 0) rayPayloadInEXT vec3 hit_value;

void main() {
    hit_value = vec3(0.0, 0.0, 0.2);
}
        "#,
    }
}
