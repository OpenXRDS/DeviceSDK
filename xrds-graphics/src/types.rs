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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    position: glam::Vec3,
    scale: glam::Vec3,
    rotation: glam::Quat,
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
