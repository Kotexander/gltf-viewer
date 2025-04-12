use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    Validated,
    buffer::{
        AllocateBufferError, Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer,
    },
    memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter},
    pipeline::graphics::vertex_input::Vertex,
};

pub mod cubemap;
pub mod equirectangular;

#[repr(C)]
#[derive(BufferContents, Vertex)]
struct CubemapVertex {
    #[format(R32G32B32_SFLOAT)]
    position: glm::Vec3,
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

pub struct CubeMesh {
    vbuf: Subbuffer<[CubemapVertex]>,
    ibuf: Subbuffer<[u16]>,
    ilen: u32,
}
impl CubeMesh {
    pub fn new(
        allocator: Arc<dyn MemoryAllocator>,
    ) -> Result<Self, Validated<AllocateBufferError>> {
        let vbuf = Buffer::from_iter(
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
            VERTICES,
        )?;
        let ibuf = Buffer::from_iter(
            allocator,
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            INDICES,
        )?;

        Ok(Self {
            vbuf,
            ibuf,
            ilen: INDICES.len() as u32,
        })
    }
}
