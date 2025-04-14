#[derive(Debug)]
pub struct Primitive {
    pub index: usize,
}
// for primative in mesh.primitives() {
//     if primative.mode() != gltf::mesh::Mode::Triangles {
//         log::warn!("Only triangle primatives supported. Skipping primative.");
//         continue;
//     }

//     let reader =
//         primative.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));

//     let vertices = reader
//         .read_positions()
//         .unwrap()
//         .zip(reader.read_normals().unwrap())
//         .map(|(pos, norm)| GltfVertex {
//             position: pos.into(),
//             normal: norm.into(),
//         });
//     let indices = reader.read_indices().unwrap().into_u32();

//     let vbuf = Buffer::from_iter(
//         alloc.clone(),
//         BufferCreateInfo {
//             usage: BufferUsage::VERTEX_BUFFER,
//             ..Default::default()
//         },
//         AllocationCreateInfo {
//             memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
//                 | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
//             ..Default::default()
//         },
//         vertices,
//     )
//     .unwrap();
//     let ibuf = Buffer::from_iter(
//         alloc.clone(),
//         BufferCreateInfo {
//             usage: BufferUsage::INDEX_BUFFER,
//             ..Default::default()
//         },
//         AllocationCreateInfo {
//             memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
//                 | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
//             ..Default::default()
//         },
//         indices,
//     )
//     .unwrap();
//     let ilen = ibuf.len() as u32;

//     let mesh = GltfMesh { vbuf, ibuf, ilen };
//     meshes.push(mesh);
// }
