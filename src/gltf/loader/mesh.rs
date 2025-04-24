use super::Loader;
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferInfo},
    descriptor_set::DescriptorSet,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    pipeline::{PipelineBindPoint, PipelineLayout, graphics::vertex_input::Vertex},
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

struct VertexData<'a, 's, F: Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>> {
    vertices: Vec<PrimitiveVertex>,
    indices: Vec<u32>,
    reader: gltf::mesh::Reader<'a, 's, F>,
}
impl<'a, 's, F: Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>> VertexData<'a, 's, F> {
    fn new(reader: gltf::mesh::Reader<'a, 's, F>) -> Option<Self> {
        // get positions or return None if they don't exist and ignore the primitve
        let vertices: Vec<_> = reader
            .read_positions()?
            .map(|pos| PrimitiveVertex {
                position: pos.into(),
                ..Default::default()
            })
            .collect();
        // get indices or assign each vertex an index
        let indices: Vec<_> = reader
            .read_indices()
            .map(|i| i.into_u32().collect())
            .unwrap_or_else(|| (0..vertices.len() as u32).collect());

        Some(Self {
            vertices,
            indices,
            reader,
        })
    }
    fn set_normals(&mut self) {
        match self.reader.read_normals() {
            // use provided normals
            Some(normals) => {
                for (i, normal) in normals.enumerate() {
                    self.vertices[i].normal = normal.into();
                }
            }
            // calculate flat normals
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
                assert!(
                    mikktspace::generate_tangents(self),
                    "generating tangents failed"
                );
            }
        }
    }
}
impl<'a, 's, F: Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>> mikktspace::Geometry
    for VertexData<'a, 's, F>
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
        // todo!("use correct tex coord set");
        self.vertices[self.indices[face * 3 + vert] as usize]
            .uv_0
            .into()
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        self.vertices[self.indices[face * 3 + vert] as usize].tangent = tangent.into()
    }
}

#[derive(Clone, Debug)]
pub struct Primitive {
    material_set: Arc<DescriptorSet>,
    vbuf: Subbuffer<[PrimitiveVertex]>,
    ibuf: Subbuffer<[u32]>,
    ilen: u32,
}
impl Primitive {
    pub fn from_loader(
        primitive: gltf::Primitive,
        buffers: &[gltf::buffer::Data],
        images: &mut [Option<::image::RgbaImage>],
        loader: &mut Loader,
    ) -> Option<Self> {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));

        let mut vertex_data = VertexData::new(reader)?;
        vertex_data.set_normals();
        vertex_data.set_textures_sets();

        let material = loader.get_material(primitive.material(), images);
        if material.uniform.nm_set >= 0 {
            vertex_data.set_tangents();
        }

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
            vertex_data.vertices,
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
            vertex_data.indices,
        )
        .unwrap();

        let vbuf = Buffer::new_slice(
            loader.allocators.mem.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST | BufferUsage::VERTEX_BUFFER,
                // | BufferUsage::SHADER_DEVICE_ADDRESS
                // | BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY,
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
                // | BufferUsage::SHADER_DEVICE_ADDRESS
                // | BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
            ibuf_stage.len(),
        )
        .unwrap();

        loader
            .builder
            .copy_buffer(CopyBufferInfo::buffers(vbuf_stage, vbuf.clone()))
            .unwrap();
        loader
            .builder
            .copy_buffer(CopyBufferInfo::buffers(ibuf_stage, ibuf.clone()))
            .unwrap();

        Some(Self {
            ilen: ibuf.len() as u32,
            vbuf,
            ibuf,
            material_set: material.set.clone(),
        })
    }
    pub fn render<L>(
        self,
        pipeline_layout: Arc<PipelineLayout>,
        instances: u32,
        builder: &mut AutoCommandBufferBuilder<L>,
    ) {
        builder
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                pipeline_layout,
                2,
                self.material_set,
            )
            .unwrap();
        builder.bind_vertex_buffers(0, self.vbuf).unwrap();
        builder.bind_index_buffer(self.ibuf).unwrap();
        unsafe { builder.draw_indexed(self.ilen, instances, 0, 0, 0) }.unwrap();
    }
    // pub fn vbuf(&self) -> &Subbuffer<[PrimitiveVertex]> {
    //     &self.vbuf
    // }
    // pub fn ibuf(&self) -> &Subbuffer<[u32]> {
    //     &self.ibuf
    // }
}

#[derive(Debug)]
pub struct Mesh {
    // pub index: usize,
    // pub name: Option<String>,
    pub primitives: Vec<Primitive>,
}
impl Mesh {
    pub fn from_loader(
        mesh: gltf::Mesh,
        buffers: &[gltf::buffer::Data],
        images: &mut [Option<::image::RgbaImage>],
        loader: &mut Loader,
    ) -> Self {
        let primitives = mesh
            .primitives()
            .filter_map(|primitive| {
                let is_triangle = primitive.mode() == gltf::mesh::Mode::Triangles;
                if !is_triangle {
                    log::warn!("triangle primitives allowed only for now. skipping.");
                    None
                } else {
                    let primitve = Primitive::from_loader(primitive, buffers, images, loader);
                    if primitve.is_none() {
                        log::warn!("a primitive couldn't be built. skipping.");
                    }
                    primitve
                }
            })
            .collect();

        Self { primitives }
    }
}
