use wgpu::{SurfaceError, SurfaceTexture};

#[derive(Debug)]
pub struct Surface<'window> {
    inner: wgpu::Surface<'window>,
}

impl<'a> Surface<'a> {
    pub fn new(surface: wgpu::Surface<'a>) -> Self {
        Self { inner: surface }
    }
    pub fn get_current_texture(&self) -> Result<SurfaceTexture, SurfaceError> {
        self.inner.get_current_texture()
    }
}
