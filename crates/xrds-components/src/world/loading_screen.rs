use xrds_core::{XrdsWorld, XrdsWorldInner};

pub struct LoadingScreen {
    inner: XrdsWorldInner,
}

impl XrdsWorld for LoadingScreen {
    fn world(&self) -> &XrdsWorldInner {
        &self.inner
    }
    fn world_mut(&mut self) -> &mut XrdsWorldInner {
        &mut self.inner
    }
}
