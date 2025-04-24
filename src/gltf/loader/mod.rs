use crate::Allocators;
use image::{convert_image, load_image};
use material::{Material, MaterialUniform};
use mesh::Mesh;
use std::{path::Path, sync::Arc};
use texture::Texture;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer},
    descriptor_set::layout::DescriptorSetLayout,
    device::DeviceOwned,
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageUsage,
        sampler::{Sampler, SamplerCreateInfo},
        view::ImageView,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
};

mod image;
mod material;
pub mod mesh;
mod texture;

pub struct Loader<'a> {
    allocators: Allocators,
    material_set_layout: Arc<DescriptorSetLayout>,
    builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,

    images: Vec<Option<(Arc<ImageView>, bool)>>,
    textures: Vec<Option<Texture>>,
    material: Vec<Option<Material>>,
    meshes: Vec<Option<Mesh>>,

    default_texture: Option<Texture>,
}
impl<'a> Loader<'a> {
    fn new(
        allocators: Allocators,
        material_set_layout: Arc<DescriptorSetLayout>,
        builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        document: &gltf::Document,
    ) -> Self {
        Self {
            allocators,
            material_set_layout,
            meshes: Vec::from_iter(std::iter::repeat_with(|| None).take(document.meshes().len())),
            textures: Vec::from_iter(
                std::iter::repeat_with(|| None).take(document.textures().len()),
            ),
            material: Vec::from_iter(
                std::iter::repeat_with(|| None).take(document.materials().len()),
            ),
            images: vec![None; document.images().len()],
            builder,
            default_texture: None,
        }
    }

    fn load_scene(
        &mut self,
        scene: gltf::Scene,
        buffers: &[gltf::buffer::Data],
        images: &mut [Option<::image::RgbaImage>],
    ) {
        for node in scene.nodes() {
            self.load_node(node, buffers, images);
        }
    }
    fn load_node(
        &mut self,
        node: gltf::Node,
        buffers: &[gltf::buffer::Data],
        images: &mut [Option<::image::RgbaImage>],
    ) {
        if let Some(mesh) = node.mesh() {
            self.load_mesh(mesh, buffers, images);
        }
        for child in node.children() {
            self.load_node(child, buffers, images);
        }
    }
    fn load_mesh(
        &mut self,
        mesh: gltf::Mesh,
        buffers: &[gltf::buffer::Data],
        images: &mut [Option<::image::RgbaImage>],
    ) {
        let i = mesh.index();
        if self.meshes[i].is_some() {
            return;
        }

        let mesh = Mesh::from_loader(mesh, buffers, images, self);
        self.meshes[i] = Some(mesh);
    }
    fn get_material(
        &mut self,
        material: gltf::Material,
        images: &mut [Option<::image::RgbaImage>],
    ) -> Material {
        let Some(i) = material.index() else {
            let uniform = MaterialUniform::default();
            return Material {
                set: Material::create_set(
                    self.allocators.set.clone(),
                    self.material_set_layout.clone(),
                    Material::create_factor_buffer(self.allocators.mem.clone(), uniform),
                    self.get_default_texture(),
                    self.get_default_texture(),
                    self.get_default_texture(),
                    self.get_default_texture(),
                    self.get_default_texture(),
                ),
                uniform,
            };
        };
        let mat = &self.material[i];
        if mat.is_none() {
            let mat = Material::from_loader(material, images, self);
            self.material[i] = Some(mat)
        }

        self.material[i].clone().unwrap()
    }
    fn get_texture(
        &mut self,
        texture: gltf::Texture,
        is_srgb: bool,
        images: &mut [Option<::image::RgbaImage>],
    ) -> Texture {
        let i = texture.index();
        let tex = &self.textures[i];
        if tex.is_none() {
            let tex = Texture::from_loader(texture, is_srgb, images, self);
            self.textures[i] = Some(tex)
        }

        self.textures[i].clone().unwrap()
    }
    fn get_image(
        &mut self,
        image: gltf::Image,
        is_srgb: bool,
        images: &mut [Option<::image::RgbaImage>],
    ) -> &Arc<ImageView> {
        let i = image.index();
        let img = &self.images[i];
        if img.is_none() {
            let img = load_image(
                self.allocators.mem.clone(),
                self.builder,
                images[i].take().unwrap(),
                is_srgb,
            );
            self.images[i] = Some((img, is_srgb))
        }

        let (tex, srgb) = self.images[i].as_ref().unwrap();
        assert_eq!(
            is_srgb, *srgb,
            "an image is being loaded as sRGB color data and linear RGB data which is weird and unimplemented"
        );

        tex
    }

    fn get_default_texture(&mut self) -> Texture {
        if self.default_texture.is_none() {
            let stage_image = Buffer::from_data(
                self.allocators.mem.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                [255u8; 4],
            )
            .unwrap();
            let image = Image::new(
                self.allocators.mem.clone(),
                ImageCreateInfo {
                    usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                    extent: [1; 3],
                    format: Format::R8G8B8A8_UNORM,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap();
            self.builder
                .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                    stage_image,
                    image.clone(),
                ))
                .unwrap();
            let view = ImageView::new_default(image).unwrap();
            let sampler = Sampler::new(
                self.allocators.mem.device().clone(),
                SamplerCreateInfo::simple_repeat_linear(),
            )
            .unwrap();
            self.default_texture = Some(Texture { view, sampler });
        }
        self.default_texture.clone().unwrap()
    }
}

pub struct GltfLoader {
    pub meshes: Vec<Option<Mesh>>,
    pub document: gltf::Document,
}
impl GltfLoader {
    pub fn new(
        allocators: Allocators,
        material_set_layout: Arc<DescriptorSetLayout>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        path: impl AsRef<Path>,
    ) -> gltf::Result<Self> {
        let (document, buffers, images) = gltf::import(path)?;

        let mut loader = Loader::new(allocators, material_set_layout, builder, &document);
        let mut images: Vec<_> = images
            .into_iter()
            .map(|data| Some(convert_image(data)))
            .collect();
        loader.load_scene(document.default_scene().unwrap(), &buffers, &mut images);

        Ok(Self {
            meshes: loader.meshes,
            document,
        })
    }
}
