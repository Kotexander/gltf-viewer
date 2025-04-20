use super::Loader;
use std::sync::Arc;
use vulkano::{
    descriptor_set::WriteDescriptorSet,
    device::DeviceOwned,
    image::{
        sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
        view::ImageView,
    },
};

#[derive(Clone)]
pub struct Texture {
    pub view: Arc<ImageView>,
    pub sampler: Arc<Sampler>,
}
impl Texture {
    pub fn from_loader(
        texture: gltf::Texture,
        is_srgb: bool,
        images: &mut [Option<::image::RgbaImage>],
        loader: &mut Loader,
    ) -> Texture {
        let sampler = texture.sampler();
        let address_mode = [
            convert_wrap(sampler.wrap_s()),
            convert_wrap(sampler.wrap_t()),
            SamplerAddressMode::ClampToEdge,
        ];
        let mag_filter = sampler
            .mag_filter()
            .map(convert_mag_filter)
            .unwrap_or(Filter::Linear);
        let (min_filter, mipmap_mode) = sampler
            .min_filter()
            .map(convert_min_filter)
            .unwrap_or((Filter::Linear, SamplerMipmapMode::Linear));
        let sampler = Sampler::new(
            loader.allocators.mem.device().clone(),
            SamplerCreateInfo {
                mag_filter,
                min_filter,
                mipmap_mode,
                address_mode,
                ..SamplerCreateInfo::simple_repeat_linear()
            },
        )
        .unwrap();

        let view = loader.get_image(texture.source(), is_srgb, images).clone();

        Self { view, sampler }
    }
    pub fn bind(self, binding: u32) -> WriteDescriptorSet {
        WriteDescriptorSet::image_view_sampler(binding, self.view, self.sampler)
    }
}

fn convert_wrap(wrap: gltf::texture::WrappingMode) -> SamplerAddressMode {
    match wrap {
        gltf::texture::WrappingMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
        gltf::texture::WrappingMode::Repeat => SamplerAddressMode::Repeat,
    }
}

fn convert_mag_filter(filter: gltf::texture::MagFilter) -> Filter {
    match filter {
        gltf::texture::MagFilter::Nearest => Filter::Nearest,
        gltf::texture::MagFilter::Linear => Filter::Linear,
    }
}

#[rustfmt::skip]
fn convert_min_filter(filter: gltf::texture::MinFilter) -> (Filter, SamplerMipmapMode) {
    match filter {
        gltf::texture::MinFilter::Nearest => (Filter::Nearest, SamplerMipmapMode::Nearest),
        gltf::texture::MinFilter::Linear => (Filter::Linear, SamplerMipmapMode::Nearest),
        gltf::texture::MinFilter::LinearMipmapLinear => (Filter::Linear, SamplerMipmapMode::Linear),
        gltf::texture::MinFilter::NearestMipmapLinear => (Filter::Nearest, SamplerMipmapMode::Linear),
        gltf::texture::MinFilter::LinearMipmapNearest => (Filter::Linear, SamplerMipmapMode::Nearest),
        gltf::texture::MinFilter::NearestMipmapNearest => (Filter::Nearest, SamplerMipmapMode::Nearest),
    }
}
