use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::{Arc, RwLock},
    time::Duration,
};

use uuid::Uuid;
use xrds_core::Transform;
use xrds_graphics::{GraphicsInstance, RenderPass, XrdsInstance, XrdsInstanceBuffer};

use crate::{Object, SpawnedObject};

#[derive(Debug, Clone)]
pub struct World {
    graphics_instance: Arc<GraphicsInstance>,
    object_pool: Arc<RwLock<HashMap<Uuid, Object>>>,
    spawned_objects: Arc<RwLock<HashMap<Uuid, SpawnedObject>>>,
    instance_buffer: XrdsInstanceBuffer,
    instances: Arc<RwLock<HashMap<Uuid, Range<u32>>>>,
}

impl World {
    pub(crate) fn new(graphics_instance: Arc<GraphicsInstance>, max_instances: usize) -> Self {
        let instance_buffer = XrdsInstanceBuffer::new(graphics_instance.clone(), max_instances);
        Self {
            graphics_instance,
            object_pool: Arc::new(RwLock::new(HashMap::new())),
            spawned_objects: Arc::new(RwLock::new(HashMap::new())),
            instance_buffer,
            instances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn update_instances(&self) -> anyhow::Result<()> {
        let spawned_objects = self.spawned_objects.read().unwrap();

        // Do frustum culling here

        // Make (object_unique_id, vector<XrdsInstance>) pair
        let mut instances_map: HashMap<Uuid, Vec<XrdsInstance>> = HashMap::new();
        spawned_objects.iter().for_each(|(_, obj)| {
            let instance = XrdsInstance::new(*obj.transform());
            if let Some(instances) = instances_map.get_mut(obj.object_id()) {
                instances.push(instance);
            } else {
                instances_map.insert(*obj.object_id(), vec![instance]);
            }
        });

        // Make (object_unique_id, range) map
        let mut instance_ranges: HashMap<Uuid, Range<u32>> = HashMap::new();
        let mut current_offset = 0u32;
        instances_map.iter().for_each(|(uuid, instances)| {
            instance_ranges.insert(
                *uuid,
                current_offset..current_offset + instances.len() as u32,
            );
            current_offset += instances.len() as u32;
        });
        *self.instances.write().unwrap() = instance_ranges;
        let instances_data: Vec<XrdsInstance> = instances_map
            .into_iter()
            .map(|(_, instances)| instances)
            .flatten()
            .collect();

        self.instance_buffer
            .write(self.graphics_instance.queue(), &instances_data);

        Ok(())
    }

    pub(crate) fn update(&mut self, diff: Duration) -> anyhow::Result<()> {
        // update objects
        Ok(())
    }

    pub(crate) fn encode(&self, render_pass: &mut RenderPass) -> anyhow::Result<()> {
        let object_pool = self.object_pool.read().unwrap();

        render_pass.bind_instance_buffer(&self.instance_buffer);

        // self.build_bulk();
        for (uuid, range) in self.instances.read().unwrap().iter() {
            if let Some(object) = object_pool.get(uuid) {
                object.encode(render_pass, range)?;
            }
        }

        Ok(())
    }

    /// Spawn new object into world. Return object's uuid for control spawned object
    pub fn spawn(&self, object: &Object, transform: &Transform) -> anyhow::Result<SpawnedObject> {
        let has_spawned = {
            let lock = self.object_pool.read().unwrap();
            lock.contains_key(object.uuid())
        };

        if !has_spawned {
            let mut lock = self.object_pool.write().unwrap();
            lock.insert(*object.uuid(), object.clone());
        }
        let spawned_object = SpawnedObject::new(Uuid::new_v4(), *object.uuid(), *transform);

        log::debug!(
            "Spawn object: {{object={}, world_pos={:?}}}",
            spawned_object.spawn_id(),
            spawned_object.transform().get_translation()
        );

        self.spawned_objects
            .write()
            .unwrap()
            .insert(*spawned_object.spawn_id(), spawned_object.clone());

        Ok(spawned_object)
    }
}
