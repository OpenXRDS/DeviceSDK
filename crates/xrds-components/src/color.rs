use bevy::prelude::*;

/// SDK-level color type to avoid exposing Bevy `Color` in public XRDS descriptors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsColor {
    pub rgba: [f32; 4],
}

impl XrdsColor {
    pub const WHITE: Self = Self {
        rgba: [1.0, 1.0, 1.0, 1.0],
    };

    pub const BLACK: Self = Self {
        rgba: [0.0, 0.0, 0.0, 1.0],
    };

    pub const fn srgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { rgba: [r, g, b, a] }
    }

    pub const fn srgb(r: f32, g: f32, b: f32) -> Self {
        Self::srgba(r, g, b, 1.0)
    }
}

impl Default for XrdsColor {
    fn default() -> Self {
        Self::WHITE
    }
}

impl From<XrdsColor> for Color {
    fn from(value: XrdsColor) -> Self {
        Color::srgba(value.rgba[0], value.rgba[1], value.rgba[2], value.rgba[3])
    }
}

impl From<Color> for XrdsColor {
    fn from(value: Color) -> Self {
        let srgba = value.to_srgba();
        Self::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha)
    }
}

/// SDK-level HDR linear color for physically-based material properties such as emissive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsLinearRgba {
    pub rgba: [f32; 4],
}

impl XrdsLinearRgba {
    pub const BLACK: Self = Self {
        rgba: [0.0, 0.0, 0.0, 1.0],
    };

    pub const WHITE: Self = Self {
        rgba: [1.0, 1.0, 1.0, 1.0],
    };

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { rgba: [r, g, b, a] }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }
}

impl Default for XrdsLinearRgba {
    fn default() -> Self {
        Self::BLACK
    }
}

impl From<XrdsLinearRgba> for LinearRgba {
    fn from(value: XrdsLinearRgba) -> Self {
        LinearRgba::new(value.rgba[0], value.rgba[1], value.rgba[2], value.rgba[3])
    }
}

impl From<LinearRgba> for XrdsLinearRgba {
    fn from(value: LinearRgba) -> Self {
        Self::rgba(value.red, value.green, value.blue, value.alpha)
    }
}
