use super::XrdsBuffer;

#[derive(Debug, Clone, Copy)]
pub enum XrdsBufferViewType {
    Scalar(XrdsBufferViewComponentType),
    Vec2(XrdsBufferViewComponentType),
    Vec3(XrdsBufferViewComponentType),
    Vec4(XrdsBufferViewComponentType),
    Mat2(XrdsBufferViewComponentType),
    Mat3(XrdsBufferViewComponentType),
    Mat4(XrdsBufferViewComponentType),
}

#[derive(Debug, Clone, Copy)]
pub enum XrdsBufferViewComponentType {
    I8,
    U8,
    I16,
    U16,
    U32,
    F32,
}

#[derive(Debug, Clone)]
pub enum XrdsBufferViewComponent {
    ScalarI8(i8),
    ScalarU8(u8),
    ScalarI16(i16),
    ScalarU16(u16),
    ScalarU32(u32),
    ScalarF32(f32),
    Vec2I8(glam::I8Vec2),
    Vec2U8(glam::U8Vec2),
    Vec2I16(glam::I16Vec2),
    Vec2U16(glam::U16Vec2),
    Vec2U32(glam::UVec2),
    Vec2F32(glam::Vec2),
    Vec3I8(glam::I8Vec3),
    Vec3U8(glam::U8Vec3),
    Vec3I16(glam::I16Vec3),
    Vec3U16(glam::U16Vec3),
    Vec3U32(glam::UVec3),
    Vec3F32(glam::Vec3),
    Vec4I8(glam::I8Vec4),
    Vec4U8(glam::U8Vec4),
    Vec4I16(glam::I16Vec4),
    Vec4U16(glam::U16Vec4),
    Vec4U32(glam::UVec4),
    Vec4F32(glam::Vec4),
    Mat2F32(glam::Mat2),
    Mat3F32(glam::Mat3),
    Mat4F32(glam::Mat4),
}

#[derive(Debug, Clone)]
pub struct XrdsBufferViewMinMax {
    pub min: XrdsBufferViewComponent,
    pub max: XrdsBufferViewComponent,
}

#[derive(Debug, Clone)]
pub struct XrdsBufferView {
    buffer: XrdsBuffer,
    offset: usize,
    count: usize,
    ty: XrdsBufferViewType,
}

impl XrdsBufferView {
    pub fn new(buffer: XrdsBuffer, offset: usize, count: usize, ty: XrdsBufferViewType) -> Self {
        Self {
            buffer,
            offset,
            count,
            ty,
        }
    }

    pub fn buffer(&self) -> &XrdsBuffer {
        &self.buffer
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn ty(&self) -> XrdsBufferViewType {
        self.ty
    }
}

impl XrdsBufferViewType {
    pub fn size(&self) -> usize {
        match *self {
            Self::Scalar(ty) => ty.size(),
            Self::Vec2(ty) => ty.size() * 2,
            Self::Vec3(ty) => ty.size() * 3,
            Self::Vec4(ty) => ty.size() * 4,
            Self::Mat2(ty) => ty.size() * 4,
            Self::Mat3(ty) => ty.size() * 9,
            Self::Mat4(ty) => ty.size() * 16,
        }
    }
}

impl XrdsBufferViewComponentType {
    pub fn size(&self) -> usize {
        match *self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::U32 | Self::F32 => 4,
        }
    }
}
