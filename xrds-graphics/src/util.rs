use ash::vk;

pub fn required_wgpu_features() -> wgpu::Features {
    wgpu::Features::MULTIVIEW
        | wgpu::Features::PUSH_CONSTANTS
        | wgpu::Features::PIPELINE_CACHE
        | wgpu::Features::BUFFER_BINDING_ARRAY
        | wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY
        | wgpu::Features::TEXTURE_BINDING_ARRAY
        | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
}

pub fn required_wgpu_memory_hints() -> wgpu::MemoryHints {
    #[cfg(target_os = "android")]
    let hints = wgpu::MemoryHints::MemoryUsage;
    #[cfg(not(target_os = "android"))]
    let hints = wgpu::MemoryHints::Performance;

    hints
}

pub fn required_wgpu_limits() -> wgpu::Limits {
    wgpu::Limits {
        max_push_constant_size: 64,
        max_color_attachment_bytes_per_sample: 64,
        max_sampled_textures_per_shader_stage: 64,
        ..Default::default()
    }
}

pub fn is_wgpu_supported_vk_format(vk_format: &vk::Format) -> bool {
    matches!(
        *vk_format,
        vk::Format::R8G8B8A8_UNORM
            | vk::Format::R8G8B8A8_SRGB
            | vk::Format::B8G8R8A8_UNORM
            | vk::Format::B8G8R8A8_SRGB
            | vk::Format::R16G16B16A16_SFLOAT
            | vk::Format::R32G32B32A32_SFLOAT
            | vk::Format::D32_SFLOAT
            | vk::Format::D32_SFLOAT_S8_UINT
            | vk::Format::D24_UNORM_S8_UINT
            | vk::Format::R8_UNORM
    )
}

pub fn wgpu_format_from_vk_format(format: vk::Format) -> anyhow::Result<wgpu::TextureFormat> {
    match format {
        vk::Format::R8G8B8A8_UNORM => Ok(wgpu::TextureFormat::Rgba8Unorm),
        vk::Format::R8G8B8A8_SRGB => Ok(wgpu::TextureFormat::Rgba8UnormSrgb),
        vk::Format::B8G8R8A8_UNORM => Ok(wgpu::TextureFormat::Bgra8Unorm),
        vk::Format::B8G8R8A8_SRGB => Ok(wgpu::TextureFormat::Bgra8UnormSrgb),
        vk::Format::R16G16B16A16_SFLOAT => Ok(wgpu::TextureFormat::Rgba16Float),
        vk::Format::R32G32B32A32_SFLOAT => Ok(wgpu::TextureFormat::Rgba32Float),
        vk::Format::D32_SFLOAT => Ok(wgpu::TextureFormat::Depth32Float),
        vk::Format::D32_SFLOAT_S8_UINT => Ok(wgpu::TextureFormat::Depth24Plus),
        vk::Format::D24_UNORM_S8_UINT => Ok(wgpu::TextureFormat::Depth24PlusStencil8),
        vk::Format::R8_UNORM => Ok(wgpu::TextureFormat::R8Unorm),
        _ => anyhow::bail!("Unsupported format"),
    }
}
