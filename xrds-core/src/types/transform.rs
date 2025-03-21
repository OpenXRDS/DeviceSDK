use std::fmt::Debug;

#[derive(Debug, Clone, Copy)]
pub struct Rect2D<T> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
}

#[derive(Debug, Clone, Copy)]
pub struct Size2D<T> {
    pub width: T,
    pub height: T,
}

pub type Rect2Di = Rect2D<i32>;
pub type Rect2Du = Rect2D<u32>;
pub type Rect2Df = Rect2D<f32>;
pub type Rect2Dd = Rect2D<f64>;
pub type Size2Di = Size2D<i32>;
pub type Size2Du = Size2D<u32>;
pub type Size2Df = Size2D<f32>;
pub type Size2Dd = Size2D<f64>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    translation: glam::Vec3,
    scale: glam::Vec3,
    rotation: glam::Quat,
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
