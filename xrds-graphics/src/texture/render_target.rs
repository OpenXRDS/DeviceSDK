use super::XrdsTexture;

#[derive(Debug, Clone)]
pub enum RenderTargetOps {
    ColorAttachment(wgpu::Operations<wgpu::Color>),
    DepthStencilAttachment {
        depth_ops: Option<wgpu::Operations<f32>>,
        stencil_ops: Option<wgpu::Operations<u32>>,
    },
}

#[derive(Debug, Clone)]
pub struct RenderTargetTexture {
    inner: XrdsTexture,
    ops: RenderTargetOps,
}

impl RenderTargetTexture {
    pub fn new(inner: XrdsTexture, ops: RenderTargetOps) -> Self {
        Self { inner, ops }
    }

    pub fn texture(&self) -> &XrdsTexture {
        &self.inner
    }

    pub fn ops(&self) -> &RenderTargetOps {
        &self.ops
    }

    pub fn is_color_target(&self) -> bool {
        match self.ops {
            RenderTargetOps::ColorAttachment(_) => true,
            RenderTargetOps::DepthStencilAttachment {
                depth_ops: _,
                stencil_ops: _,
            } => false,
        }
    }

    pub fn is_depth_stencil_target(&self) -> bool {
        match self.ops {
            RenderTargetOps::ColorAttachment(_) => false,
            RenderTargetOps::DepthStencilAttachment {
                depth_ops: _,
                stencil_ops: _,
            } => true,
        }
    }

    pub fn is_depth_target(&self) -> bool {
        match self.ops {
            RenderTargetOps::ColorAttachment(_) => false,
            RenderTargetOps::DepthStencilAttachment {
                depth_ops,
                stencil_ops: _,
            } => depth_ops.is_some(),
        }
    }

    pub fn is_stencil_target(&self) -> bool {
        match self.ops {
            RenderTargetOps::ColorAttachment(_) => false,
            RenderTargetOps::DepthStencilAttachment {
                depth_ops: _,
                stencil_ops,
            } => stencil_ops.is_some(),
        }
    }

    pub fn as_color_operation(&self) -> anyhow::Result<wgpu::Operations<wgpu::Color>> {
        if let RenderTargetOps::ColorAttachment(c) = self.ops {
            Ok(c)
        } else {
            anyhow::bail!("Render target is not color target")
        }
    }

    pub fn as_depth_operation(&self) -> anyhow::Result<Option<wgpu::Operations<f32>>> {
        if let RenderTargetOps::DepthStencilAttachment {
            depth_ops,
            stencil_ops: _,
        } = self.ops
        {
            Ok(depth_ops)
        } else {
            Ok(None)
        }
    }

    pub fn as_stencil_operation(&self) -> anyhow::Result<Option<wgpu::Operations<u32>>> {
        if let RenderTargetOps::DepthStencilAttachment {
            depth_ops: _,
            stencil_ops,
        } = self.ops
        {
            Ok(stencil_ops)
        } else {
            Ok(None)
        }
    }
}
