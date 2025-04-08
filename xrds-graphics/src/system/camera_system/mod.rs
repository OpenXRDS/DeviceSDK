mod camera_data;
mod camera_info;

pub use camera_data::*;
pub use camera_info::*;

use std::{collections::HashMap, num::NonZeroU64};

use uuid::Uuid;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, BufferBinding,
    BufferDescriptor, BufferUsages,
};
use xrds_core::Transform;

use crate::{create_deferred_lighting_proc, Framebuffer, GraphicsInstance, TextureFormat};

#[derive(Debug, Clone)]
pub struct CameraSystem {
    cameras: HashMap<Uuid, CameraData>,
    graphics_instance: GraphicsInstance,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl CameraSystem {
    pub fn new(graphics_instance: GraphicsInstance) -> Self {
        let bind_group_layout =
            graphics_instance
                .device()
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        Self {
            cameras: HashMap::new(),
            graphics_instance,
            bind_group_layout,
        }
    }

    pub fn add_camera(
        &mut self,
        camera_entity_id: &Uuid,
        cameras: &[CameraInfo],
        transforms: &[Transform],
        extent: Option<wgpu::Extent3d>,
        output_format: TextureFormat,
    ) -> anyhow::Result<Uuid> {
        let spawn_id = Uuid::new_v4();
        let extent = extent.unwrap_or(wgpu::Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        });

        let uniform_size = (std::mem::size_of::<ViewParams>() * cameras.len()) as u64;
        let uniform_buffer = self
            .graphics_instance
            .device()
            .create_buffer(&BufferDescriptor {
                label: None,
                size: uniform_size,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        let bind_group = self
            .graphics_instance
            .device()
            .create_bind_group(&BindGroupDescriptor {
                label: Some("Camera"),
                layout: &self.bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(uniform_size),
                    }),
                }],
            });

        // TODO: backbuffering
        let framebuffers = vec![
            Framebuffer::new(&self.graphics_instance, extent, output_format),
            Framebuffer::new(&self.graphics_instance, extent, output_format),
        ];

        let deferred_lighting_proc = create_deferred_lighting_proc(
            &self.graphics_instance,
            framebuffers[0].gbuffer_bind_group_layout(),
            output_format,
        )?;
        log::info!("deferred_lighing_proc: {:?}", deferred_lighting_proc);

        self.cameras.insert(
            spawn_id.clone(),
            CameraData {
                camera_entity_id: *camera_entity_id,
                cameras: cameras.to_vec(),
                transforms: transforms.to_vec(),
                cam_uniform_buffer: uniform_buffer,
                cam_bind_group: bind_group,
                framebuffers,
                framebuffer_index: 0,
                copy_target: None,
                deferred_lighting: deferred_lighting_proc,
            },
        );
        Ok(spawn_id)
    }

    pub fn begin_frame(&mut self) {
        for (_, camera) in &mut self.cameras {
            camera.begin_frame();
        }
    }

    pub fn cameras(&self) -> Vec<CameraData> {
        self.cameras
            .iter()
            .map(|(_, camera_data)| camera_data.clone())
            .collect()
    }

    pub fn camera(&self, camera_id: &Uuid) -> Option<&CameraData> {
        self.cameras.get(camera_id)
    }

    pub fn camera_mut(&mut self, camera_id: &Uuid) -> Option<&mut CameraData> {
        self.cameras.get_mut(camera_id)
    }
}
