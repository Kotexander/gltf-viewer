use std::sync::Arc;

use nalgebra_glm as glm;
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferInfoTyped},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::graphics::vertex_input::Vertex,
};

#[repr(C)]
#[derive(BufferContents, Vertex)]
pub struct CubemapVertex {
    #[format(R32G32B32_SFLOAT)]
    pub position: glm::Vec3,
}

#[rustfmt::skip]
const VERTICES: [CubemapVertex; 8] = [
    CubemapVertex { position: glm::Vec3::new(-0.5, -0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5, -0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5,  0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new(-0.5,  0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new(-0.5, -0.5,  0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5, -0.5,  0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5,  0.5,  0.5) },
    CubemapVertex { position: glm::Vec3::new(-0.5,  0.5,  0.5) },
];

#[rustfmt::skip]
const INDICES: [u16; 36] = [
    // back face (z+)
    6, 5, 4,
    4, 7, 6,

    // front face (z-)
    2, 3, 0,
    0, 1, 2,

    // left face (x-)
    7, 4, 0,
    0, 3, 7,

    // right face (x+)
    6, 2, 1,
    1, 5, 6,

    // top face (y+)
    6, 7, 3,
    3, 2, 6,

    // bottom face (y-)
    5, 1, 0,
    0, 4, 5,
];

#[derive(Clone)]
pub struct CubeMesh {
    vbuf: Subbuffer<[CubemapVertex]>,
    ibuf: Subbuffer<[u16]>,
    ilen: u32,
}
impl CubeMesh {
    pub fn new<L>(
        allocator: Arc<StandardMemoryAllocator>,
        builder: &mut AutoCommandBufferBuilder<L>,
    ) -> Self {
        let vbuf_stage = Buffer::from_iter(
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
            VERTICES,
        )
        .unwrap();
        let ibuf_stage = Buffer::from_iter(
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
            INDICES,
        )
        .unwrap();

        let vbuf = Buffer::new_slice(
            allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST | BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
            VERTICES.len() as DeviceSize,
        )
        .unwrap();
        let ibuf = Buffer::new_slice(
            allocator,
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST | BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
            INDICES.len() as DeviceSize,
        )
        .unwrap();

        builder
            .copy_buffer(CopyBufferInfoTyped::buffers(vbuf_stage, vbuf.clone()))
            .unwrap();
        builder
            .copy_buffer(CopyBufferInfoTyped::buffers(ibuf_stage, ibuf.clone()))
            .unwrap();

        Self {
            vbuf,
            ibuf,
            ilen: INDICES.len() as u32,
        }
    }
    pub fn render<L>(&self, builder: &mut AutoCommandBufferBuilder<L>) {
        builder
            .bind_vertex_buffers(0, self.vbuf.clone())
            .unwrap()
            .bind_index_buffer(self.ibuf.clone())
            .unwrap();
        unsafe { builder.draw_indexed(self.ilen, 1, 0, 0, 0) }.unwrap();
    }
}
