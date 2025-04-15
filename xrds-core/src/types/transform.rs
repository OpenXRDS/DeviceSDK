use std::fmt::Debug;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    translation: glam::Vec3,
    scale: glam::Vec3,
    rotation: glam::Quat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewDirection {
    eye: glam::Vec3,
    direction: glam::Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: glam::Vec3::ZERO,
            scale: glam::Vec3::ONE,
            rotation: glam::Quat::IDENTITY,
        }
    }
}

impl Default for ViewDirection {
    fn default() -> Self {
        Self {
            eye: glam::Vec3::ZERO,
            direction: glam::Vec3::NEG_Z,
        }
    }
}

impl Transform {
    pub fn with_translation(mut self, translation: glam::Vec3) -> Self {
        self.translation = translation;
        self
    }

    pub fn with_rotation(mut self, rotation: glam::Quat) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_scale(mut self, scale: glam::Vec3) -> Self {
        self.scale = scale;
        self
    }

    pub fn from_matrix(matrix: &glam::Mat4) -> Self {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Self {
            translation,
            rotation,
            scale,
        }
    }
    pub fn from_decomposed(
        translation: &glam::Vec3,
        rotation: &glam::Quat,
        scale: &glam::Vec3,
    ) -> Self {
        Self {
            translation: *translation,
            rotation: *rotation,
            scale: *scale,
        }
    }

    pub fn translate(&mut self, translation: glam::Vec3) {
        self.translation += translation;
    }

    pub fn scale(&mut self, scale: glam::Vec3) {
        self.scale *= scale;
    }

    pub fn rotate(&mut self, rotation: glam::Quat) {
        self.rotation *= rotation;
    }

    pub fn to_model_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn to_model_array(&self) -> [f32; 16] {
        self.to_model_matrix().to_cols_array()
    }

    pub fn get_translation(&self) -> glam::Vec3 {
        self.translation
    }

    pub fn get_rotation(&self) -> glam::Quat {
        self.rotation
    }

    pub fn get_scale(&self) -> glam::Vec3 {
        self.scale
    }
}

impl ViewDirection {
    pub fn with_eye(mut self, center: glam::Vec3) -> Self {
        self.eye = center;
        self
    }

    pub fn with_direction(mut self, direction: glam::Vec3) -> Self {
        self.direction = direction;
        self
    }

    pub fn from_matrix(matrix: &glam::Mat4) -> Self {
        let (_scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Self {
            eye: translation,
            direction: rotation.mul_vec3(glam::Vec3::Z),
        }
    }

    pub fn eye(&self) -> glam::Vec3 {
        self.eye
    }

    pub fn direction(&self) -> glam::Vec3 {
        self.direction
    }
}
