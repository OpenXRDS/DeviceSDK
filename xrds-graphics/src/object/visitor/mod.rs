mod camera_finder;
mod primitive_collector;

pub use camera_finder::*;
pub use primitive_collector::*;

use crate::AssetServer;

use super::Entity;

#[allow(unused)]
pub trait Visitor<E> {
    fn visit(&mut self, entity: &Entity, asset_server: &AssetServer) -> Result<(), E>;

    /// Implement only if you need actual mutable work
    fn visit_mut(&mut self, entity: &mut Entity, asset_server: &AssetServer) -> Result<(), E> {
        self.visit(entity, asset_server)
    }
}
