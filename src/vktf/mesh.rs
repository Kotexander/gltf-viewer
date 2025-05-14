use super::{loader::Primitive, material::Materials};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::AutoCommandBufferBuilder,
    memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter},
    pipeline::{PipelineLayout, graphics::vertex_input::Vertex},
};

#[repr(C)]
#[derive(BufferContents, Vertex, Debug)]
pub struct Instance {
    #[format(R32G32B32A32_SFLOAT)]
    pub model_x: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub model_y: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub model_z: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub model_w: [f32; 4],
}
impl From<glm::Mat4> for Instance {
    fn from(value: glm::Mat4) -> Self {
        Self {
            model_x: value.data.0[0],
            model_y: value.data.0[1],
            model_z: value.data.0[2],
            model_w: value.data.0[3],
        }
    }
}

#[derive(Clone)]
pub struct MaterialPrimitive {
    material: Option<usize>,
    primitive: Primitive,
}

#[derive(Clone)]
pub struct Mesh {
    primitives: Vec<MaterialPrimitive>,
    instances: Subbuffer<[Instance]>,
    len: u32,
}
impl Mesh {
    pub fn new<'a>(
        allocator: Arc<dyn MemoryAllocator>,
        primitives: impl Iterator<Item = (gltf::Primitive<'a>, Primitive)>,
        instances: Vec<glm::Mat4>,
    ) -> Self {
        let instance_buffer = Buffer::from_iter(
            allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            instances.iter().copied().map(Instance::from),
        )
        .unwrap();
        let primitives = primitives
            .filter_map(|(gltf, primitive)| {
                if gltf.mode() != gltf::mesh::Mode::Triangles {
                    None
                } else {
                    Some(MaterialPrimitive {
                        material: gltf.material().index(),
                        primitive,
                    })
                }
            })
            .collect();
        Mesh {
            primitives,
            len: instance_buffer.len() as u32,
            instances: instance_buffer,
        }
    }

    pub fn render<L>(
        self,
        builder: &mut AutoCommandBufferBuilder<L>,
        materials: &Materials,
        layout: &Arc<PipelineLayout>,
    ) {
        builder.bind_vertex_buffers(1, self.instances).unwrap();
        for primitive in self.primitives {
            materials
                .get(primitive.material)
                .unwrap()
                .clone()
                .set(builder, layout.clone());
            primitive.primitive.render(self.len, builder);
        }
    }
}
