mod light_instance;
mod shadowmap_pool;

use std::collections::HashMap;

pub use light_instance::*;
pub use shadowmap_pool::*;
use uuid::Uuid;
use wgpu::{
    BindGroupLayoutDescriptor, BufferBinding, BufferDescriptor, BufferUsages,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment,
};
use xrds_core::ViewDirection;

use crate::{Constant, GraphicsInstance, LightComponent, LightType, ShadowMapping, XrdsLight};

use super::BindGroupLayoutHelper;

type LightIndex = usize;

#[derive(Debug)]
pub struct LightSystem {
    graphics_instance: GraphicsInstance,
    lights: Vec<LightInstance>,
    spawned_lights: HashMap<Uuid, LightIndex>,
    shadowmap_pool: ShadowmapPool,
    light_storage_buffer: wgpu::Buffer,
    light_params_buffer: wgpu::Buffer,
    /// bind group for lighting pass
    lighting_bind_group: wgpu::BindGroup,
    lighting_bind_group_layout: wgpu::BindGroupLayout,
    shadow_mapping_bind_group: wgpu::BindGroup,
    shadow_mapping: ShadowMapping,
    light_updated: bool,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightSystemParams {
    light_count: u32,
}

impl LightSystem {
    const DEFAULT_MAX_LIGHT_COUNT: usize = 1024;
    const DEFAULT_SHADOW_QUALITY: ShadowQuality = ShadowQuality::UltraHigh;

    pub fn new(
        graphics_instance: GraphicsInstance,
        shadow_quality: Option<ShadowQuality>,
    ) -> anyhow::Result<Self> {
        let shadowmap_pool = ShadowmapPool::new(
            &graphics_instance,
            shadow_quality.unwrap_or(Self::DEFAULT_SHADOW_QUALITY),
        );

        // Buffers for light data
        let light_storage_buffer =
            Self::create_storage_buffer(graphics_instance.device(), Self::DEFAULT_MAX_LIGHT_COUNT);
        let light_params_buffer = Self::create_light_params_buffer(graphics_instance.device());

        let lighting_bind_group_layout =
            BindGroupLayoutHelper::create_light_params(graphics_instance.device());
        let shadowmap_views = shadowmap_pool.shadowmap_views();
        let lighting_bind_group = Self::create_lighting_bind_group(
            graphics_instance.device(),
            &lighting_bind_group_layout,
            &light_storage_buffer,
            &light_params_buffer,
            shadowmap_pool.sampler(),
            &shadowmap_views,
        );

        let output_bind_group_layout =
            Self::create_shadow_mapping_bind_group_layout(graphics_instance.device());
        let shadow_mapping_bind_group = Self::create_shadow_mapping_bind_group(
            graphics_instance.device(),
            &output_bind_group_layout,
            &light_storage_buffer,
        );

        let shadow_mapping = ShadowMapping::new(&graphics_instance, &output_bind_group_layout)?;

        Ok(Self {
            graphics_instance,
            lights: Vec::new(),
            spawned_lights: HashMap::new(),
            shadowmap_pool,
            light_storage_buffer,
            light_params_buffer,
            lighting_bind_group,
            lighting_bind_group_layout,
            shadow_mapping_bind_group,
            shadow_mapping,
            light_updated: true, // initially true for update light buffer in first frame
        })
    }

    pub fn spawn_light(
        &mut self,
        entity_id: &Uuid,
        view_direction: &ViewDirection,
        light_component: &LightComponent,
    ) -> anyhow::Result<Uuid> {
        let spawned_uuid = Uuid::new_v4();

        let mut light_instance = LightInstance::new(*entity_id, *light_component.light_type());
        let cast_shadow = if light_component.cast_shadow() {
            if let Ok(increased) = self.shadowmap_pool.assign_index(&mut light_instance) {
                if increased {
                    let shadowmap_views = self.shadowmap_pool.shadowmap_views();
                    let lighting_bind_group = Self::create_lighting_bind_group(
                        self.graphics_instance.device(),
                        &self.lighting_bind_group_layout,
                        &self.light_storage_buffer,
                        &self.light_params_buffer,
                        self.shadowmap_pool.sampler(),
                        &shadowmap_views,
                    );
                    self.lighting_bind_group = lighting_bind_group;
                }
                true
            } else {
                log::warn!("Shadowmap pool is full. Currently not support dynamic pool size. Force cast shadow off");
                false
            }
        } else {
            false
        };

        let light_state = light_instance.state_mut();
        light_state.set_transform(*view_direction);
        light_state.set_color(*light_component.color());
        light_state.set_intensity(light_component.intensity());
        let range = match light_component.light_type() {
            LightType::Directional => f32::MAX,
            LightType::Point(description) => description.range,
            LightType::Spot(description) => description.range,
        };
        light_state.set_range(range);
        light_state.set_cast_shadow(cast_shadow);

        let new_light_index = self.lights.len();
        self.lights.push(light_instance);
        self.spawned_lights.insert(spawned_uuid, new_light_index);

        self.light_updated = true;

        Ok(spawned_uuid)
    }

