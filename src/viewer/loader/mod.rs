use crate::Allocators;
use image::load_images;
use material::Material;
use mesh::Mesh;
use std::{path::Path, sync::Arc};
use texture::Texture;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer},
    descriptor_set::layout::DescriptorSetLayout,
    format::Format,
    image::{Image, ImageCreateInfo, ImageUsage, view::ImageView},
    memory::allocator::AllocationCreateInfo,
};

mod image;
mod material;
pub mod mesh;
mod texture;

pub struct Loader {
    allocators: Allocators,
    material_set_layout: Arc<DescriptorSetLayout>,
    builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,

    images: Vec<Arc<ImageView>>,
    textures: Vec<Arc<Texture>>,
    material: Vec<Arc<Material>>,
    meshes: Vec<Arc<Mesh>>,

    default_texture: Option<Arc<ImageView>>,
}
impl Loader {
    fn new(
        allocators: Allocators,
        material_set_layout: Arc<DescriptorSetLayout>,
        queue_family: u32,
    ) -> Self {
        let builder = AutoCommandBufferBuilder::primary(
            allocators.cmd.clone(),
            queue_family,
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        Self {
            allocators,
            material_set_layout,
            meshes: vec![],
            textures: vec![],
            material: vec![],
            images: vec![],
            builder,
            default_texture: None,
        }
    }

    fn load_images(&mut self, document: &gltf::Document, images: Vec<gltf::image::Data>) {
        self.images = load_images(document, images, &self.allocators.mem, &mut self.builder);
    }

    fn load_textures(&mut self, document: &gltf::Document) {
        self.textures = document
            .textures()
            .map(|texture| Arc::new(Texture::from_loader(texture, self)))
            .collect();
    }

    fn load_materials(&mut self, document: &gltf::Document) {
        self.material = document
            .materials()
            .map(|material| Arc::new(Material::from_loader(material, self)))
            .collect();
    }

    fn load_meshes(&mut self, document: &gltf::Document, buffers: &[gltf::buffer::Data]) {
        self.meshes = document
            .meshes()
            .map(|mesh| Arc::new(Mesh::from_loader(mesh, buffers, self)))
            .collect();
    }

    fn get_default_texture(&mut self) -> &Arc<ImageView> {
        if self.default_texture.is_none() {
            let image = Image::new(
                self.allocators.mem.clone(),
                ImageCreateInfo {
                    usage: ImageUsage::SAMPLED,
                    extent: [1; 3],
                    format: Format::R8G8B8A8_SRGB,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap();
            let view = ImageView::new_default(image).unwrap();
            self.default_texture = Some(view);
        }
        self.default_texture.as_ref().unwrap()
    }
}

pub struct GltfLoader {
    pub cb: Arc<PrimaryAutoCommandBuffer>,
    pub meshes: Vec<Arc<Mesh>>,
    pub document: gltf::Document,
}
impl GltfLoader {
    pub fn new(
        allocators: Allocators,
        material_set_layout: Arc<DescriptorSetLayout>,
        queue_family: u32,
        path: impl AsRef<Path>,
    ) -> gltf::Result<Self> {
        let (document, buffers, images) = gltf::import(path)?;

        let mut loader = Loader::new(allocators, material_set_layout, queue_family);
        loader.load_images(&document, images);
        loader.load_textures(&document);
        loader.load_materials(&document);
        loader.load_meshes(&document, &buffers);

        Ok(Self {
            cb: loader.builder.build().unwrap(),
            meshes: loader.meshes,
            document,
        })
    }
}
