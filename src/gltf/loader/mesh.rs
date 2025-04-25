use super::{Loader, primitive::Primitive};

#[derive(Debug)]
pub struct Mesh {
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
