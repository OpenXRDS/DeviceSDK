use std::{
    collections::{BTreeMap, HashMap},
    ops::Range,
    sync::{Arc, RwLock},
    time::Duration,
};

use uuid::Uuid;
use xrds_core::Transform;
use xrds_graphics::{
    AssetServer, CameraData, CameraFinder, CameraSystem, GraphicsInstance, LightSystem, ObjectData,
    ObjectInstance, PrimitiveCollector, RenderItem, RenderSystem, State, TextureFormat,
    TransformSystem, Visitor, XrdsInstance,
};

use crate::{WorldEvent, WorldOnCameraUpdated};

#[derive(Debug, Clone)]
pub struct World {
    asset_server: Arc<RwLock<AssetServer>>,
    camera_system: CameraSystem,
    render_system: RenderSystem,
    light_system: LightSystem,
    transform_system: TransformSystem,
    instance_ranges: BTreeMap<Uuid, Range<u32>>,
    spawned_objects: Arc<RwLock<HashMap<Uuid, ObjectData>>>,
}

impl World {
    pub(crate) fn new(
        asset_server: Arc<RwLock<AssetServer>>,
        graphics_instance: &GraphicsInstance,
        render_system: RenderSystem,
        light_system: LightSystem,
        transform_system: TransformSystem,
    ) -> Self {
        let spawned_objects = Arc::new(RwLock::new(HashMap::new()));
        let camera_system = CameraSystem::new(graphics_instance.clone());

        Self {
            asset_server,
            spawned_objects,
            camera_system,
            render_system,
            light_system,
            transform_system,
            instance_ranges: BTreeMap::new(),
        }
    }

    /// update tickable
    pub(crate) fn on_update(&mut self, diff: Duration) -> anyhow::Result<()> {
        // self.transform_system.update(&mut self.entities);
        // self.light_system.update(&mut self.entities);

        Ok(())
        // todo!()
    }

