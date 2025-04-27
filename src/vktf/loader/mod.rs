use crate::Allocators;
use image::{convert_image, load_image};
use material::{Material, MaterialUniform};
use mesh::Mesh;
use sampler::create_sampler;
use std::{path::Path, sync::Arc};
use texture::Texture;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer},
    descriptor_set::layout::DescriptorSetLayout,
    device::{Device, DeviceOwned},
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageUsage,
        sampler::{Sampler, SamplerCreateInfo},
        view::ImageView,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
};

pub mod image;
pub mod material;
pub mod mesh;
pub mod primitive;
pub mod sampler;
pub mod texture;

pub struct Loader<'a> {
    device: Arc<Device>,
    allocators: Allocators,
    material_set_layout: Arc<DescriptorSetLayout>,
    builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,

    samplers: Vec<Arc<Sampler>>,
    images: Vec<Arc<ImageView>>,
    textures: Vec<Texture>,
    materials: Vec<Material>,
    meshes: Vec<Mesh>,

    default_texture: Option<Texture>,
    default_sampler: Option<Arc<Sampler>>,
    default_material: Option<Material>,
}
impl<'a> Loader<'a> {
    fn new(
        allocators: Allocators,
        material_set_layout: Arc<DescriptorSetLayout>,
        builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Self {
        Self {
            device: allocators.mem.device().clone(),
            allocators,
            material_set_layout,
            builder,
            meshes: vec![],
            materials: vec![],
            samplers: vec![],
            images: vec![],
            textures: vec![],

            default_texture: None,
            default_sampler: None,
            default_material: None,
        }
    }

    fn load_samplers(&mut self, document: &gltf::Document) {
        let device = self.allocators.mem.device();
        for sampler in document.samplers() {
            self.samplers.push(create_sampler(device.clone(), sampler));
        }
    }
    fn load_images(&mut self, document: &gltf::Document, images: Vec<gltf::image::Data>) {
        let mut is_srgb = vec![true; images.len()];
        for material in document.materials() {
            if let Some(tex) = material
                .pbr_metallic_roughness()
                .metallic_roughness_texture()
            {
                is_srgb[tex.texture().source().index()] = false;
            }
            if let Some(tex) = material.occlusion_texture() {
                is_srgb[tex.texture().source().index()] = false;
            }
            if let Some(tex) = material.normal_texture() {
                is_srgb[tex.texture().source().index()] = false;
            }
        }

        for (image, is_srgb) in images.into_iter().zip(is_srgb) {
            self.images.push(load_image(
                self.allocators.mem.clone(),
                self.builder,
                convert_image(image),
                is_srgb,
            ));
        }
    }
    fn load_textures(&mut self, document: &gltf::Document) {
        for texture in document.textures() {
            let texture = Texture::from_loader(texture, self);
            self.textures.push(texture);
        }
    }
    fn load_materials(&mut self, document: &gltf::Document) {
        for material in document.materials() {
            let material = Material::from_loader(material, self);
            self.materials.push(material);
        }
    }
    fn load_meshes(&mut self, document: &gltf::Document, buffers: &[gltf::buffer::Data]) {
        for mesh in document.meshes() {
            let mesh = Mesh::from_loader(mesh, buffers, self);
            self.meshes.push(mesh);
        }
    }

    fn get_sampler(&mut self, sampler: gltf::texture::Sampler) -> &Arc<Sampler> {
        match sampler.index() {
            Some(i) => &self.samplers[i],
            None => {
                if self.default_sampler.is_none() {
                    self.default_sampler = Some(create_sampler(self.device.clone(), sampler))
                }
                self.default_sampler.as_ref().unwrap()
            }
        }
    }
    fn get_image(&self, image: gltf::Image) -> &Arc<ImageView> {
        &self.images[image.index()]
    }
    fn get_texture(&self, texture: gltf::Texture) -> Texture {
        self.textures[texture.index()].clone()
    }
    fn get_material(&mut self, material: gltf::Material) -> Material {
        if let Some(i) = material.index() {
            self.materials[i].clone()
        } else {
            if self.default_material.is_none() {
                let material = Material {
                    set: Material::create_set(
                        self.allocators.set.clone(),
                        self.material_set_layout.clone(),
                        self.get_default_texture(),
                        self.get_default_texture(),
                        self.get_default_texture(),
                        self.get_default_texture(),
                        self.get_default_texture(),
                    ),
                    uniform: MaterialUniform::default(),
                };
                self.default_material = Some(material);
            }
            self.default_material.clone().unwrap()
        }
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
                self.device.clone(),
                SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
            )
            .unwrap();
            self.default_texture = Some(Texture { view, sampler });
        }
        self.default_texture.clone().unwrap()
    }
}

pub struct GltfLoader {
    pub meshes: Vec<Mesh>,
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

        let mut loader = Loader::new(allocators, material_set_layout, builder);
        loader.load_samplers(&document);
        loader.load_images(&document, images);
        loader.load_textures(&document);
        loader.load_materials(&document);
        loader.load_meshes(&document, &buffers);

        Ok(Self {
            meshes: loader.meshes,
            document,
        })
    }
}
