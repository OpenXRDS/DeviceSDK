use bevy::{
    camera::ManualTextureViewHandle,
    prelude::*,
    render::{
        graph::CameraDriverLabel,
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        renderer::{RenderContext, RenderDevice},
        texture::ManualTextureViews,
        view::ExtractedWindows,
        RenderApp,
    },
};

use crate::openxr::{schedule::OpenXrSessionState, swapchain::view_index};

pub struct OpenXrBlitPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct XrBlitLabel;

impl Plugin for OpenXrBlitPlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
        graph.add_node(XrBlitLabel, XrBlitNode { pipeline: None });
        graph.add_node_edge(CameraDriverLabel, XrBlitLabel);
    }
}

struct XrBlitNode {
    pipeline: Option<BlitPipelineInner>,
}

struct BlitPipelineInner {
    pipeline:          wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler:           wgpu::Sampler,
    window_format:     wgpu::TextureFormat,
}

impl BlitPipelineInner {
    fn new(device: &wgpu::Device, window_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("xr_blit_shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("xr_blit_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty:         wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty:         wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count:      None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("xr_blit_layout"),
            bind_group_layouts:   &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("xr_blit_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module:              &shader,
                entry_point:         Some("vs_main"),
                compilation_options: Default::default(),
                buffers:             &[],
            },
            fragment: Some(wgpu::FragmentState {
                module:              &shader,
                entry_point:         Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     window_format,
                    blend:      None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive:     wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self { pipeline, bind_group_layout: bgl, sampler, window_format }
    }
}

impl Node for XrBlitNode {
    fn update(&mut self, world: &mut World) {
        let windows = world.resource::<ExtractedWindows>();
        let Some(primary) = windows.primary else { return };
        let Some(window) = windows.windows.get(&primary) else { return };
        let Some(fmt) = window.swap_chain_texture_format else { return };

        if self.pipeline.as_ref().map(|p| p.window_format) == Some(fmt) {
            return;
        }

        let device = world.resource::<RenderDevice>();
        self.pipeline = Some(BlitPipelineInner::new(device.wgpu_device(), fmt));
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let session_state = world.resource::<OpenXrSessionState>();
        if *session_state != OpenXrSessionState::Running {
            return Ok(());
        }

        let Some(pip) = &self.pipeline else {
            return Ok(());
        };

        let manual_views = world.resource::<ManualTextureViews>();
        let windows = world.resource::<ExtractedWindows>();

        let left_handle = ManualTextureViewHandle(view_index(0));
        let Some(src) = manual_views.get(&left_handle) else {
            return Ok(());
        };
        let Some(primary) = windows.primary else {
            return Ok(());
        };
        let Some(window) = windows.windows.get(&primary) else {
            return Ok(());
        };
        let Some(dst_view) = &window.swap_chain_texture_view else {
            return Ok(());
        };

        let wgpu_device = render_context.render_device().wgpu_device();

        let bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("xr_blit_bg"),
            layout:  &pip.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(&src.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&pip.sampler),
                },
            ],
        });

        let mut encoder = wgpu_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("xr_blit_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label:                    Some("xr_blit_pass"),
                color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
                    view:           dst_view,
                    resolve_target: None,
                    depth_slice:    None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes:         None,
                occlusion_query_set:      None,
            });
            pass.set_pipeline(&pip.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        render_context.add_command_buffer(encoder.finish());
        Ok(())
    }
}

const BLIT_WGSL: &str = r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_smp: sampler;

struct V2F {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> V2F {
    // Fullscreen triangle: NDC covers [-1,1] x [-1,1]
    let uv  = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    // Y is flipped: NDC +1 = screen top = texture V=0
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    return V2F(vec4<f32>(ndc, 0.0, 1.0), uv);
}

@fragment
fn fs_main(v: V2F) -> @location(0) vec4<f32> {
    return textureSample(src_tex, src_smp, v.uv);
}
"#;
