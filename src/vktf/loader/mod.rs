use crate::Allocators;
use image::Image;
use material::{Material, MaterialUniform};
use mesh::Mesh;
use sampler::Sampler;
use std::{path::Path, sync::Arc};
use texture::Texture;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer},
    descriptor_set::layout::DescriptorSetLayout,
    device::{Device, DeviceOwned},
    format::Format,
    image::{ImageCreateInfo, ImageUsage, sampler::SamplerCreateInfo, view::ImageView},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
};

pub mod image;
pub mod material;
pub mod mesh;
pub mod primitive;
pub mod sampler;
pub mod texture;

#[derive(Default)]
pub struct Vktf {
    samplers: Vec<Sampler>,
    images: Vec<Image>,
    textures: Vec<Texture>,
    materials: Vec<Material>,
    meshes: Vec<Mesh>,

    default_texture: Option<Texture>,
    default_sampler: Option<Sampler>,
    default_material: Option<Material>,
}

struct Loader<'a> {
    device: Arc<Device>,
    allocators: Allocators,
    material_set_layout: Arc<DescriptorSetLayout>,
    builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,

    vktf: Vktf,
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
            vktf: Vktf::default(),
        }
    }

    fn load_samplers(&mut self, document: &gltf::Document) {
        let device = self.allocators.mem.device();
        for sampler in document.samplers() {
            self.vktf
                .samplers
                .push(Sampler::new(device.clone(), sampler));
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

        for ((data, image), is_srgb) in images.into_iter().zip(document.images()).zip(is_srgb) {
            self.vktf.images.push(Image::new(
                self.allocators.mem.clone(),
                self.builder,
                image,
                data,
                is_srgb,
            ));
        }
    }
    fn load_textures(&mut self, document: &gltf::Document) {
        for texture in document.textures() {
            let texture = Texture::from_loader(texture, self);
            self.vktf.textures.push(texture);
        }
    }
    fn load_materials(&mut self, document: &gltf::Document) {
        for material in document.materials() {
            let material = Material::from_loader(material, self);
            self.vktf.materials.push(material);
        }
    }
    fn load_meshes(&mut self, document: &gltf::Document, buffers: &[gltf::buffer::Data]) {
        for mesh in document.meshes() {
            let mesh = Mesh::from_loader(mesh, buffers, self);
            self.vktf.meshes.push(mesh);
        }
    }

    fn get_sampler(&mut self, sampler: gltf::texture::Sampler) -> &Sampler {
        match sampler.index() {
            Some(i) => &self.vktf.samplers[i],
            None => {
                if self.vktf.default_sampler.is_none() {
                    self.vktf.default_sampler = Some(Sampler::new(self.device.clone(), sampler))
                }
                self.vktf.default_sampler.as_ref().unwrap()
            }
        }
    }
    fn get_image(&self, image: gltf::Image) -> &Image {
        &self.vktf.images[image.index()]
    }
    fn get_texture(&self, texture: gltf::Texture) -> Texture {
        self.vktf.textures[texture.index()].clone()
    }
    fn get_material(&mut self, material: gltf::Material) -> Material {
        if let Some(i) = material.index() {
            self.vktf.materials[i].clone()
        } else {
            if self.vktf.default_material.is_none() {
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
                    name: Some(Arc::from("vkTF Default Material")),
                };
                self.vktf.default_material = Some(material);
            }
            self.vktf.default_material.clone().unwrap()
        }
    }

    fn get_default_texture(&mut self) -> Texture {
        if self.vktf.default_texture.is_none() {
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
            let image = vulkano::image::Image::new(
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
            let vk_view = ImageView::new_default(image).unwrap();
            let vk_sampler = vulkano::image::sampler::Sampler::new(
                self.device.clone(),
                SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
            )
            .unwrap();
            self.vktf.default_texture = Some(Texture {
                image: Image {
                    vk: vk_view,
                    name: Some(Arc::from("vkTF Default Image")),
                },
                sampler: Sampler {
                    vk: vk_sampler,
                    name: Some(Arc::from("vkTF Default Texture Sampler")),
                },
                name: Some(Arc::from("vkTF Default Texture")),
            });
        }
        self.vktf.default_texture.clone().unwrap()
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
            meshes: loader.vktf.meshes,
            document,
        })
    }
}
