use log::debug;
use wgpu::{
    include_wgsl, FragmentState, MultisampleState, PipelineCompilationOptions,
    PipelineLayoutDescriptor, RenderPipelineDescriptor, VertexState,
};

use crate::{GraphicsInstance, RenderTargetTexture};
use std::{num::NonZeroU32, sync::Arc};

use super::Postproc;

pub fn create_deferred_lighting_proc(
    graphics_instance: Arc<GraphicsInstance>,
    view_count: u32,
    bind_group_layout: &wgpu::BindGroupLayout,
    output: &RenderTargetTexture,
) -> anyhow::Result<Postproc> {
    let device = graphics_instance.device();
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    // let mut composer = Composer::default()
    //     .with_capabilities(Capabilities::MULTIVIEW | Capabilities::PUSH_CONSTANT);
    // let mut defs = HashMap::new();
    // defs.insert(
    //     "VIEW_COUNT".to_owned(),
    //     naga_oil::compose::ShaderDefValue::UInt(view_count),
    // );
    // debug!("defs.insert()");
    // debug!(
    //     "Shader source: {}",
    //     include_str!("shader/deferred_lighting.wgsl")
    // );
    // let naga_module = composer.make_naga_module(NagaModuleDescriptor {
    //     source: include_str!("shader/deferred_lighting.wgsl"),
    //     file_path: "shader/deferred_lighting.wgsl",
    //     shader_type: naga_oil::compose::ShaderType::Wgsl,
    //     shader_defs: defs,
    //     additional_imports: &[],
    // })?;
    // debug!("composer.make_naga_module()");
    // let module = device.create_shader_module(ShaderModuleDescriptor {
    //     label: None,
    //     source: wgpu::ShaderSource::Naga(Cow::Owned(naga_module)),
    // });
    // debug!("device.create_shader_module()");
    let module = device.create_shader_module(include_wgsl!("../shader/deferred_lighting.wgsl"));

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
        multiview: if view_count > 0 {
            NonZeroU32::new(view_count)
        } else {
            None
        },
        cache: graphics_instance.pipeline_cache(),
    });
    debug!("device.create_render_pipeline()");

    Ok(Postproc::new(pipeline))
}
