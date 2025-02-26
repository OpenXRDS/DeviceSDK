use ash::vk;

/// Texture formats supports by the engine.
/// This is a subset of the graphics library's texture formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureFormat(wgpu::TextureFormat);

impl TextureFormat {
    pub fn as_wgpu(&self) -> wgpu::TextureFormat {
        self.0
    }

    pub fn as_vk(&self) -> vk::Format {
        match self.0 {
            wgpu::TextureFormat::R8Unorm => vk::Format::R8_UNORM,
            wgpu::TextureFormat::R8Snorm => vk::Format::R8_SNORM,
            wgpu::TextureFormat::R8Uint => vk::Format::R8_UINT,
            wgpu::TextureFormat::R8Sint => vk::Format::R8_SINT,
            wgpu::TextureFormat::Rg8Unorm => vk::Format::R8G8_UNORM,
            wgpu::TextureFormat::Rg8Snorm => vk::Format::R8G8_SNORM,
            wgpu::TextureFormat::Rg8Uint => vk::Format::R8G8_UINT,
            wgpu::TextureFormat::Rg8Sint => vk::Format::R8G8_SINT,
            wgpu::TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
            wgpu::TextureFormat::Rgba8Snorm => vk::Format::R8G8B8A8_SNORM,
            wgpu::TextureFormat::Rgba8Uint => vk::Format::R8G8B8A8_UINT,
            wgpu::TextureFormat::Rgba8Sint => vk::Format::R8G8B8A8_SINT,
            wgpu::TextureFormat::Rgba8UnormSrgb => vk::Format::R8G8B8A8_SRGB,
            wgpu::TextureFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
            wgpu::TextureFormat::Bgra8UnormSrgb => vk::Format::B8G8R8A8_SRGB,
            wgpu::TextureFormat::R16Unorm => vk::Format::R16_UNORM,
            wgpu::TextureFormat::R16Snorm => vk::Format::R16_SNORM,
            wgpu::TextureFormat::R16Uint => vk::Format::R16_UINT,
            wgpu::TextureFormat::R16Sint => vk::Format::R16_SINT,
            wgpu::TextureFormat::R16Float => vk::Format::R16_SFLOAT,
            wgpu::TextureFormat::Rg16Unorm => vk::Format::R16G16_UNORM,
            wgpu::TextureFormat::Rg16Snorm => vk::Format::R16G16_SNORM,
            wgpu::TextureFormat::Rg16Uint => vk::Format::R16G16_UINT,
            wgpu::TextureFormat::Rg16Sint => vk::Format::R16G16_SINT,
            wgpu::TextureFormat::Rg16Float => vk::Format::R16G16_SFLOAT,
            wgpu::TextureFormat::Rgba16Unorm => vk::Format::R16G16B16A16_UNORM,
            wgpu::TextureFormat::Rgba16Snorm => vk::Format::R16G16B16A16_SNORM,
            wgpu::TextureFormat::Rgba16Uint => vk::Format::R16G16B16A16_UINT,
            wgpu::TextureFormat::Rgba16Sint => vk::Format::R16G16B16A16_SINT,
            wgpu::TextureFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
            wgpu::TextureFormat::R32Uint => vk::Format::R32_UINT,
            wgpu::TextureFormat::R32Sint => vk::Format::R32_SINT,
            wgpu::TextureFormat::R32Float => vk::Format::R32_SFLOAT,
            wgpu::TextureFormat::Rg32Uint => vk::Format::R32G32_UINT,
            wgpu::TextureFormat::Rg32Sint => vk::Format::R32G32_SINT,
            wgpu::TextureFormat::Rg32Float => vk::Format::R32G32_SFLOAT,
            wgpu::TextureFormat::Rgba32Uint => vk::Format::R32G32B32A32_UINT,
            wgpu::TextureFormat::Rgba32Sint => vk::Format::R32G32B32A32_SINT,
            wgpu::TextureFormat::Rgba32Float => vk::Format::R32G32B32A32_SFLOAT,
            wgpu::TextureFormat::R64Uint => vk::Format::R64_UINT,
            wgpu::TextureFormat::Depth16Unorm => vk::Format::D16_UNORM,
            wgpu::TextureFormat::Depth32Float => vk::Format::D32_SFLOAT,
            wgpu::TextureFormat::Stencil8 => vk::Format::S8_UINT,
            wgpu::TextureFormat::Depth24PlusStencil8 => vk::Format::D24_UNORM_S8_UINT,
            wgpu::TextureFormat::Depth32FloatStencil8 => vk::Format::D32_SFLOAT_S8_UINT,
            wgpu::TextureFormat::Bc1RgbaUnorm => vk::Format::BC1_RGBA_UNORM_BLOCK,
            wgpu::TextureFormat::Bc1RgbaUnormSrgb => vk::Format::BC1_RGBA_SRGB_BLOCK,
            wgpu::TextureFormat::Bc2RgbaUnorm => vk::Format::BC2_UNORM_BLOCK,
            wgpu::TextureFormat::Bc2RgbaUnormSrgb => vk::Format::BC2_SRGB_BLOCK,
            wgpu::TextureFormat::Bc3RgbaUnorm => vk::Format::BC3_UNORM_BLOCK,
            wgpu::TextureFormat::Bc3RgbaUnormSrgb => vk::Format::BC3_SRGB_BLOCK,
            wgpu::TextureFormat::Bc4RUnorm => vk::Format::BC4_UNORM_BLOCK,
            wgpu::TextureFormat::Bc4RSnorm => vk::Format::BC4_SNORM_BLOCK,
            wgpu::TextureFormat::Bc5RgUnorm => vk::Format::BC5_UNORM_BLOCK,
            wgpu::TextureFormat::Bc5RgSnorm => vk::Format::BC5_SNORM_BLOCK,
            wgpu::TextureFormat::Bc6hRgbUfloat => vk::Format::BC6H_UFLOAT_BLOCK,
            wgpu::TextureFormat::Bc6hRgbFloat => vk::Format::BC6H_SFLOAT_BLOCK,
            wgpu::TextureFormat::Bc7RgbaUnorm => vk::Format::BC7_UNORM_BLOCK,
            wgpu::TextureFormat::Bc7RgbaUnormSrgb => vk::Format::BC7_SRGB_BLOCK,
            wgpu::TextureFormat::Etc2Rgb8Unorm => vk::Format::ETC2_R8G8B8_UNORM_BLOCK,
            wgpu::TextureFormat::Etc2Rgb8UnormSrgb => vk::Format::ETC2_R8G8B8_SRGB_BLOCK,
            wgpu::TextureFormat::Etc2Rgb8A1Unorm => vk::Format::ETC2_R8G8B8A1_UNORM_BLOCK,
            wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb => vk::Format::ETC2_R8G8B8A1_SRGB_BLOCK,
            wgpu::TextureFormat::Etc2Rgba8Unorm => vk::Format::ETC2_R8G8B8A8_UNORM_BLOCK,
            wgpu::TextureFormat::Etc2Rgba8UnormSrgb => vk::Format::ETC2_R8G8B8A8_SRGB_BLOCK,
            wgpu::TextureFormat::EacR11Unorm => vk::Format::EAC_R11_UNORM_BLOCK,
            wgpu::TextureFormat::EacR11Snorm => vk::Format::EAC_R11_SNORM_BLOCK,
            wgpu::TextureFormat::EacRg11Unorm => vk::Format::EAC_R11G11_UNORM_BLOCK,
            wgpu::TextureFormat::EacRg11Snorm => vk::Format::EAC_R11G11_SNORM_BLOCK,
            wgpu::TextureFormat::Astc { block, channel } => match block {
                wgpu::AstcBlock::B4x4 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_4X4_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_4X4_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B5x4 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_5X4_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_5X4_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B5x5 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_5X5_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_5X5_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B6x5 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_6X5_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_6X5_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B6x6 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_6X6_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_6X6_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B8x5 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_8X5_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_8X5_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B8x6 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_8X6_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_8X6_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B8x8 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_8X8_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_8X8_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B10x8 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_10X8_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_10X8_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B10x5 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_10X5_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_10X5_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B10x6 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_10X6_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_10X6_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B10x10 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_10X10_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_10X10_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B12x10 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_12X10_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_12X10_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
                wgpu::AstcBlock::B12x12 => match channel {
                    wgpu::AstcChannel::Unorm => vk::Format::ASTC_12X12_UNORM_BLOCK,
                    wgpu::AstcChannel::UnormSrgb => vk::Format::ASTC_12X12_SRGB_BLOCK,
                    wgpu::AstcChannel::Hdr => panic!("ASTC HDR format not supported"),
                },
            },
            _ => panic!("Unsupported texture format: {:?}", self.0),
        }
    }
}

