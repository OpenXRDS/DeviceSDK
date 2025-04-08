use wgpu::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, FragmentState,
    MultisampleState, PipelineCompilationOptions, PipelineLayoutDescriptor,
    RenderPipelineDescriptor, ShaderStages, VertexState,
};

use crate::{preprocessor::Preprocessor, GraphicsInstance, TextureFormat};
use std::collections::HashMap;

use super::Postproc;

pub fn create_deferred_lighting_proc(
    graphics_instance: &GraphicsInstance,
    gbuffer_bind_group_layout: &wgpu::BindGroupLayout,
    output_format: TextureFormat,
) -> anyhow::Result<Postproc> {
    let device = graphics_instance.device();

    // TODO: resuable
    let view_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("ViewProjectionBindings"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&view_bind_group_layout, gbuffer_bind_group_layout],
        push_constant_ranges: &[],
    });

    let mut preprocessor = Preprocessor::default();
    let defs = HashMap::default();

    preprocessor.add_include_module(
        "common::view_params",
        include_str!("../shader/common/view_params.wgsl"),
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
                format: output_format.as_wgpu(),
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

    Ok(Postproc::new(pipeline))
}
