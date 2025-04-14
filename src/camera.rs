use nalgebra_glm as glm;
use std::f32::consts::FRAC_PI_2;
use std::f32::consts::FRAC_PI_3;
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy)]
pub struct OrbitCamera {
    pub target: glm::Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub zoom: f32,

    pub fov: f32,
    pub near: f32,
    pub far: f32,
}
impl OrbitCamera {
    pub fn eye(&self) -> glm::Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        self.target + glm::vec3(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch) * self.zoom
    }
    pub fn up(&self) -> glm::Vec3 {
        if self.is_upside_down() {
            glm::Vec3::y()
        } else {
            -glm::Vec3::y()
        }
    }

    pub fn look_at(&self) -> glm::Mat4 {
        glm::look_at_lh(&self.eye(), &self.target, &self.up())
    }
    pub fn perspective(&self, aspect: f32) -> glm::Mat4 {
        glm::perspective_lh_zo(aspect, self.fov, self.near, self.far)
    }

    pub fn is_upside_down(&self) -> bool {
        self.pitch > FRAC_PI_2 && self.pitch < 3.0 * FRAC_PI_2
    }

    pub fn wrap(&mut self) {
        self.pitch = self.pitch.rem_euclid(TAU);
        self.yaw = self.yaw.rem_euclid(TAU);
    }
    pub fn clamp(&mut self) {
        self.zoom = self.zoom.clamp(self.near, self.far);
    }
}
impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: glm::Vec3::zeros(),
            pitch: 0.0,
            yaw: 0.0,
            zoom: 3.0,
            fov: FRAC_PI_3,
            near: 0.1,
            far: 100.0,
        }
    }
}
