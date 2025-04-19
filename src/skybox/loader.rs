use crate::{
    Allocators,
    cubemap::{
        CubeMesh, CubemapPipelineLayout, CubemapShaders,
        renderer::{CubemapRenderPass, CubemapRenderPipeline, create_cubemap_image},
    },
    set_layouts::SetLayouts,
};
use image::{EncodableLayout, ImageError};
use std::{path::Path, sync::Arc};
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo},
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    device::DeviceOwned,
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageType, ImageUsage,
        sampler::{Sampler, SamplerCreateInfo},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::Pipeline,
};

#[derive(Clone)]
pub struct SkyboxLoader {
    pub equirectangular_renderer: CubemapRenderPipeline,
    pub convolute_renderer: CubemapRenderPipeline,
    pub allocators: Allocators,
}
impl SkyboxLoader {
    pub fn new(
        allocators: Allocators,
        pipeline_layout: &CubemapPipelineLayout,
        shaders: &CubemapShaders,
        set_layouts: &SetLayouts,
        cube: Arc<CubeMesh>,
    ) -> Self {
        let cube_render_pass = Arc::new(CubemapRenderPass::new(
            allocators.mem.clone(),
            allocators.set.clone(),
            set_layouts.camera.clone(),
        ));
        let equirectangular_renderer = CubemapRenderPipeline {
            pipeline: pipeline_layout.clone().create_pipeline(
                shaders.vs.clone(),
                shaders.equi_fs.clone(),
                shaders.vertex_input_state.clone(),
                cube_render_pass.subpass.clone(),
            ),
            renderer: cube_render_pass.clone(),
            cube: cube.clone(),
        };
        let convolute_renderer = CubemapRenderPipeline {
            pipeline: pipeline_layout.clone().create_pipeline(
                shaders.vs.clone(),
                shaders.conv_fs.clone(),
                shaders.vertex_input_state.clone(),
                cube_render_pass.subpass.clone(),
            ),
            renderer: cube_render_pass,
            cube,
        };

        Self {
            equirectangular_renderer,
            convolute_renderer,
            allocators,
        }
    }

    pub fn load<L>(
        &self,
        path: impl AsRef<Path>,
        builder: &mut AutoCommandBufferBuilder<L>,
    ) -> Result<(Arc<Image>, Arc<Image>), LoadSkyboxError> {
        let equi = load_skybox(self.allocators.mem.clone(), path, builder)?;
        let equi_view = ImageView::new_default(equi.clone()).unwrap();
        let equi_set = DescriptorSet::new(
            self.allocators.set.clone(),
            self.equirectangular_renderer
                .pipeline
                .layout()
                .set_layouts()[1]
                .clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                equi_view,
                Sampler::new(
                    self.allocators.mem.device().clone(),
                    SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
                )
                .unwrap(),
            )],
            [],
        )
        .unwrap();

        let cube = create_cubemap_image(self.allocators.mem.clone(), equi.extent()[0] / 4);
        self.equirectangular_renderer
            .render(builder, &equi_set, &cube);

        let cube_view = ImageView::new(
            cube.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                ..ImageViewCreateInfo::from_image(&cube)
            },
        )
        .unwrap();
        let cube_set = DescriptorSet::new(
            self.allocators.set.clone(),
            self.convolute_renderer.pipeline.layout().set_layouts()[1].clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                cube_view.clone(),
                Sampler::new(
                    self.allocators.mem.device().clone(),
                    SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
                )
                .unwrap(),
            )],
            [],
        )
        .unwrap();

        let conv = create_cubemap_image(self.allocators.mem.clone(), 32);
        self.convolute_renderer.render(builder, &cube_set, &conv);

        // let conv_view = ImageView::new(
        //     conv.clone(),
        //     ImageViewCreateInfo {
        //         view_type: ImageViewType::Cube,
        //         ..ImageViewCreateInfo::from_image(&conv)
        //     },
        // )
        // .unwrap();
        // let conv_set = DescriptorSet::new(
        //     allocators.set.clone(),
        //     environment_layout.clone(),
        //     [
        //         WriteDescriptorSet::image_view_sampler(
        //             0,
        //             conv_view,
        //             Sampler::new(
        //                 allocators.mem.device().clone(),
        //                 SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
        //             )
        //             .unwrap(),
        //         ),
        //         WriteDescriptorSet::image_view_sampler(
        //             1,
        //             cube_view,
        //             Sampler::new(
        //                 allocators.mem.device().clone(),
        //                 SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
        //             )
        //             .unwrap(),
        //         ),
        //     ],
        //     [],
        // )
        // .unwrap();

        Ok((cube, conv))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadSkyboxError {
    #[error(transparent)]
    Image(#[from] ImageError),
    #[error("equirectangular image must be 2:1")]
    WrongAspect,
}
fn load_skybox<L>(
    allocator: Arc<StandardMemoryAllocator>,
    path: impl AsRef<Path>,
    builder: &mut AutoCommandBufferBuilder<L>,
) -> Result<Arc<Image>, LoadSkyboxError> {
    // let mut reader = BufReader::new(std::fs::File::open(path).unwrap());
    // let mut image_reader = image::ImageReader::new(&mut reader)
    //     .with_guessed_format()
    //     .unwrap();
    // image_reader.no_limits();
    // let image = image_reader.decode().unwrap().to_rgba32f();

    let image = image::open(path)?.to_rgba32f();
    if image.width() / 2 != image.height() {
        return Err(LoadSkyboxError::WrongAspect);
    }

    let stage_buffer = Buffer::new_slice(
        allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        image.as_bytes().len() as DeviceSize,
    )
    .unwrap();
    stage_buffer
        .write()
        .unwrap()
        .copy_from_slice(image.as_bytes());

    let image = Image::new(
        allocator,
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format: Format::R32G32B32A32_SFLOAT,
            extent: [image.width(), image.height(), 1],
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap();

    builder
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            stage_buffer,
            image.clone(),
        ))
        .unwrap();

    Ok(image)
}
