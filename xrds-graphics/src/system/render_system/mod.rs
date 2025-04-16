mod framebuffer;
mod gbuffer;

pub use framebuffer::*;
pub use gbuffer::*;
use xrds_core::Transform;

use std::{
    collections::HashMap,
    ops::Range,
    sync::{Arc, RwLock},
};

use wgpu::{
    Color, CommandEncoder, CommandEncoderDescriptor, Operations, RenderPassColorAttachment,
    RenderPassDescriptor,
};

use crate::{
    AssetId, AssetServer, CameraInstance, CopySwapchainProc, DeferredLightingProc,
    GraphicsInstance, XrdsInstance, XrdsInstanceBuffer, XrdsPrimitive,
};

use super::{Constant, LightInstance, LightSystem};

#[derive(Debug, Clone)]
pub struct RenderItem {
    pub primitive: XrdsPrimitive,
    pub local_transform: Transform,
    pub instances: Range<u32>,
}

#[derive(Debug, Clone)]
pub struct RenderSystem {
    graphics_instance: GraphicsInstance,
    asset_server: Arc<RwLock<AssetServer>>,
    instance_buffer: XrdsInstanceBuffer,
    material_renderitem_map: HashMap<AssetId, Vec<RenderItem>>,
    deferred_lighting_proc: DeferredLightingProc,
    // query_set: QuerySet,
}

impl RenderSystem {
    const DEFAULT_MAXIMUM_INSTANCES: usize = 10000;

    pub fn new(
        graphics_instance: GraphicsInstance,
        asset_server: Arc<RwLock<AssetServer>>,
        maximum_instances: Option<usize>,
    ) -> anyhow::Result<Self> {
        let instance_buffer = XrdsInstanceBuffer::new(
            &graphics_instance,
            maximum_instances.unwrap_or(Self::DEFAULT_MAXIMUM_INSTANCES),
        );

        let deferred_lighting_proc = DeferredLightingProc::new(&graphics_instance)?;

        Ok(Self {
            graphics_instance,
            instance_buffer,
            asset_server,
            material_renderitem_map: HashMap::new(),
            deferred_lighting_proc,
        })
    }

