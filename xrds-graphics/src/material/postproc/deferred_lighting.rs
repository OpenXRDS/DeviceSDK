use wgpu::{
    FragmentState, MultisampleState, PipelineCompilationOptions, PipelineLayoutDescriptor,
    RenderPass, RenderPipelineDescriptor, VertexState,
};

use crate::{
    preprocessor::{Preprocessor, ShaderValue},
    BindGroupLayoutHelper, Constant, GraphicsInstance,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DeferredLightingProc {
    pipeline: wgpu::RenderPipeline,
}

impl DeferredLightingProc {
    pub fn new(graphics_instance: &GraphicsInstance) -> anyhow::Result<Self> {
        let device = graphics_instance.device();

        let view_params_bgl = BindGroupLayoutHelper::create_view_params(device);
        let gbuffer_params_bgl = BindGroupLayoutHelper::create_gbuffer_params(device);
        let light_params_bgl = BindGroupLayoutHelper::create_light_params(device);

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&view_params_bgl, &gbuffer_params_bgl, &light_params_bgl],
            push_constant_ranges: &[],
        });

        let mut preprocessor = Preprocessor::default();
        let defs = HashMap::from([
            ("VIEW_PARAMS_GROUP_INDEX".to_owned(), ShaderValue::Int(0)),
            ("GBUFFER_PARAMS_GROUP_INDEX".to_owned(), ShaderValue::Int(1)),
            ("LIGHT_PARAMS_GROUP_INDEX".to_owned(), ShaderValue::Int(2)),
        ]);

        preprocessor
            .add_include_module("common::utils", include_str!("../shader/common/utils.wgsl"));
        preprocessor.add_include_module(
            "common::view_params",
            include_str!("../shader/common/view_params.wgsl"),
        );
        preprocessor.add_include_module(
            "pbr::gbuffer_params",
            include_str!("../shader/pbr/gbuffer_params.wgsl"),
        );
        preprocessor.add_include_module(
            "postproc::types",
            include_str!("../shader/postproc/types.wgsl"),
        );
        preprocessor.add_include_module(
            "common::light_params",
            include_str!("../shader/common/light_params.wgsl"),
        );
        let vertex_descriptor = preprocessor
            .build(
                include_str!("../shader/postproc/simple_quad.wgsl"),
                &defs,
                Some("simple_quad.wgsl"),
            )
            .unwrap();
        let fragment_descriptor = preprocessor
            .build(
                include_str!("../shader/postproc/deferred_lighting.wgsl"),
                &defs,
                Some("deferred_lighting.wgsl"),
            )
            .unwrap();

        let vertex_module = device.create_shader_module(vertex_descriptor);
        let fragment_module = device.create_shader_module(fragment_descriptor);

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("DeferredLighting"),
            vertex: VertexState {
                module: &vertex_module,
                entry_point: None,
                buffers: &[],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &fragment_module,
                entry_point: None,
                targets: &[Some(wgpu::ColorTargetState {
                    format: Constant::INTERMEDIATE_RENDER_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            depth_stencil: None,
            layout: Some(&pipeline_layout),
            multisample: MultisampleState::default(),
            primitive: wgpu::PrimitiveState::default(),
            multiview: graphics_instance.multiview(),
            cache: graphics_instance.pipeline_cache(),
        });

        Ok(Self { pipeline })
    }

    pub fn encode(&self, render_pass: &mut RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.draw(0..3, 0..1);
    }
}
