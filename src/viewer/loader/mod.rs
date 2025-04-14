use crate::Allocators;
use mesh::Mesh;
use node::Node;
use scene::Scene;
use std::{path::Path, sync::Arc};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SecondaryAutoCommandBuffer},
    device::Queue,
};

pub mod mesh;
pub mod node;
pub mod scene;

pub struct Import {
    pub document: gltf::Document,
    pub buffers: Vec<gltf::buffer::Data>,
    pub images: Vec<gltf::image::Data>,
}
pub struct Loader {
    allocators: Allocators,
    builder: Option<AutoCommandBufferBuilder<SecondaryAutoCommandBuffer>>,
    queue: Arc<Queue>,
    nodes: Vec<Arc<Node>>,
    meshes: Vec<Arc<Mesh>>,
}
impl Loader {
    pub fn new(allocators: Allocators, queue: Arc<Queue>) -> Self {
        Self {
            allocators,
            nodes: vec![],
            meshes: vec![],
            builder: None,
            queue,
        }
    }

    pub fn builder(&mut self) -> &mut AutoCommandBufferBuilder<SecondaryAutoCommandBuffer> {
        if self.builder.is_none() {
            self.builder = Some(
                AutoCommandBufferBuilder::secondary(
                    self.allocators.cmd.clone(),
                    self.queue.queue_family_index(),
                    CommandBufferUsage::OneTimeSubmit,
                    Default::default(),
                )
                .unwrap(),
            );
        }
        self.builder.as_mut().unwrap()
    }

    fn add_node(&mut self, node: Arc<Node>) {
        match self
            .nodes
            .binary_search_by_key(&node.index, |node| node.index)
        {
            Ok(_) => panic!("nodes[{}] already exists.", node.index),
            Err(i) => self.nodes.insert(i, node),
        }
    }
    pub fn get_node(&mut self, node: gltf::Node, import: &Import) -> Arc<Node> {
        match self
            .nodes
            .binary_search_by_key(&node.index(), |node| node.index)
        {
            Ok(i) => self.nodes[i].clone(),
            Err(_) => {
                let node = Arc::new(Node::from_loader(node, import, self));
                self.add_node(node.clone());
                node
            }
        }
    }

    pub fn get_mesh(&mut self, mesh: gltf::Mesh, import: &Import) -> Arc<Mesh> {
        match self
            .meshes
            .binary_search_by_key(&mesh.index(), |mesh| mesh.index)
        {
            Ok(i) => self.meshes[i].clone(),
            Err(i) => {
                let mesh = Arc::new(Mesh::from_loader(mesh, import, self));
                self.meshes.insert(i, mesh.clone());
                mesh
            }
        }
    }
}

pub struct GltfLoader {
    import: Import,
    loader: Loader,
}
impl GltfLoader {
    pub fn new(
        allocators: Allocators,
        queue: Arc<Queue>,
        // material_set_layout: Arc<DescriptorSetLayout>,
        path: impl AsRef<Path>,
    ) -> gltf::Result<Self> {
        let (document, buffers, images) = gltf::import(path)?;

        Ok(Self {
            import: Import {
                document,
                buffers,
                images,
            },
            loader: Loader::new(allocators, queue),
        })
    }
    pub fn load_default_scene(&mut self) -> Option<Scene> {
        self.import
            .document
            .default_scene()
            .map(|scene| Scene::from_loader(scene, &self.import, &mut self.loader))
    }

    pub fn build(&mut self) -> Arc<SecondaryAutoCommandBuffer> {
        self.loader.builder.take().unwrap().build().unwrap()
    }
}
