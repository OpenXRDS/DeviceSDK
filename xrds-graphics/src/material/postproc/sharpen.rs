use std::collections::HashMap;

use wgpu::RenderPass;

use crate::{preprocessor::Preprocessor, BindGroupLayoutHelper, Constant, GraphicsInstance};

#[derive(Debug, Clone)]
pub struct SharpenProc {
    pipeline: wgpu::RenderPipeline,
}

impl SharpenProc {
    pub fn new(graphics_instance: &GraphicsInstance) -> anyhow::Result<Self> {
        let device = graphics_instance.device();

        let bind_group_layout = BindGroupLayoutHelper::create_intermediate(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let mut preprocessor = Preprocessor::default();
        preprocessor.add_include_module(
            "postproc::types",
            include_str!("../shader/postproc/types.wgsl"),
        );
        let defs = HashMap::new();

        let vertex_descriptor = preprocessor
            .build(
                include_str!("../shader/postproc/simple_quad.wgsl"),
                &defs,
                Some("simple_quad.wgsl"),
            )
            .unwrap();
        let fragment_descriptor = preprocessor
            .build(
                include_str!("../shader/postproc/sharpen.wgsl"),
                &defs,
                Some("sharpen.wgsl"),
            )
            .unwrap();

        let vertex_module = device.create_shader_module(vertex_descriptor);
        let fragment_module = device.create_shader_module(fragment_descriptor);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sharpen"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_module,
                entry_point: None,
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_module,
                entry_point: None,
                targets: &[Some(wgpu::ColorTargetState {
                    format: Constant::INTERMEDIATE_RENDER_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: graphics_instance.multiview(),
            cache: graphics_instance.pipeline_cache(),
        });

        Ok(Self { pipeline })
    }

    pub fn encode(&self, render_pass: &mut RenderPass) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.draw(0..3, 0..1);
    }
}
