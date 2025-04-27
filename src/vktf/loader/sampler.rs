use std::sync::Arc;
use vulkano::{
    device::Device,
    image::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
};

// TODO: cache samplers
pub fn create_sampler(device: Arc<Device>, sampler: gltf::texture::Sampler) -> Arc<Sampler> {
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

    let anisotropy = Some(device.physical_device().properties().max_sampler_anisotropy);

    Sampler::new(
        device,
        SamplerCreateInfo {
            mag_filter,
            min_filter,
            mipmap_mode,
            address_mode,
            anisotropy,
            ..SamplerCreateInfo::simple_repeat_linear()
        },
    )
    .unwrap()
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
