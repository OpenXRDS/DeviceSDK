mod format;
mod render_target;

pub use format::*;
pub use render_target::*;

#[derive(Debug, Clone)]
pub struct XrdsTexture {
    inner: wgpu::Texture,
    format: TextureFormat,
    size: wgpu::Extent3d,
    view: wgpu::TextureView,
}

impl XrdsTexture {
    pub fn new(
        inner: wgpu::Texture,
        format: TextureFormat,
        size: wgpu::Extent3d,
        view: wgpu::TextureView,
    ) -> Self {
        Self {
            inner,
            format,
            size,
            view,
        }
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.inner
    }

    pub fn format(&self) -> &TextureFormat {
        &self.format
    }

    pub fn size(&self) -> &wgpu::Extent3d {
        &self.size
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}
