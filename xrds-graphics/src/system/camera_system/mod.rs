mod camera_info;
mod camera_instance;

pub use camera_info::*;
pub use camera_instance::*;

use std::{collections::HashMap, num::NonZeroU64};

use uuid::Uuid;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, BufferBinding,
    BufferDescriptor, BufferUsages,
};
use xrds_core::Transform;

use crate::{CopySwapchainProc, Framebuffer, GraphicsInstance, TextureFormat};

#[derive(Debug)]
pub struct CameraSystem {
    cameras: HashMap<Uuid, CameraInstance>,
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
        output_format: Option<TextureFormat>,
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
        let framebuffer = Framebuffer::new(
            &self.graphics_instance,
            extent,
            TextureFormat::from(wgpu::TextureFormat::Rgba16Float),
        );

        let copy_swapchain_proc = if let Some(output_format) = output_format {
            Some(CopySwapchainProc::new(
                &self.graphics_instance,
                output_format,
            )?)
        } else {
            None
        };

        self.cameras.insert(
            spawn_id.clone(),
            CameraInstance {
                graphics_instance: self.graphics_instance.clone(),
                camera_entity_id: *camera_entity_id,
                cameras: cameras.to_vec(),
                view_params: Vec::new(),
                transforms: transforms.to_vec(),
                cam_uniform_buffer: uniform_buffer,
                cam_bind_group: bind_group,
                framebuffer,
                copy_swapchain_proc,
                frame_index: 0,
            },
        );

        Ok(spawn_id)
    }

    pub fn on_pre_render(&mut self) {
        for (_, camera) in &mut self.cameras {
            camera.begin_frame();
            camera.update_view_params();
            camera.update_uniform();
        }
    }

    pub fn cameras(&self) -> Vec<&CameraInstance> {
        self.cameras
            .iter()
            .map(|(_, camera_data)| camera_data)
            .collect()
    }

    pub fn cameras_mut(&mut self) -> Vec<&mut CameraInstance> {
        self.cameras
            .iter_mut()
            .map(|(_, camera_data)| camera_data)
            .collect()
    }

    pub fn camera_ids(&self) -> Vec<Uuid> {
        self.cameras.keys().cloned().collect()
    }

    pub fn camera(&self, camera_id: &Uuid) -> Option<&CameraInstance> {
        self.cameras.get(camera_id)
    }

    pub fn camera_mut(&mut self, camera_id: &Uuid) -> Option<&mut CameraInstance> {
        self.cameras.get_mut(camera_id)
    }
}
