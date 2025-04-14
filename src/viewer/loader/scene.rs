use super::{Import, Loader, node::Node};
use std::sync::Arc;

#[derive(Debug)]
pub struct Scene {
    pub index: usize,
    pub name: Option<String>,
    pub nodes: Vec<Arc<Node>>,
}
impl Scene {
    pub fn from_loader(scene: gltf::Scene, import: &Import, loader: &mut Loader) -> Self {
        Self {
            index: scene.index(),
            name: scene.name().map(String::from),
            nodes: scene
                .nodes()
                .map(|node| loader.get_node(node, import))
                .collect(),
        }
    }
}
