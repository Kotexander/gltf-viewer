use std::{path::Path, sync::Arc};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    device::{Device, DeviceOwned},
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageUsage,
        sampler::{Sampler, SamplerAddressMode, SamplerCreateInfo},
        view::ImageView,
    },
    memory::allocator::{AllocationCreateInfo, MemoryAllocator},
};

mod image;
mod primitive;
mod sampler;

use image::*;
pub use primitive::*;
use sampler::*;

#[derive(Default)]
pub struct Vktf {
    samplers: Vec<Arc<Sampler>>,
    images: Vec<Arc<ImageView>>,
    meshes: Vec<Vec<Primitive>>,

    default_sampler: Option<Arc<Sampler>>,
    default_image: Option<Arc<ImageView>>,
}
impl Vktf {
    pub fn get_sampler(&self, index: Option<usize>) -> Option<&Arc<Sampler>> {
        match index {
            Some(i) => self.samplers.get(i),
            None => self.default_sampler.as_ref(),
        }
    }
    pub fn get_image(&self, index: Option<usize>) -> Option<&Arc<ImageView>> {
        match index {
            Some(i) => self.images.get(i),
            None => self.default_image.as_ref(),
        }
    }
    pub fn get_mesh(&self, index: usize) -> Option<&[Primitive]> {
        self.meshes.get(index).map(Vec::as_slice)
    }
}

pub struct Loader<'a, L> {
    device: Arc<Device>,
    allocator: Arc<dyn MemoryAllocator>,
    builder: &'a mut AutoCommandBufferBuilder<L>,

    vktf: Vktf,
}
impl<'a, L> Loader<'a, L> {
    pub fn new(
        allocator: Arc<dyn MemoryAllocator>,
        builder: &'a mut AutoCommandBufferBuilder<L>,
    ) -> Self {
        Self {
            device: allocator.device().clone(),
            allocator,
            builder,
            vktf: Vktf::default(),
        }
    }
    pub fn load(
        mut self,
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        images: Vec<gltf::image::Data>,
    ) -> Vktf {
        self.load_meshes(document, buffers);
        self.load_images(document, images);
        self.load_samplers(document);
        self.load_defaults();
        self.vktf
    }

    fn load_samplers(&mut self, document: &gltf::Document) {
        for sampler in document.samplers() {
            self.vktf
                .samplers
                .push(create_vk_sampler(self.device.clone(), &sampler));
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

        for (data, is_srgb) in images.into_iter().zip(is_srgb) {
            let image = create_vk_image(self.allocator.clone(), self.builder, data, is_srgb);
            let view = ImageView::new_default(image).unwrap();
            self.vktf.images.push(view);
        }
    }
    fn load_meshes(&mut self, document: &gltf::Document, buffers: &[gltf::buffer::Data]) {
        for mesh in document.meshes() {
            let primitives = mesh
                .primitives()
                .map(|primitive| Primitive::from_loader(&primitive, buffers, self).unwrap()) // TODO: do smt better than unwrap
                .collect();
            self.vktf.meshes.push(primitives);
        }
    }
    fn load_defaults(&mut self) {
        let address_mode = [
            convert_wrap(gltf::texture::WrappingMode::default()),
            convert_wrap(gltf::texture::WrappingMode::default()),
            SamplerAddressMode::ClampToEdge,
        ];
        let mag_filter = DEFAULT_MAG;
        let (min_filter, mipmap_mode) = (DEFAULT_MIN, DEFAULT_MIPMAP);
        let anisotropy = Some(
            self.device
                .physical_device()
                .properties()
                .max_sampler_anisotropy,
        );

        self.vktf.default_sampler = Some(
            Sampler::new(
                self.device.clone(),
                SamplerCreateInfo {
                    mag_filter,
                    min_filter,
                    mipmap_mode,
                    address_mode,
                    anisotropy,
                    ..SamplerCreateInfo::simple_repeat_linear()
                },
            )
            .unwrap(),
        );

        let image = Image::new(
            self.allocator.clone(),
            ImageCreateInfo {
                extent: [1, 1, 1],
                usage: ImageUsage::SAMPLED,
                format: Format::R8G8B8A8_UNORM,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .unwrap();
        let view = ImageView::new_default(image).unwrap();
        self.vktf.default_image = Some(view);
    }
}

pub struct VktfDocument {
    pub vktf: Vktf,
    pub document: gltf::Document,
}
impl VktfDocument {
    pub fn new(
        allocator: Arc<dyn MemoryAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        path: impl AsRef<Path>,
    ) -> gltf::Result<Self> {
        let (document, buffers, images) = gltf::import(path)?;

        let loader = Loader::new(allocator, builder);
        let vktf = loader.load(&document, &buffers, images);

        Ok(Self { document, vktf })
    }
}
