use std::{collections::HashMap, sync::Arc};

use wgpu::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, FragmentState,
    MultisampleState, PipelineCompilationOptions, PipelineLayoutDescriptor,
    RenderPipelineDescriptor, SamplerBindingType, ShaderStages, TextureSampleType,
    TextureViewDimension, VertexState,
};

use crate::{preprocessor::Preprocessor, GraphicsInstance};

use super::Postproc;

pub fn create_shadow_proc(graphics_instance: Arc<GraphicsInstance>) -> anyhow::Result<Postproc> {
    let device = graphics_instance.device();

    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("ShadowBindGroupLayout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let mut preprocessor = Preprocessor::default();
    let defs = HashMap::default();

    preprocessor.add_include_module(
        "light_params",
        include_str!("../shader/common/light_params.wgsl"),
    );
    preprocessor.add_include_module(
        "postproc::types",
        include_str!("../shader/postproc/types.wgsl"),
    );
    let vertex_descriptor = preprocessor.build(
        include_str!("../shader/postproc/simple_quad.wgsl"),
        &defs,
        Some("simple_quad.wgsl"),
    )?;
    let fragment_descriptor = preprocessor.build(
        include_str!("../shader/postproc/shadow_mapping.wgsl"),
        &defs,
        Some("shadow_mapping.wgsl"),
    )?;

    let vertex_module = device.create_shader_module(vertex_descriptor);
    let fragment_module = device.create_shader_module(fragment_descriptor);

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("ShadowMapping"),
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
                format: wgpu::TextureFormat::Rg32Float,
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
