use crate::RenderTargetTexture;

#[derive(Debug, Clone)]
pub struct Framebuffer {
    color_attachments: Vec<RenderTargetTexture>,
    depth_stencil_attachment: Option<RenderTargetTexture>,
}

impl Framebuffer {
    pub fn new(
        color_attachments: &[RenderTargetTexture],
        depth_stencil_attachment: Option<RenderTargetTexture>,
    ) -> Self {
        Self {
            color_attachments: color_attachments.to_vec(),
            depth_stencil_attachment,
        }
    }

    pub fn color_textures(&self) -> Vec<&RenderTargetTexture> {
        self.color_attachments.iter().collect()
    }

    pub fn depth_stencil_texture(&self) -> Option<&RenderTargetTexture> {
        self.depth_stencil_attachment.as_ref()
    }

    pub fn color_attachments(
        &self,
    ) -> anyhow::Result<Vec<Option<wgpu::RenderPassColorAttachment>>> {
        let result: Vec<_> = self
            .color_attachments
            .iter()
            .map(|target| {
                if target.texture().view().is_some() && target.is_color_target() {
                    Some(wgpu::RenderPassColorAttachment {
                        view: target.texture().view().unwrap(),
                        ops: target.as_color_operation().unwrap(),
                        resolve_target: None,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(result)
    }

    pub fn depth_stencil_attachment(
        &self,
    ) -> anyhow::Result<Option<wgpu::RenderPassDepthStencilAttachment>> {
        let res = if let Some(ds) = self.depth_stencil_attachment.as_ref() {
            if ds.texture().view().is_some() && ds.is_depth_stencil_target() {
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: ds.texture().view().unwrap(),
                    depth_ops: ds.as_depth_operation().unwrap(),
                    stencil_ops: ds.as_stencil_operation().unwrap(),
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(res)
    }
}
