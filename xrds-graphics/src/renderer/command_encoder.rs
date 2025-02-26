#[derive(Debug)]
pub struct CommandEncoder {
    inner: wgpu::CommandEncoder,
}

impl CommandEncoder {
    pub fn new(inner: wgpu::CommandEncoder) -> Self {
        Self { inner }
    }

    pub fn encoder(&self) -> &wgpu::CommandEncoder {
        &self.inner
    }

    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        &mut self.inner
    }

    pub fn end(self) -> wgpu::CommandBuffer {
        self.inner.finish()
    }
}
