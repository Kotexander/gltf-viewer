use super::{Import, Loader, mesh::Mesh};
use nalgebra_glm as glm;
use std::sync::Arc;

#[derive(Debug)]
pub struct Node {
    pub index: usize,
    pub name: Option<String>,
    pub translation: glm::Mat4,
    pub children: Vec<Arc<Node>>,
    pub mesh: Option<Arc<Mesh>>,
    pub camera: Option<glm::Mat4>,
}
impl Node {
    pub fn from_loader(node: gltf::Node, import: &Import, loader: &mut Loader) -> Self {
        Self {
            index: node.index(),
            name: node.name().map(String::from),
            translation: node.transform().matrix().into(),
            children: node
                .children()
                .map(|child| loader.get_node(child, import))
                .collect(),
            mesh: node.mesh().map(|mesh| loader.get_mesh(mesh, import)),
            camera: node.camera().map(calc_camera),
        }
    }
}

/// TODO: test this and confirm spec compliance
fn calc_camera(camera: gltf::Camera) -> glm::Mat4 {
    use gltf::camera::Projection;
    match camera.projection() {
        Projection::Orthographic(orthographic) => {
            let half_xmag = orthographic.xmag() / 2.0;
            let half_ymag = orthographic.ymag() / 2.0;
            glm::ortho_rh_zo(
                -half_xmag,
                half_xmag,
                -half_ymag,
                half_ymag,
                orthographic.znear(),
                orthographic.zfar(),
            )
        }
        Projection::Perspective(perspective) => glm::perspective_rh_zo(
            perspective.aspect_ratio().unwrap_or(1.0),
            perspective.yfov(),
            perspective.znear(),
            perspective.zfar().unwrap_or(100.0),
        ),
    }
}
