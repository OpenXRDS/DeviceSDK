mod camera;
mod light;
mod mesh;
mod transform;

pub use camera::*;
pub use light::*;
pub use mesh::*;
pub use transform::*;

use std::fmt::Debug;
use uuid::Uuid;

use crate::AssetServer;

use super::Visitor;

#[derive(Debug, Clone)]
pub enum Component {
    Mesh(MeshComponent),
    Light(LightComponent),
    Transform(TransformComponent),
    Camera(CameraComponent),
}

#[derive(Debug, Default, Clone)]
pub struct Entity {
    pub id: Uuid,
    pub name: String,
    // TODO: using trait for dynamic component
    pub mesh_component: Option<MeshComponent>,
    pub light_component: Option<LightComponent>,
    pub transform_component: Option<TransformComponent>,
    pub camera_component: Option<CameraComponent>,
}

impl Entity {
    pub fn new(id: Uuid, name: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
            ..Default::default()
        }
    }

    pub fn with_id(mut self, id: &Uuid) -> Self {
        self.id = id.clone();
        self
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_owned();
        self
    }

    pub fn with_mesh_component(mut self, mesh_component: MeshComponent) -> Self {
        self.mesh_component = Some(mesh_component);
        self
    }

    pub fn with_light_component(mut self, light_component: LightComponent) -> Self {
        self.light_component = Some(light_component);
        self
    }

    pub fn with_transform_component(mut self, transform_component: TransformComponent) -> Self {
        self.transform_component = Some(transform_component);
        self
    }

    pub fn with_camera_component(mut self, camera_component: CameraComponent) -> Self {
        self.camera_component = Some(camera_component);
        self
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_component(&mut self, component: Component) {
        match component {
            Component::Mesh(mesh) => {
                self.mesh_component = Some(mesh);
            }
            Component::Light(light) => {
                self.light_component = Some(light);
            }
            Component::Transform(transform) => {
                self.transform_component = Some(transform);
            }
            Component::Camera(camera) => {
                self.camera_component = Some(camera);
            }
        }
    }

    pub fn get_mesh_component(&self) -> Option<&MeshComponent> {
        self.mesh_component.as_ref()
    }

    pub fn get_mesh_component_mut(&mut self) -> Option<&mut MeshComponent> {
        self.mesh_component.as_mut()
    }

    pub fn get_light_component(&self) -> Option<&LightComponent> {
        self.light_component.as_ref()
    }

    pub fn get_light_component_mut(&mut self) -> Option<&mut LightComponent> {
        self.light_component.as_mut()
    }

    pub fn get_transform_component(&self) -> Option<&TransformComponent> {
        self.transform_component.as_ref()
    }

    pub fn get_transform_component_mut(&mut self) -> Option<&mut TransformComponent> {
        self.transform_component.as_mut()
    }

    pub fn get_camera_component(&self) -> Option<&CameraComponent> {
        self.camera_component.as_ref()
    }

    pub fn get_camera_component_mut(&mut self) -> Option<&mut CameraComponent> {
        self.camera_component.as_mut()
    }

    pub fn accept<E>(
        &self,
        visitor: &mut dyn Visitor<E>,
        asset_server: &AssetServer,
    ) -> anyhow::Result<()>
    where
        E: std::error::Error + Sync + Send + 'static,
    {
        // Visit itself
        visitor.visit(self, asset_server)?;

        // Traverse entity hierachy using transform component
        if let Some(transform_component) = &self.transform_component {
            for child in transform_component.childs() {
                if let Some(child_entity) = asset_server.get_entity(child) {
                    child_entity.accept(visitor, asset_server)?;
                }
            }
        }

        Ok(())
    }
}
