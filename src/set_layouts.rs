use std::{collections::BTreeMap, sync::Arc};
use vulkano::{
    descriptor_set::layout::{
        DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
        DescriptorType,
    },
    device::Device,
    shader::ShaderStages,
};

fn texture_layout(set: u32) -> (u32, DescriptorSetLayoutBinding) {
    (
        set,
        DescriptorSetLayoutBinding {
            stages: ShaderStages::FRAGMENT,
            ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::CombinedImageSampler)
        },
    )
}

#[derive(Clone)]
pub struct SetLayouts {
    pub camera: Arc<DescriptorSetLayout>,
    pub texture: Arc<DescriptorSetLayout>,
    pub material: Arc<DescriptorSetLayout>,
    pub environment: Arc<DescriptorSetLayout>,
}
impl SetLayouts {
    pub fn new(device: Arc<Device>) -> Self {
        let camera = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([(
                    0,
                    DescriptorSetLayoutBinding {
                        stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                    },
                )]),
                ..Default::default()
            },
        )
        .unwrap();
        let material = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([
                    (
                        0,
                        DescriptorSetLayoutBinding {
                            stages: ShaderStages::FRAGMENT,
                            ..DescriptorSetLayoutBinding::descriptor_type(
                                DescriptorType::UniformBuffer,
                            )
                        },
                    ),
                    texture_layout(1),
                    texture_layout(2),
                    texture_layout(3),
                    texture_layout(4),
                    texture_layout(5),
                ]),
                ..Default::default()
            },
        )
        .unwrap();
        let texture = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([texture_layout(0)]),
                ..Default::default()
            },
        )
        .unwrap();
        let environment = DescriptorSetLayout::new(
            device,
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([texture_layout(0)]),
                ..Default::default()
            },
        )
        .unwrap();

        Self {
            camera,
            texture,
            material,
            environment,
        }
    }
}
