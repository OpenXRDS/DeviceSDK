use log::debug;
use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};
use wgpu::{
    naga::valid::Capabilities, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType,
    BufferBindingType, FragmentState, MultisampleState, PipelineCompilationOptions,
    PipelineLayoutDescriptor, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStages,
    VertexState,
};

use crate::{GraphicsInstance, RenderTargetTexture, ViewParams};
use std::{
    borrow::Cow,
    collections::HashMap,
    num::{NonZeroU32, NonZeroU64},
    sync::Arc,
};

use super::Postproc;

pub fn create_deferred_lighting_proc(
    graphics_instance: Arc<GraphicsInstance>,
    gbuffer_bind_group_layout: &wgpu::BindGroupLayout,
    output: &RenderTargetTexture,
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
                min_binding_size: NonZeroU64::new((std::mem::size_of::<ViewParams>() * 2) as u64),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&view_bind_group_layout, gbuffer_bind_group_layout],
        push_constant_ranges: &[],
    });

    let mut composer = Composer::default().with_capabilities(Capabilities::all());
    composer.validate = false;
    log::debug!("composer::default()");
    let defs = HashMap::new();
    composer.add_composable_module(ComposableModuleDescriptor {
        source: include_str!("../shader/view_params.wgsl"),
        file_path: "../shader/view_params.wgsl",
        ..Default::default()
    })?;
    log::debug!("composer.add_composable_module()");
    let naga_module = composer.make_naga_module(NagaModuleDescriptor {
        source: include_str!("../shader/postproc/deferred_lighting.wgsl"),
        file_path: "../shader/postproc/deferred_lighting.wgsl",
        shader_type: naga_oil::compose::ShaderType::Wgsl,
        shader_defs: defs,
        additional_imports: &[],
    });
    log::debug!("composer.make_naga_module()");
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("shader/deferred_lighting.wgsl"),
        source: wgpu::ShaderSource::Naga(Cow::Owned(naga_module.unwrap())),
    });
    debug!("device.create_shader_module()");

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        vertex: VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: PipelineCompilationOptions::default(),
        },
        fragment: Some(FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: output.texture().format().as_wgpu(),
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: PipelineCompilationOptions::default(),
        }),
        depth_stencil: None,
        layout: Some(&pipeline_layout),
        multisample: MultisampleState::default(),
        primitive: wgpu::PrimitiveState::default(),
        multiview: NonZeroU32::new(2),
        cache: graphics_instance.pipeline_cache(),
    });
    debug!("device.create_render_pipeline()");

    Ok(Postproc::new(pipeline))
}