impl From<wgpu::TextureFormat> for TextureFormat {
    fn from(fmt: wgpu::TextureFormat) -> Self {
        Self(fmt)
    }
}

impl TryFrom<vk::Format> for TextureFormat {
    type Error = String;

    fn try_from(value: vk::Format) -> Result<Self, Self::Error> {
        let fmt = match value {
            vk::Format::R8_UNORM => wgpu::TextureFormat::R8Unorm,
            vk::Format::R8_SNORM => wgpu::TextureFormat::R8Snorm,
            vk::Format::R8_UINT => wgpu::TextureFormat::R8Uint,
            vk::Format::R8_SINT => wgpu::TextureFormat::R8Sint,
            vk::Format::R8G8_UNORM => wgpu::TextureFormat::Rg8Unorm,
            vk::Format::R8G8_SNORM => wgpu::TextureFormat::Rg8Snorm,
            vk::Format::R8G8_UINT => wgpu::TextureFormat::Rg8Uint,
            vk::Format::R8G8_SINT => wgpu::TextureFormat::Rg8Sint,
            vk::Format::R8G8B8A8_UNORM => wgpu::TextureFormat::Rgba8Unorm,
            vk::Format::R8G8B8A8_SNORM => wgpu::TextureFormat::Rgba8Snorm,
            vk::Format::R8G8B8A8_UINT => wgpu::TextureFormat::Rgba8Uint,
            vk::Format::R8G8B8A8_SINT => wgpu::TextureFormat::Rgba8Sint,
            vk::Format::R8G8B8A8_SRGB => wgpu::TextureFormat::Rgba8UnormSrgb,
            vk::Format::B8G8R8A8_UNORM => wgpu::TextureFormat::Bgra8Unorm,
            vk::Format::B8G8R8A8_SRGB => wgpu::TextureFormat::Bgra8UnormSrgb,
            vk::Format::R16_UNORM => wgpu::TextureFormat::R16Unorm,
            vk::Format::R16_SNORM => wgpu::TextureFormat::R16Snorm,
            vk::Format::R16_UINT => wgpu::TextureFormat::R16Uint,
            vk::Format::R16_SINT => wgpu::TextureFormat::R16Sint,
            vk::Format::R16_SFLOAT => wgpu::TextureFormat::R16Float,
            vk::Format::R16G16_UNORM => wgpu::TextureFormat::Rg16Unorm,
            vk::Format::R16G16_SNORM => wgpu::TextureFormat::Rg16Snorm,
            vk::Format::R16G16_UINT => wgpu::TextureFormat::Rg16Uint,
            vk::Format::R16G16_SINT => wgpu::TextureFormat::Rg16Sint,
            vk::Format::R16G16_SFLOAT => wgpu::TextureFormat::Rg16Float,
            vk::Format::R16G16B16A16_UNORM => wgpu::TextureFormat::Rgba16Unorm,
            vk::Format::R16G16B16A16_SNORM => wgpu::TextureFormat::Rgba16Snorm,
            vk::Format::R16G16B16A16_UINT => wgpu::TextureFormat::Rgba16Uint,
            vk::Format::R16G16B16A16_SINT => wgpu::TextureFormat::Rgba16Sint,
            vk::Format::R16G16B16A16_SFLOAT => wgpu::TextureFormat::Rgba16Float,
            vk::Format::R32_UINT => wgpu::TextureFormat::R32Uint,
            vk::Format::R32_SINT => wgpu::TextureFormat::R32Sint,
            vk::Format::R32_SFLOAT => wgpu::TextureFormat::R32Float,
            vk::Format::R32G32_UINT => wgpu::TextureFormat::Rg32Uint,
            vk::Format::R32G32_SINT => wgpu::TextureFormat::Rg32Sint,
            vk::Format::R32G32_SFLOAT => wgpu::TextureFormat::Rg32Float,
            vk::Format::R32G32B32A32_UINT => wgpu::TextureFormat::Rgba32Uint,
            vk::Format::R32G32B32A32_SINT => wgpu::TextureFormat::Rgba32Sint,
            vk::Format::R32G32B32A32_SFLOAT => wgpu::TextureFormat::Rgba32Float,
            vk::Format::R64_UINT => wgpu::TextureFormat::R64Uint,
            vk::Format::D16_UNORM => wgpu::TextureFormat::Depth16Unorm,
            vk::Format::D32_SFLOAT => wgpu::TextureFormat::Depth32Float,
            vk::Format::S8_UINT => wgpu::TextureFormat::Stencil8,
            vk::Format::D24_UNORM_S8_UINT => wgpu::TextureFormat::Depth24PlusStencil8,
            vk::Format::D32_SFLOAT_S8_UINT => wgpu::TextureFormat::Depth32FloatStencil8,
            vk::Format::BC1_RGBA_UNORM_BLOCK => wgpu::TextureFormat::Bc1RgbaUnorm,
            vk::Format::BC1_RGBA_SRGB_BLOCK => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
            vk::Format::BC2_UNORM_BLOCK => wgpu::TextureFormat::Bc2RgbaUnorm,
            vk::Format::BC2_SRGB_BLOCK => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
            vk::Format::BC3_UNORM_BLOCK => wgpu::TextureFormat::Bc3RgbaUnorm,
            vk::Format::BC3_SRGB_BLOCK => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
            vk::Format::BC4_UNORM_BLOCK => wgpu::TextureFormat::Bc4RUnorm,
            vk::Format::BC4_SNORM_BLOCK => wgpu::TextureFormat::Bc4RSnorm,
            vk::Format::BC5_UNORM_BLOCK => wgpu::TextureFormat::Bc5RgUnorm,
            vk::Format::BC5_SNORM_BLOCK => wgpu::TextureFormat::Bc5RgSnorm,
            vk::Format::BC6H_UFLOAT_BLOCK => wgpu::TextureFormat::Bc6hRgbUfloat,
            vk::Format::BC6H_SFLOAT_BLOCK => wgpu::TextureFormat::Bc6hRgbFloat,
            vk::Format::BC7_UNORM_BLOCK => wgpu::TextureFormat::Bc7RgbaUnorm,
            vk::Format::BC7_SRGB_BLOCK => wgpu::TextureFormat::Bc7RgbaUnormSrgb,
            vk::Format::ETC2_R8G8B8_UNORM_BLOCK => wgpu::TextureFormat::Etc2Rgb8Unorm,
            vk::Format::ETC2_R8G8B8_SRGB_BLOCK => wgpu::TextureFormat::Etc2Rgb8UnormSrgb,
            vk::Format::ETC2_R8G8B8A1_UNORM_BLOCK => wgpu::TextureFormat::Etc2Rgb8A1Unorm,
            vk::Format::ETC2_R8G8B8A1_SRGB_BLOCK => wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb,
            vk::Format::ETC2_R8G8B8A8_UNORM_BLOCK => wgpu::TextureFormat::Etc2Rgba8Unorm,
            vk::Format::ETC2_R8G8B8A8_SRGB_BLOCK => wgpu::TextureFormat::Etc2Rgba8UnormSrgb,
            vk::Format::EAC_R11_UNORM_BLOCK => wgpu::TextureFormat::EacR11Unorm,
            vk::Format::EAC_R11_SNORM_BLOCK => wgpu::TextureFormat::EacR11Snorm,
            vk::Format::EAC_R11G11_UNORM_BLOCK => wgpu::TextureFormat::EacRg11Unorm,
            vk::Format::EAC_R11G11_SNORM_BLOCK => wgpu::TextureFormat::EacRg11Snorm,
            vk::Format::ASTC_4X4_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B4x4,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_4X4_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B4x4,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_5X4_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B5x4,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_5X4_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B5x4,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_5X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B5x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_5X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B5x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_6X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B6x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_6X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B6x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_6X6_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B6x6,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_6X6_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B6x6,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_8X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B8x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_8X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B8x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_8X6_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B8x6,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_8X6_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B8x6,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_8X8_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B8x8,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_8X8_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B8x8,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_10X5_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x5,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_10X5_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x5,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_10X6_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x6,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_10X6_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x6,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_10X8_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x8,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_10X8_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x8,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_10X10_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x10,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_10X10_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B10x10,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_12X10_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B12x10,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_12X10_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B12x10,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            vk::Format::ASTC_12X12_UNORM_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B12x12,
                channel: wgpu::AstcChannel::Unorm,
            },
            vk::Format::ASTC_12X12_SRGB_BLOCK => wgpu::TextureFormat::Astc {
                block: wgpu::AstcBlock::B12x12,
                channel: wgpu::AstcChannel::UnormSrgb,
            },

            vk::Format::R4G4_UNORM_PACK8
            | vk::Format::R4G4B4A4_UNORM_PACK16
            | vk::Format::B4G4R4A4_UNORM_PACK16
            | vk::Format::R5G6B5_UNORM_PACK16
            | vk::Format::B5G6R5_UNORM_PACK16
            | vk::Format::R5G5B5A1_UNORM_PACK16
            | vk::Format::B5G5R5A1_UNORM_PACK16
            | vk::Format::A1R5G5B5_UNORM_PACK16
            | vk::Format::R8_USCALED
            | vk::Format::R8_SSCALED
            | vk::Format::R8_SRGB
            | vk::Format::R8G8_USCALED
            | vk::Format::R8G8_SSCALED
            | vk::Format::R8G8_SRGB
            | vk::Format::R8G8B8_UNORM
            | vk::Format::R8G8B8_SNORM
            | vk::Format::R8G8B8_USCALED
            | vk::Format::R8G8B8_SSCALED
            | vk::Format::R8G8B8_UINT
            | vk::Format::R8G8B8_SINT
            | vk::Format::R8G8B8_SRGB
            | vk::Format::B8G8R8_UNORM
            | vk::Format::B8G8R8_SNORM
            | vk::Format::B8G8R8_USCALED
            | vk::Format::B8G8R8_SSCALED
            | vk::Format::B8G8R8_UINT
            | vk::Format::B8G8R8_SINT
            | vk::Format::B8G8R8_SRGB
            | vk::Format::R8G8B8A8_USCALED
            | vk::Format::R8G8B8A8_SSCALED
            | vk::Format::B8G8R8A8_SNORM
            | vk::Format::B8G8R8A8_USCALED
            | vk::Format::B8G8R8A8_SSCALED
            | vk::Format::B8G8R8A8_UINT
            | vk::Format::B8G8R8A8_SINT
            | vk::Format::A8B8G8R8_UNORM_PACK32
            | vk::Format::A8B8G8R8_SNORM_PACK32
            | vk::Format::A8B8G8R8_USCALED_PACK32
            | vk::Format::A8B8G8R8_SSCALED_PACK32
            | vk::Format::A8B8G8R8_UINT_PACK32
            | vk::Format::A8B8G8R8_SINT_PACK32
            | vk::Format::A8B8G8R8_SRGB_PACK32
            | vk::Format::A2R10G10B10_UNORM_PACK32
            | vk::Format::A2R10G10B10_SNORM_PACK32
            | vk::Format::A2R10G10B10_USCALED_PACK32
            | vk::Format::A2R10G10B10_SSCALED_PACK32
            | vk::Format::A2R10G10B10_UINT_PACK32
            | vk::Format::A2R10G10B10_SINT_PACK32
            | vk::Format::A2B10G10R10_UNORM_PACK32
            | vk::Format::A2B10G10R10_SNORM_PACK32
            | vk::Format::A2B10G10R10_USCALED_PACK32
            | vk::Format::A2B10G10R10_SSCALED_PACK32
            | vk::Format::A2B10G10R10_UINT_PACK32
            | vk::Format::A2B10G10R10_SINT_PACK32
            | vk::Format::R16_USCALED
            | vk::Format::R16_SSCALED
            | vk::Format::R16G16_USCALED
            | vk::Format::R16G16_SSCALED
            | vk::Format::R16G16B16_UNORM
            | vk::Format::R16G16B16_SNORM
            | vk::Format::R16G16B16_USCALED
            | vk::Format::R16G16B16_SSCALED
            | vk::Format::R16G16B16_UINT
            | vk::Format::R16G16B16_SINT
            | vk::Format::R16G16B16_SFLOAT
            | vk::Format::R16G16B16A16_USCALED
            | vk::Format::R16G16B16A16_SSCALED
            | vk::Format::R32G32B32_UINT
            | vk::Format::R32G32B32_SINT
            | vk::Format::R32G32B32_SFLOAT
            | vk::Format::R64_SINT
            | vk::Format::R64_SFLOAT
            | vk::Format::R64G64_UINT
            | vk::Format::R64G64_SINT
            | vk::Format::R64G64_SFLOAT
            | vk::Format::R64G64B64_UINT
            | vk::Format::R64G64B64_SINT
            | vk::Format::R64G64B64_SFLOAT
            | vk::Format::R64G64B64A64_UINT
            | vk::Format::R64G64B64A64_SINT
            | vk::Format::R64G64B64A64_SFLOAT
            | vk::Format::B10G11R11_UFLOAT_PACK32
            | vk::Format::E5B9G9R9_UFLOAT_PACK32
            | vk::Format::X8_D24_UNORM_PACK32
            | vk::Format::D16_UNORM_S8_UINT
            | vk::Format::BC1_RGB_UNORM_BLOCK
            | vk::Format::BC1_RGB_SRGB_BLOCK
            | vk::Format::UNDEFINED
            | _ => {
                return Err(format!(
                    "Could not convert ash::vk::Format({:?}) to TextureFormat",
                    value,
                ))
            }
        };
        Ok(Self(fmt))
    }
}
