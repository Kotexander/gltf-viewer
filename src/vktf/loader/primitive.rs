use super::Loader;
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferInfo},
    memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter},
    pipeline::graphics::vertex_input::Vertex,
};

#[repr(C)]
#[derive(Debug, Default, BufferContents, Vertex)]
pub struct PrimitiveVertex {
    #[format(R32G32B32_SFLOAT)]
    pub position: glm::Vec3,
    #[format(R32G32B32_SFLOAT)]
    pub normal: glm::Vec3,
    #[format(R32G32B32A32_SFLOAT)]
    pub tangent: glm::Vec4,
    #[format(R32G32_SFLOAT)]
    pub uv_0: glm::Vec2,
    #[format(R32G32_SFLOAT)]
    pub uv_1: glm::Vec2,
}

struct PrimitiveVertexDataBuilder<'a, 's, F: Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>> {
    vertices: Vec<PrimitiveVertex>,
    indices: Vec<u32>,
    nm_set: i32,
    reader: gltf::mesh::Reader<'a, 's, F>,
}
impl<'a, 's, F: Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>>
    PrimitiveVertexDataBuilder<'a, 's, F>
{
    fn new(reader: gltf::mesh::Reader<'a, 's, F>, nm_set: i32) -> Option<Self> {
        let vertices: Vec<_> = reader
            .read_positions()?
            .map(|pos| PrimitiveVertex {
                position: pos.into(),
                ..Default::default()
            })
            .collect();

        let indices: Vec<_> = reader
            .read_indices()
            .map(|i| i.into_u32().collect())
            .unwrap_or_else(|| (0..vertices.len() as u32).collect());

        Some(Self {
            vertices,
            indices,
            reader,
            nm_set,
        })
    }
    fn set_normals(&mut self) {
        match self.reader.read_normals() {
            Some(normals) => {
                for (i, normal) in normals.enumerate() {
                    self.vertices[i].normal = normal.into();
                }
            }
            None => {
                unimplemented!("calculate flat normals and ignore provided tangents")
            }
        }
    }
    fn set_textures_sets(&mut self) {
        for (i, tex) in self
            .reader
            .read_tex_coords(0)
            .into_iter()
            .flat_map(|iter| iter.into_f32())
            .enumerate()
        {
            self.vertices[i].uv_0 = tex.into();
        }
        for (i, tex) in self
            .reader
            .read_tex_coords(1)
            .into_iter()
            .flat_map(|iter| iter.into_f32())
            .enumerate()
        {
            self.vertices[i].uv_1 = tex.into();
        }
    }
    fn set_tangents(&mut self) {
        match self.reader.read_tangents() {
            // use provided tangents
            Some(tangents) => {
                for (i, tangent) in tangents.enumerate() {
                    self.vertices[i].tangent = tangent.into();
                }
            }
            None => {
                if self.nm_set >= 0 {
                    assert!(
                        mikktspace::generate_tangents(self),
                        "generating tangents failed"
                    );
                }
            }
        }
    }
}
impl<'a, 's, F: Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>> mikktspace::Geometry
    for PrimitiveVertexDataBuilder<'a, 's, F>
{
    fn num_faces(&self) -> usize {
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.vertices[self.indices[face * 3 + vert] as usize]
            .position
            .into()
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.vertices[self.indices[face * 3 + vert] as usize]
            .normal
            .into()
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        match self.nm_set {
            0 => self.vertices[self.indices[face * 3 + vert] as usize]
                .uv_0
                .into(),
            1 => self.vertices[self.indices[face * 3 + vert] as usize]
                .uv_1
                .into(),
            set => {
                panic!("invalid texture set: {set}");
            }
        }
    }

    fn set_tangent(
        &mut self,
        tangent: [f32; 3],
        _bi_tangent: [f32; 3],
        _f_mag_s: f32,
        _f_mag_t: f32,
        bi_tangent_preserves_orientation: bool,
        face: usize,
        vert: usize,
    ) {
        let sign = if bi_tangent_preserves_orientation {
            -1.0
        } else {
            1.0
        };
        let tangent = [tangent[0], tangent[1], tangent[2], sign];
        self.vertices[self.indices[face * 3 + vert] as usize].tangent = tangent.into();
    }
}

#[derive(Clone, Debug)]
pub struct Primitive {
    vbuf: Subbuffer<[PrimitiveVertex]>,
    ibuf: Subbuffer<[u32]>,
    ilen: u32,
}
impl Primitive {
    pub(super) fn from_loader<L>(
        primitive: &gltf::Primitive,
        buffers: &[gltf::buffer::Data],
        loader: &mut Loader<L>,
    ) -> Option<Self> {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));

        let mut vertex_data = PrimitiveVertexDataBuilder::new(
            reader,
            primitive
                .material()
                .normal_texture()
                .map(|nm| nm.tex_coord() as i32)
                .unwrap_or(-1),
        )?;
        vertex_data.set_normals();
        vertex_data.set_textures_sets();
        vertex_data.set_tangents();

        let vbuf = stage(
            loader.builder,
            loader.allocator.clone(),
            BufferUsage::VERTEX_BUFFER,
            vertex_data.vertices,
        );
        let ibuf = stage(
            loader.builder,
            loader.allocator.clone(),
            BufferUsage::INDEX_BUFFER,
            vertex_data.indices,
        );

        Some(Self {
            ilen: ibuf.len() as u32,
            vbuf,
            ibuf,
        })
    }
    pub fn render<L>(self, instances: u32, builder: &mut AutoCommandBufferBuilder<L>) {
        builder
            .bind_vertex_buffers(0, self.vbuf)
            .unwrap()
            .bind_index_buffer(self.ibuf)
            .unwrap();
        unsafe { builder.draw_indexed(self.ilen, instances, 0, 0, 0) }.unwrap();
    }
}

fn stage<L, T: BufferContents>(
    builder: &mut AutoCommandBufferBuilder<L>,
    allocator: Arc<dyn MemoryAllocator>,
    usage: BufferUsage,
    data: Vec<T>,
) -> Subbuffer<[T]> {
    let stage = Buffer::from_iter(
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
        data,
    )
    .unwrap();
    let buf = Buffer::new_slice(
        allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_DST | usage,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
        stage.len(),
    )
    .unwrap();

    builder
        .copy_buffer(CopyBufferInfo::buffers(stage, buf.clone()))
        .unwrap();

    buf
}