    /// update render specific data
    pub(crate) fn on_pre_render(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_instances(&mut self, _camera_data: &CameraData) -> anyhow::Result<()> {
        let spawned_objects = self.spawned_objects.read().unwrap();
        let asset_server = self.asset_server.read().unwrap();

        // Build instance buffer
        let visible_objects: Vec<_> = spawned_objects.iter().collect();
        let mut instances_map: HashMap<Uuid, Vec<XrdsInstance>> = HashMap::new();
        for (_spawn_id, obj) in &visible_objects {
            let instance = XrdsInstance::new(*obj.transform());
            if let Some(instances) = instances_map.get_mut(obj.entity_id()) {
                instances.push(instance);
            } else {
                instances_map.insert(*obj.entity_id(), vec![instance]);
            }
        }
        // Build entity-range map
        let mut instance_ranges: BTreeMap<Uuid, Range<u32>> = BTreeMap::new();
        let mut current_offset = 0u32;
        for (uuid, instances) in instances_map.iter() {
            instance_ranges.insert(
                *uuid,
                current_offset..current_offset + instances.len() as u32,
            );
            current_offset += instances.len() as u32;
        }

        // Collect all primitives in entire transform hierachy and indexing by primitive's material id
        let mut primitive_collector = PrimitiveCollector::new();
        for (_, obj) in &visible_objects {
            let root_entity_id = obj.entity_id();
            let root_entity = asset_server
                .get_entity(root_entity_id)
                .expect("Entity not found");
            root_entity.accept(&mut primitive_collector, &asset_server)?;
        }
        let material_primitive_map = primitive_collector.primitives();
        let mut material_renderitem_map = HashMap::new();
        for (material_id, mapped_primitives) in material_primitive_map {
            let render_items: &mut Vec<_> =
                if let Some(render_items) = material_renderitem_map.get_mut(material_id) {
                    render_items
                } else {
                    material_renderitem_map.insert(material_id.clone(), Vec::new());
                    material_renderitem_map.get_mut(material_id).unwrap()
                };

            for mapped_primitive in mapped_primitives {
                let render_item = RenderItem {
                    primitive: mapped_primitive.primitive.clone(),
                    instances: instance_ranges
                        .get(&mapped_primitive.root_entity_id)
                        .expect("Could not found matched entity id")
                        .clone(),
                };
                render_items.push(render_item);
            }
        }

        self.instance_ranges = instance_ranges;
        let instances_data: Vec<XrdsInstance> = instances_map
            .into_iter()
            .map(|(_, instances)| instances)
            .flatten()
            .collect();

        self.render_system
            .update_instances(&instances_data, material_renderitem_map)?;

        Ok(())
    }

    pub(crate) fn on_render(&mut self) -> anyhow::Result<()> {
        for camera_data in self.camera_system.cameras() {
            let mut command_encoder = self.render_system.on_pre_render();
            self.update_instances(&camera_data)?;
            self.render_system
                .on_render(&mut command_encoder, &camera_data)?;
            self.render_system.on_post_render(command_encoder);
        }
        Ok(())
    }

    pub(crate) fn on_post_render(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Spawn new object into world. Return object's uuid for control spawned object
    pub fn spawn(
        &mut self,
        entity_id: &Uuid,
        transform: &Transform,
    ) -> anyhow::Result<ObjectInstance> {
        let spawned_id = Uuid::new_v4();
        let spawned_object = ObjectInstance::new(spawned_id);
        let object_data = ObjectData::new(*entity_id, *transform, State::default());

        log::debug!(
            "Spawn object: {{spawn_id={}, entity_id={}, world_pos={:?}}}",
            spawned_object.spawn_id(),
            object_data.entity_id(),
            object_data.transform().get_translation()
        );

        // Pre-defined camera from source (like gltf)
        // So use fixed size and format for camera rendering
        self.spawn_camera(
            entity_id,
            None,
            TextureFormat::from(wgpu_types::TextureFormat::Rgba32Float), // this is intermediate texture. So set to rgba32float
        )?;

        self.spawned_objects
            .write()
            .unwrap()
            .insert(*spawned_object.spawn_id(), object_data);

        Ok(spawned_object)
    }

    /// Spawns a camera into the world.
    ///
    /// This function adds a camera to the world's camera management system. It searches for
    /// camera components within the provided entity and, if found, creates and registers
    /// the camera with the `CameraManager`.
    ///
    /// # Parameters
    ///
    /// * `entity_id`: A reference to the `Uuid` of the entity that potentially contains a camera component.
    ///   This ID is used to look up the entity within the `AssetServer`.
    ///
    /// * `extent`: An optional `wgpu::Extent3d` that specifies the size of the camera's render target.
    ///   If `None`, a default size will be used. This parameter determines the resolution of the
    ///   framebuffer associated with the camera.
    ///
    /// * `final_format`: A `TextureFormat` that specifies the format of the final output texture for the camera.
    ///   This format is used when creating the framebuffer for the camera.
    ///
    /// # Returns
    ///
    /// This function returns `Ok(())` if the camera was successfully spawned or if no camera was found.
    /// It returns an `Err` if there was an error during the process, such as if the entity does not exist
    /// or if a camera component is missing required data.
    ///
    /// # Errors
    ///
    /// This function can return an error in the following situations:
    ///
    /// *   If the entity specified by `entity_id` does not exist in the `AssetServer`.
    /// *   If a camera entity is found but does not have a `CameraComponent`.
    /// *   If there are any internal errors during the camera creation or registration process.
    pub fn spawn_camera(
        &mut self,
        entity_id: &Uuid,
        extent: Option<wgpu_types::Extent3d>,
        final_format: TextureFormat,
    ) -> anyhow::Result<Vec<Uuid>> {
        let mut finder = CameraFinder::new();
        let mut camera_spawn_ids = Vec::new();

        let asset_server = self.asset_server.read().unwrap();
        let entity = asset_server.get_entity(entity_id).unwrap();

        finder.visit(entity, &asset_server)?;
        for camera_entity_id in finder.camera_entity_ids() {
            if let Some(camera_entity) = asset_server.get_entity(camera_entity_id) {
                let camera_component = camera_entity
                    .get_camera_component()
                    .expect("Camera entity but CameraComponent not found");
                let cameras = camera_component.cameras();
                log::info!("Spawn camera: {:?}", cameras);

                let transforms =
                    if let Some(transform_component) = camera_entity.get_transform_component() {
                        vec![transform_component.local_transform; cameras.len()]
                    } else {
                        vec![Transform::default(); cameras.len()]
                    };

                let camera_spawn_id = self.camera_system.add_camera(
                    camera_entity_id,
                    &cameras,
                    &transforms,
                    extent,
                    final_format,
                )?;

                camera_spawn_ids.push(camera_spawn_id);
            }
        }

        Ok(camera_spawn_ids)
    }

    pub fn emit_event(&mut self, event: WorldEvent) -> anyhow::Result<()> {
        match event {
            WorldEvent::OnCameraUpdated(camera_info) => self.on_camera_updated(camera_info)?,
        }
        Ok(())
    }

    fn on_camera_updated(&mut self, camera_info: WorldOnCameraUpdated) -> anyhow::Result<()> {
        if let Some(camera) = self.camera_system.camera_mut(camera_info.camera_id) {
            let (fovs, transforms): (Vec<_>, Vec<_>) =
                camera_info.camera_update_infos.into_iter().unzip();
            camera.set_fovs(&fovs);
            camera.set_transforms(&transforms);
            camera.set_copy_target(camera_info.copy_target);
        } else {
            log::warn!(
                "Camera update event received, but camera not found {:?}",
                camera_info.camera_id
            );
        }
        Ok(())
    }
}
