use std::sync::Arc;

use xrds_graphics::{RenderPass, XrdsObject};

#[derive(Debug, Clone)]
pub struct Object {
    pub inner: Arc<XrdsObject>,
}

impl Object {
    pub(crate) fn encode(&self, render_pass: &mut RenderPass) -> anyhow::Result<()> {
        self.inner.encode(render_pass);

        Ok(())
    }
}
