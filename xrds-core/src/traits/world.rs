use std::time::Duration;

use crate::{XrdsResource, XrdsWorldInner};

pub trait XrdsWorld {
    fn world(&self) -> &XrdsWorldInner;
    fn world_mut(&mut self) -> &mut XrdsWorldInner;

    fn update(&mut self, elapsed: Duration) {
        self.world_mut().update(elapsed);
    }
    fn query_resources(&self) -> Vec<XrdsResource> {
        self.world().query_resources()
    }
}
