use std::fmt::Debug;

#[derive(Debug, Clone, Copy)]
pub struct Rect2Di {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect2Df {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Size2Di {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Size2Df {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, PartialEq)]
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
}

impl Debug for Transform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{translation: {}, scale: {}, rotation: {}}}",
            self.translation, self.scale, self.rotation
        )
    }
}

impl From<Rect2Di> for Rect2Df {
    fn from(rect: Rect2Di) -> Self {
        Self {
            x: rect.x as f32,
            y: rect.y as f32,
            width: rect.width as f32,
            height: rect.height as f32,
        }
    }
}

impl From<Rect2Df> for Rect2Di {
    fn from(rect: Rect2Df) -> Self {
        Self {
            x: rect.x as i32,
            y: rect.y as i32,
            width: rect.width as u32,
            height: rect.height as u32,
        }
    }
}

impl From<Size2Di> for Size2Df {
    fn from(size: Size2Di) -> Self {
        Self {
            width: size.width as f32,
            height: size.height as f32,
        }
    }
}

impl From<Size2Df> for Size2Di {
    fn from(size: Size2Df) -> Self {
        Self {
            width: size.width as u32,
            height: size.height as u32,
        }
    }
}