    pub fn on_pre_render(&mut self) -> CommandEncoder {
        // Travel entity tree and make as vector for bulk rendering for each instance
        // let bulk_instances = Vec<Vec<XrdsPrimitive>>
        self.graphics_instance
            .device()
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("RenderSystemCommandEncoder"),
            })
    }
    pub fn on_render(
        &mut self,
        encoder: &mut CommandEncoder,
        camera_data: &CameraInstance,
        light_system: &LightSystem,
    ) -> anyhow::Result<()> {
        // Iterate over cameras (that has render target)
        let framebuffer = camera_data.current_frame();
        camera_data.update_uniform(&self.graphics_instance);

        self.do_shadow_pass(encoder, light_system)?;
        self.do_gbuffer_pass(encoder, camera_data, framebuffer)?;
        self.do_deferred_lighting(encoder, camera_data, light_system, framebuffer)?;

        {
            // Future work
            // let upscale_pass =
            // let taa_pass =
        }

        if let Some(copy_swapchain) = camera_data.copy_swapchain_proc() {
            self.do_copy_swapchain(encoder, framebuffer, copy_swapchain)?;
        }

        Ok(())
    }

    fn do_shadow_pass(
        &mut self,
        encoder: &mut CommandEncoder,
        light_system: &LightSystem,
    ) -> anyhow::Result<()> {
        for light_uuid in light_system.light_uuids() {
            if let Some(light_instance) = light_system.get_light_instance(light_uuid) {
                if light_instance.state().cast_shadow() {
                    let mut render_pass =
                        self.create_shadow_pass(encoder, light_instance, light_system)?;
                    self.instance_buffer
                        .encode(&mut render_pass, Constant::VERTEX_ID_INSTANCES);

                    light_system.encode_shadow_mapping(light_uuid, &mut render_pass);
                    for (_, render_items) in &self.material_renderitem_map {
                        // Get material from asset_server
                        for render_item in render_items {
                            render_item.primitive.encode_geometry(
                                &mut render_pass,
                                &render_item.local_transform,
                                render_item.instances.clone(),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn do_gbuffer_pass(
        &mut self,
        encoder: &mut CommandEncoder,
        camera_data: &CameraInstance,
        framebuffer: &Framebuffer,
    ) -> anyhow::Result<()> {
        let mut render_pass = self.create_gbuffer_pass(encoder, framebuffer)?;
        camera_data.encode_view_params(&mut render_pass, Constant::BIND_GROUP_ID_VIEW_PARAMS);
        self.instance_buffer
            .encode(&mut render_pass, Constant::VERTEX_ID_INSTANCES);

        let asset_server = self.asset_server.read().unwrap();
        for (material_id, render_items) in &self.material_renderitem_map {
            // Get material from asset_server
            if let Some(material_instance) = asset_server.get_material_instance_by_id(material_id) {
                material_instance.encode(&mut render_pass);
                for render_item in render_items {
                    render_item.primitive.encode(
                        &mut render_pass,
                        &render_item.local_transform,
                        render_item.instances.clone(),
                    );
                }
            }
        }

        Ok(())
    }

    fn do_deferred_lighting(
        &mut self,
        encoder: &mut CommandEncoder,
        camera_data: &CameraInstance,
        light_system: &LightSystem,
        framebuffer: &Framebuffer,
    ) -> anyhow::Result<()> {
        let mut render_pass =
            self.create_postproc_pass(encoder, &framebuffer.final_color_attachments()?);
        camera_data.encode_view_params(&mut render_pass, 0);
        framebuffer.encode_gbuffer_params(&mut render_pass, 1);
        light_system.encode_light_params(&mut render_pass, 2);

        // draw deferred light with gbuffer, shadowmap and bulk light
        self.deferred_lighting_proc.encode(&mut render_pass);

        Ok(())
    }

    fn do_copy_swapchain(
        &mut self,
        encoder: &mut CommandEncoder,
        framebuffer: &Framebuffer,
        copy_swapchain_proc: &CopySwapchainProc,
    ) -> anyhow::Result<()> {
        if let Some(target) = copy_swapchain_proc.target_view() {
            let mut render_pass = self.create_postproc_pass(
                encoder,
                &[Some(RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
            );
            framebuffer.encode_final_color(&mut render_pass, 0);
            copy_swapchain_proc.encode(&mut render_pass);
        }

        Ok(())
    }

    pub fn on_post_render(&mut self, command_encoder: CommandEncoder) {
        // Do something for finishing rendering. Maybe submit queue?
        // command_encoder.resolve_query_set(&self.query_set, 0..4, &self.query_buffer, 0);
        let command_buffer = command_encoder.finish();
        self.graphics_instance.queue().submit([command_buffer]);

        self.graphics_instance.queue().get_timestamp_period();
    }

    pub fn update_instances(
        &mut self,
        instances: &[XrdsInstance],
        material_renderitem_map: HashMap<AssetId, Vec<RenderItem>>,
    ) -> anyhow::Result<()> {
        self.instance_buffer
            .write(self.graphics_instance.queue(), instances);
        self.material_renderitem_map = material_renderitem_map;

        Ok(())
    }

    fn create_gbuffer_pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        framebuffer: &Framebuffer,
    ) -> anyhow::Result<wgpu::RenderPass<'e>> {
        let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &framebuffer.gbuffer().as_color_attachments()?,
            depth_stencil_attachment: framebuffer.gbuffer().as_depth_stencil_attachment()?,
            ..Default::default()
        });

        Ok(render_pass)
    }

    fn create_shadow_pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        light_instance: &LightInstance,
        light_system: &LightSystem,
    ) -> anyhow::Result<wgpu::RenderPass<'e>> {
        let index = light_instance
            .state()
            .shadow_map_index()
            .expect("Light instance not attached");
        let color_attachments = light_system.get_shadowmap_attachments(index)?;
        let depth_attachment = light_system.get_depth_attachment(index)?;

        let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments, // &light.shadowmap_attachments()?
            depth_stencil_attachment: depth_attachment,
            ..Default::default()
        });

        Ok(render_pass)
    }

    fn create_postproc_pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        color_attachments: &[Option<RenderPassColorAttachment>],
    ) -> wgpu::RenderPass<'e> {
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments,
            depth_stencil_attachment: None,
            ..Default::default()
        })
    }
}
