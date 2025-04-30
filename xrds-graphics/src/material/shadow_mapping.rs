use std::collections::HashMap;

use wgpu::{
    DepthStencilState, FragmentState, MultisampleState, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PushConstantRange, RenderPipelineDescriptor, ShaderStages,
    VertexAttribute, VertexBufferLayout, VertexState, VertexStepMode,
};

use crate::GraphicsInstance;

use super::preprocessor::{Preprocessor, ShaderValue};

#[derive(Debug, Clone)]
pub struct ShadowMapping {
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
}

impl ShadowMapping {
    pub fn new(
        graphics_instance: &GraphicsInstance,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<ShadowMapping> {
        let device = graphics_instance.device();

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("ShadowMapping"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[PushConstantRange {
                range: 0..std::mem::size_of::<glam::Mat4>() as _,
                stages: ShaderStages::VERTEX,
            }],
        });

        let mut preprocessor = Preprocessor::default();
        let mut defs = HashMap::default();
        defs.insert("VERTEX_INPUT_POSITION".to_owned(), ShaderValue::Def);
        defs.insert("SHADOW_MAPPING".to_owned(), ShaderValue::Def);

        preprocessor.add_include_module("common::utils", include_str!("shader/common/utils.wgsl"));
        preprocessor.add_include_module(
            "common::light_params",
            include_str!("shader/common/light_params.wgsl"),
        );
        preprocessor.add_include_module(
            "common::skinning",
            include_str!("shader/common/skinning.wgsl"),
        );
        preprocessor.add_include_module(
            "postproc::types",
            include_str!("shader/postproc/types.wgsl"),
        );
        preprocessor.add_include_module(
            "pbr::vertex_params",
            include_str!("shader/pbr/vertex_params.wgsl"),
        );
        let vertex_descriptor = preprocessor
            .build(
                include_str!("shader/pbr/vertex.wgsl"),
                &defs,
                Some("vertex.wgsl"),
            )
            .unwrap();
        let fragment_descriptor = preprocessor
            .build(
                include_str!("shader/pbr/shadow_mapping.wgsl"),
                &defs,
                Some("shadow_mapping.wgsl"),
            )
            .unwrap();

        let vertex_module = device.create_shader_module(vertex_descriptor);
        let fragment_module = device.create_shader_module(fragment_descriptor);

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("ShadowMapping"),
            vertex: VertexState {
                module: &vertex_module,
                entry_point: None,
                buffers: &[
                    VertexBufferLayout {
                        step_mode: VertexStepMode::Instance,
                        array_stride: std::mem::size_of::<glam::Mat4>() as _,
                        attributes: &[
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 0,
                                shader_location: 10,
                            },
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: std::mem::size_of::<[f32; 4]>() as u64,
                                shader_location: 11,
                            },
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: std::mem::size_of::<[f32; 8]>() as u64,
                                shader_location: 12,
                            },
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: std::mem::size_of::<[f32; 12]>() as u64,
                                shader_location: 13,
                            },
                        ],
                    },
                    VertexBufferLayout {
                        step_mode: VertexStepMode::Instance,
                        array_stride: std::mem::size_of::<glam::Mat4>() as _,
                        attributes: &[
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 0,
                                shader_location: 14,
                            },
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: std::mem::size_of::<[f32; 4]>() as u64,
                                shader_location: 15,
                            },
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: std::mem::size_of::<[f32; 8]>() as u64,
                                shader_location: 16,
                            },
                            VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: std::mem::size_of::<[f32; 12]>() as u64,
                                shader_location: 17,
                            },
                        ],
                    },
                    VertexBufferLayout {
                        step_mode: VertexStepMode::Vertex,
                        array_stride: std::mem::size_of::<glam::Vec3>() as _,
                        attributes: &[VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    }, // Position
                ], // Instance
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &fragment_module,
                entry_point: None,
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rg32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            depth_stencil: Some(DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            layout: Some(&pipeline_layout),
            multisample: MultisampleState::default(),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            multiview: None,
            cache: graphics_instance.pipeline_cache(),
        });

        Ok(Self {
            pipeline_layout,
            pipeline,
        })
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn pipeline_layout(&self) -> &wgpu::PipelineLayout {
        &self.pipeline_layout
    }
}
