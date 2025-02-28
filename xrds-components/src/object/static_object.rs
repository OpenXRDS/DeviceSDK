use xrds_core::{XrdsComponent, XrdsObject, XrdsWorldComponent, XrdsWorldComponentInner};

#[derive(Debug, Clone)]
pub struct StaticObject {
    name: String,
    world_component: XrdsWorldComponentInner,
}

impl StaticObject {}

impl XrdsWorldComponent for StaticObject {
    fn world_component(&self) -> &XrdsWorldComponentInner {
        &self.world_component
    }
    fn world_component_mut(&mut self) -> &mut XrdsWorldComponentInner {
        &mut self.world_component
    }
}

impl XrdsComponent for StaticObject {
    fn update(&mut self, elapsed: std::time::Duration) {}
    fn query_resources(&self) -> Vec<xrds_core::XrdsResource> {
        todo!()
    }
}

impl XrdsObject for StaticObject {
    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }
    fn on_construct(&self) {}
    fn on_destroy(&self) {}
}
