use std::collections::HashMap;

use crate::{
    preprocessor::Preprocessor, BindGroupLayoutHelper, GraphicsInstance, TextureFormat, XrdsTexture,
};

#[derive(Debug, Clone)]
pub struct CopySwapchainProc {
    pipeline: wgpu::RenderPipeline,
    target: Option<XrdsTexture>,
}

impl CopySwapchainProc {
    pub fn new(
        graphics_instance: &GraphicsInstance,
        format: TextureFormat,
    ) -> anyhow::Result<Self> {
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
                include_str!("../shader/postproc/copy_swapchain.wgsl"),
                &defs,
                Some("copy_swapchain.wgsl"),
            )
            .unwrap();

        let vertex_module = device.create_shader_module(vertex_descriptor);
        let fragment_module = device.create_shader_module(fragment_descriptor);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("CopySwapchainProc"),
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
                    format: format.as_wgpu(),
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

        Ok(Self {
            pipeline,
            target: None,
        })
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn target_view(&self) -> Option<&wgpu::TextureView> {
        self.target.as_ref().map(|t| t.view())
    }

    pub fn set_target_view(&mut self, target: &XrdsTexture) {
        self.target = Some(target.clone());
    }

    pub fn encode(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.draw(0..3, 0..1);
    }
}
