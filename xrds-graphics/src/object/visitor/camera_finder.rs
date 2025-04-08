use std::{error::Error, fmt::Display};

use uuid::Uuid;

use crate::{AssetServer, Entity, RenderTargetType};

use super::Visitor;

#[derive(Debug)]
pub struct CameraFinder {
    camera_entity_ids: Vec<Uuid>,
}

#[derive(Debug)]
pub enum CameraFinderError {
    RenderTargetTypeMismatch {
        ty: RenderTargetType,
        expected: RenderTargetType,
    },
}

impl CameraFinder {
    pub fn new() -> Self {
        Self {
            camera_entity_ids: Vec::new(),
        }
    }

    pub fn camera_entity_ids(&self) -> &[Uuid] {
        &self.camera_entity_ids
    }

    fn visit_impl(&mut self, entity: &Entity) -> Result<(), CameraFinderError> {
        if let Some(camera_component) = entity.get_camera_component() {
            let cameras = camera_component.cameras();
            let render_target_type = camera_component.render_target_type();

            if cameras.len() > 1 && render_target_type != RenderTargetType::Texture2DArray {
                return Err(CameraFinderError::RenderTargetTypeMismatch {
                    ty: render_target_type,
                    expected: RenderTargetType::Texture2DArray,
                });
            }

            self.camera_entity_ids.push(*entity.id());
        }

        Ok(())
    }
}

impl Visitor<CameraFinderError> for CameraFinder {
    fn visit(
        &mut self,
        entity: &Entity,
        _asset_server: &AssetServer,
    ) -> Result<(), CameraFinderError> {
        self.visit_impl(entity)
    }
}

impl Error for CameraFinderError {}
impl Display for CameraFinderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}