    pub fn light_uuids(&self) -> Vec<&Uuid> {
        self.spawned_lights.keys().collect()
    }

    pub fn get_light_instance(&self, uuid: &Uuid) -> Option<&LightInstance> {
        self.spawned_lights
            .get(uuid)
            .and_then(|index| self.lights.get(*index))
    }

    pub fn get_shadowmap_attachments(
        &self,
        index: u32,
    ) -> anyhow::Result<Vec<Option<RenderPassColorAttachment>>> {
        Ok(vec![Some(RenderPassColorAttachment {
            view: self.shadowmap_pool.get_shadowmap(index as usize)?.view(),
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })])
    }

    pub fn get_depth_attachment(
        &self,
        index: u32,
    ) -> anyhow::Result<Option<RenderPassDepthStencilAttachment>> {
        Ok(Some(RenderPassDepthStencilAttachment {
            view: self
                .shadowmap_pool
                .get_shadowmap_depth(index as usize)?
                .view(),
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }))
    }

    pub fn lighting_bind_group(&self) -> &wgpu::BindGroup {
        &self.lighting_bind_group
    }

    pub fn lighting_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.lighting_bind_group_layout
    }

    pub fn on_pre_render(&mut self) {
        if self.light_updated {
            let light_buffer_data: Vec<XrdsLight> = self
                .lights
                .iter()
                .map(|light_instance| light_instance.into())
                .collect();
            let queue = self.graphics_instance.queue();
            queue.write_buffer(
                &self.light_storage_buffer,
                0,
                bytemuck::cast_slice(&light_buffer_data),
            );
            let params = LightSystemParams {
                light_count: self.lights.len() as u32,
            };
            queue.write_buffer(
                &self.light_params_buffer,
                0,
                bytemuck::cast_slice(&[params]),
            );
            self.light_updated = false;
        }
    }

    pub fn encode_shadow_mapping(&self, light_uuid: &Uuid, render_pass: &mut wgpu::RenderPass<'_>) {
        let light_index = *self
            .spawned_lights
            .get(light_uuid)
            .expect("Unexpected light uuid");
        let light_offset = light_index * std::mem::size_of::<XrdsLight>();

        render_pass.set_pipeline(self.shadow_mapping.pipeline());
        render_pass.set_bind_group(
            Constant::BIND_GROUP_ID_SHADOWMAP_LIGHT,
            &self.shadow_mapping_bind_group,
            &[light_offset as u32],
        );
    }

    pub fn encode_light_params(&self, render_pass: &mut wgpu::RenderPass<'_>, index: u32) {
        render_pass.set_bind_group(index, &self.lighting_bind_group, &[]);
    }

    fn create_storage_buffer(device: &wgpu::Device, max_light_count: usize) -> wgpu::Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("LightDataBuffer"),
            size: (std::mem::size_of::<XrdsLight>() * max_light_count) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_light_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("LightParamsBuffer"),
            size: std::mem::size_of::<LightSystemParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_lighting_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        storage_buffer: &wgpu::Buffer,
        light_param_buffer: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        shadowmap_views: &[&wgpu::TextureView],
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("LightingBindGroup"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: storage_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: light_param_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureViewArray(shadowmap_views),
                },
            ],
        })
    }

    fn create_shadow_mapping_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("ShadowMappingBindGroupLayout"),
            entries: &[
                // var<storage, read> s_light_data: array<Light>
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: true, // Light uniform buffer is dynamic
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_shadow_mapping_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        storage_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ShadowMappingBindGroup"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(BufferBinding {
                    buffer: storage_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        })
    }
}
