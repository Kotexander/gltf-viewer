use super::{Import, Loader};
use nalgebra_glm as glm;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferInfo},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    pipeline::graphics::vertex_input::Vertex,
};

#[repr(C)]
#[derive(BufferContents, Vertex, Debug)]
pub struct PrimitiveVertex {
    #[format(R32G32B32_SFLOAT)]
    pub position: glm::Vec3,
    #[format(R32G32B32_SFLOAT)]
    pub normal: glm::Vec3,
    #[format(R32G32_SFLOAT)]
    pub bc_tex: glm::Vec2,
    #[format(R32G32_SFLOAT)]
    pub rm_tex: glm::Vec2,
    #[format(R32G32_SFLOAT)]
    pub ao_tex: glm::Vec2,
}

#[derive(Clone, Debug)]
pub struct Primitive {
    vbuf: Subbuffer<[PrimitiveVertex]>,
    ibuf: Subbuffer<[u32]>,
    ilen: u32,
}
impl Primitive {
    pub fn new(primitive: gltf::Primitive, import: &Import, loader: &mut Loader) -> Self {
        let reader =
            primitive.reader(|buffer| import.buffers.get(buffer.index()).map(|d| d.0.as_slice()));

        let vertices = reader
            .read_positions()
            .unwrap()
            .zip(reader.read_normals().unwrap())
            .map(|(pos, norm)| PrimitiveVertex {
                position: pos.into(),
                normal: norm.into(),
                bc_tex: glm::Vec2::zeros(),
                rm_tex: glm::Vec2::zeros(),
                ao_tex: glm::Vec2::zeros(),
            });
        let indices = reader.read_indices().unwrap().into_u32();

        let vbuf_stage = Buffer::from_iter(
            loader.allocators.mem.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vertices,
        )
        .unwrap();
        let ibuf_stage = Buffer::from_iter(
            loader.allocators.mem.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            indices,
        )
        .unwrap();

        let vbuf = Buffer::new_slice(
            loader.allocators.mem.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST | BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
            vbuf_stage.len(),
        )
        .unwrap();
        let ibuf = Buffer::new_slice(
            loader.allocators.mem.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST | BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
            ibuf_stage.len(),
        )
        .unwrap();

        let builder = loader.builder();
        builder
            .copy_buffer(CopyBufferInfo::buffers(vbuf_stage, vbuf.clone()))
            .unwrap();
        builder
            .copy_buffer(CopyBufferInfo::buffers(ibuf_stage, ibuf.clone()))
            .unwrap();

        Self {
            ilen: ibuf.len() as u32,
            vbuf,
            ibuf,
        }
    }
    pub fn render<L>(self, builder: &mut AutoCommandBufferBuilder<L>) {
        builder.bind_vertex_buffers(0, self.vbuf).unwrap();
        builder.bind_index_buffer(self.ibuf).unwrap();
        unsafe { builder.draw_indexed(self.ilen, 1, 0, 0, 0) }.unwrap();
    }
}

#[derive(Debug)]
pub struct Mesh {
    pub index: usize,
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
}
impl Mesh {
    pub fn from_loader(mesh: gltf::Mesh, import: &Import, loader: &mut Loader) -> Self {
        let primitives = mesh
            .primitives()
            .filter_map(|primitive| {
                let is_triangle = primitive.mode() == gltf::mesh::Mode::Triangles;
                if !is_triangle {
                    log::warn!("triangle primitives allowed only for now. skipping.");
                    None
                } else {
                    Some(Primitive::new(primitive, import, loader))
                }
            })
            .collect();

        Self {
            index: mesh.index(),
            name: mesh.name().map(String::from),
            primitives,
        }
    }
}
