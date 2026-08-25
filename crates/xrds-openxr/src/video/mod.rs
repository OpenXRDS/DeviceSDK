//! Android video frames as wgpu textures.
//!
//! Route B of `docs/video-asset-spike.md`. `xrds-media` decodes into GPU-resident
//! `AHardwareBuffer`s; this turns one into a `wgpu::Texture` a material can sample.
//!
//! ```ignore
//! let mut video = AndroidVideoPipeline::new(render_device.wgpu_device())?;
//! // once per frame, with a buffer from xrds-media's HardwareVideoDecoder:
//! let texture = video.frame_to_texture(buffer, width, height)?;
//! ```
//!
//! # Why this is not just "import the buffer"
//!
//! Measured on a Quest 3: the decoder's buffer is vendor-tiled (`0x7fa30c06`, in
//! Qualcomm's range) and Vulkan describes it only by *external format* —
//! `vkGetAndroidHardwareBufferPropertiesANDROID` reports `VK_FORMAT_UNDEFINED`.
//! Sampling such an image requires a `VkSamplerYcbcrConversion` built from that
//! external format id, baked into the descriptor set layout as an **immutable**
//! sampler.
//!
//! wgpu can express neither: `create_texture_from_hal` takes a `wgpu::TextureFormat`
//! and there is no external variant, and `BindGroupLayoutEntry` has no
//! immutable-sampler concept. Asking the decoder for a known layout instead does not
//! help — `AIMAGE_FORMAT_YUV_420_888` is accepted, sustains full frame rate, and
//! still yields a vendor-tiled buffer. That was route A, and eliminating it is why
//! this exists.
//!
//! So the work splits in two: [`import`] makes the buffer samplable *by Vulkan*, and
//! [`convert`] runs one fullscreen pass writing ordinary RGBA that wgpu can own.
//! The cost is one full-frame write per frame, which is the price of the whole
//! approach and is measured rather than assumed.
//!
//! # Provenance
//!
//! The import and barrier sequences follow `HMDViewer`'s `vk_renderer.rs`, which has
//! run them on a Quest 3 in production.

mod convert;
mod import;

use anyhow::{anyhow, Result};
use ash::vk;

/// Decoded frames in, wgpu textures out.
///
/// The texture is created once and rewritten in place every frame — the handle a
/// material binds stays valid for the life of the pipeline, which is what lets this
/// sit behind the same "runtime-owned texture" idea the desktop path uses.
pub struct AndroidVideoPipeline {
    importer: import::VideoImporter,
    converter: Option<convert::VideoConverter>,
    texture: Option<wgpu::Texture>,
    device: wgpu::Device,
    size: (u32, u32),
}

impl AndroidVideoPipeline {
    /// Build against the renderer's own Vulkan device.
    ///
    /// It must be that device and no other: the images live on it, and a Vulkan
    /// image cannot cross devices.
    pub fn new(device: &wgpu::Device) -> Result<Self> {
        Ok(Self {
            importer: import::VideoImporter::new(device)?,
            converter: None,
            texture: None,
            device: device.clone(),
            size: (0, 0),
        })
    }

    /// Import `buffer` and convert it, returning the texture holding the result.
    ///
    /// The same texture is returned every call; only its contents change.
    pub fn frame_to_texture(
        &mut self,
        buffer: *mut vk::AHardwareBuffer,
        width: u32,
        height: u32,
    ) -> Result<&wgpu::Texture> {
        if self.size != (0, 0) && self.size != (width, height) {
            return Err(anyhow!(
                "video size changed mid-stream: {:?} -> {:?}",
                self.size,
                (width, height)
            ));
        }

        let (view, image) = {
            let frame = self.importer.import(buffer, width, height)?;
            (frame.view, frame.image)
        };

        if self.converter.is_none() {
            // Deferred until here because the converter's descriptor layout embeds
            // the importer's sampler, which does not exist until a buffer has
            // arrived and its external format is known.
            let sampler = self
                .importer
                .sampler()
                .ok_or_else(|| anyhow!("importer produced no sampler"))?;
            let (converter, texture) =
                convert::VideoConverter::new(&self.device, sampler, width, height)?;
            self.converter = Some(converter);
            self.texture = Some(texture);
            self.size = (width, height);
            log::info!("video pipeline ready: {width}x{height} -> Rgba8UnormSrgb");
        }

        self.converter
            .as_mut()
            .expect("created just above")
            .convert(view, image)?;

        self.texture
            .as_ref()
            .ok_or_else(|| anyhow!("no output texture"))
    }

    /// The output texture, if a frame has been converted.
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        self.texture.as_ref()
    }
}
