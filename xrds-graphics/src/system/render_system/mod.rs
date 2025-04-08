mod framebuffer;
mod gbuffer;

pub use framebuffer::*;
pub use gbuffer::*;
use xrds_core::Transform;

use std::{
    collections::HashMap,
    num::NonZeroU32,
    ops::Range,
    sync::{Arc, RwLock},
};

use wgpu::{
    CommandEncoder, CommandEncoderDescriptor, QuerySet, QuerySetDescriptor, RenderPassDescriptor,
    RenderPassTimestampWrites,
};

use crate::{
    AssetId, AssetServer, CameraData, GraphicsInstance, XrdsInstance, XrdsInstanceBuffer,
    XrdsPrimitive,
};

use super::Constant;

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
    // query_set: QuerySet,
}

impl RenderSystem {
    const DEFAULT_MAXIMUM_INSTANCES: usize = 10000;

    pub fn new(
        graphics_instance: GraphicsInstance,
        asset_server: Arc<RwLock<AssetServer>>,
        maximum_instances: Option<usize>,
    ) -> Self {
        let instance_buffer = XrdsInstanceBuffer::new(
            &graphics_instance,
            maximum_instances.unwrap_or(Self::DEFAULT_MAXIMUM_INSTANCES),
        );

        // let query_set = graphics_instance
        //     .device()
        //     .create_query_set(&QuerySetDescriptor {
        //         label: Some("RenderSystem"),
        //         ty: wgpu::QueryType::Timestamp,
        //         count: 4,
        //     });

        Self {
            graphics_instance,
            instance_buffer,
            asset_server,
            material_renderitem_map: HashMap::new(),
            // query_set,
        }
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
        camera_data: &CameraData,
    ) -> anyhow::Result<()> {
        // Iterate over cameras (that has render target)
        let framebuffer = camera_data.current_frame();

        camera_data.update_uniform(&self.graphics_instance);

        // G-Buffer Pass
        {
            let mut render_pass = self.create_gbuffer_pass(encoder, framebuffer)?;
            camera_data.encode_view_params(&mut render_pass);
            // draw bulk entities
            let asset_server = self.asset_server.read().unwrap();

            render_pass
                .set_vertex_buffer(Constant::VERTEX_ID_INSTANCES, self.instance_buffer.slice());
            for (material_id, render_items) in &self.material_renderitem_map {
                // Get material from asset_server
                if let Some(material_instance) =
                    asset_server.get_material_instance_by_id(material_id)
                {
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
        }
        // Shadowmap Pass
        {
            // for light in lights {
            // let mut shadow_pass = self.create_shadow_pass(encoder, framebuffer)?;
            //     // draw shadow with bulk light
            // }
        }
        // Deferred-Lighting Pass
        {
            let mut render_pass = self.create_lighting_pass(encoder, framebuffer)?;
            camera_data.encode_view_params(&mut render_pass);
            framebuffer.encode(&mut render_pass);
            // lights.encode(&mut render_pass);
            // shadowmaps.encode(&mut render_pass);

            // draw deferred light with gbuffer, shadowmap and bulk light
            camera_data.deferred_lighting.encode(&mut render_pass);
        }
        {
            // Future work
            // let upscale_pass =
            // let taa_pass =
        }

        encoder.copy_texture_to_texture(
            camera_data.get_copy_from(),
            camera_data.get_copy_to(),
            camera_data.get_copy_size(),
        );

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
            // timestamp_writes: Some(RenderPassTimestampWrites {
            //     query_set: &self.query_set,
            //     beginning_of_pass_write_index: Some(0),
            //     end_of_pass_write_index: Some(1),
            // }),
            ..Default::default()
        });

        Ok(render_pass)
    }

    fn create_shadow_pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        // light: LightData,
        framebuffer: &Framebuffer,
    ) -> anyhow::Result<wgpu::RenderPass<'e>> {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[],         // &light.shadowmap_attachments()?
            depth_stencil_attachment: None, // we using shadowmap with format Rg32Float. So we're not using depth stencil but color attachment
            ..Default::default()
        });

        Ok(render_pass)
    }

    fn create_lighting_pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        framebuffer: &Framebuffer,
    ) -> anyhow::Result<wgpu::RenderPass<'e>> {
        let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &framebuffer.final_color_attachments()?,
            depth_stencil_attachment: None,
            // timestamp_writes: Some(RenderPassTimestampWrites {
            //     query_set: &self.query_set,
            //     beginning_of_pass_write_index: Some(2),
            //     end_of_pass_write_index: Some(3),
            // }),
            ..Default::default()
        });

        Ok(render_pass)
    }
}
