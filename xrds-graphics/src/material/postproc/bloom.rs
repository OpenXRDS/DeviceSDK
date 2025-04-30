use std::collections::HashMap;

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroupLayoutDescriptor, BlendComponent, BlendState, BufferUsages, Operations,
    PipelineLayoutDescriptor, RenderPassColorAttachment, RenderPipelineDescriptor,
};

use crate::{
    preprocessor::{Preprocessor, ShaderValue},
    BindGroupLayoutHelper, Constant, Framebuffer, GraphicsInstance,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BloomParams {
    thredhold: f32,
    intensity: f32,
    knee_width: f32,
    _padding: u32,
}

#[derive(Debug, Clone)]
pub struct BloomProc {
    params_uniform: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    brightness_pipeline: wgpu::RenderPipeline,
    downsampling_pipeline: wgpu::RenderPipeline,
    horizontal_blurring_pipeline: wgpu::RenderPipeline,
    vertical_blurring_pipeline: wgpu::RenderPipeline,
    upsampling_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
}

impl BloomProc {
    pub fn new(graphics_instance: &GraphicsInstance) -> anyhow::Result<Self> {
        let device = graphics_instance.device();

        // Create params uniform
        let params_uniform = Self::create_uniform_buffer_with_default_params(device);
        let params_bind_group_layout = Self::create_bloom_params_bind_group_layout(device);
        let params_bind_group = Self::create_bloom_params_bind_group(
            device,
            &params_bind_group_layout,
            &params_uniform,
        );

        let texture_input_bind_group_layout = BindGroupLayoutHelper::create_intermediate(device);
        let single_input_bind_group_layouts =
            [&texture_input_bind_group_layout, &params_bind_group_layout];
        let mut defs = HashMap::new();

        let brightness_pipeline = Self::create_pipeline(
            graphics_instance,
            &single_input_bind_group_layouts,
            "Bloom-Brightness",
            "brightness_main",
            None,
            &defs,
        )?;

        let downsampling_pipeline = Self::create_pipeline(
            graphics_instance,
            &single_input_bind_group_layouts,
            "Bloom-Downsampling",
            "downsample_main",
            None,
            &defs,
        )?;

        let horizontal_blurring_pipeline = Self::create_pipeline(
            graphics_instance,
            &single_input_bind_group_layouts,
            "Bloom-BlurHorizontal",
            "blur_horizontal_main",
            None,
            &defs,
        )?;

        let vertical_blurring_pipeline = Self::create_pipeline(
            graphics_instance,
            &single_input_bind_group_layouts,
            "Bloom-BlurVertical",
            "blur_vertical_main",
            None,
            &defs,
        )?;

        let upsampling_pipeline = Self::create_pipeline(
            graphics_instance,
            &single_input_bind_group_layouts,
            "Bloom-Upsampling",
            "upsample_main",
            Some(BlendState {
                color: BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: BlendComponent::REPLACE,
            }),
            &defs,
        )?;

        defs.insert("COMPOSITE_PASS".to_owned(), ShaderValue::Def);
        let composite_pipeline = Self::create_pipeline(
            graphics_instance,
            &[
                &texture_input_bind_group_layout,
                &params_bind_group_layout,
                &texture_input_bind_group_layout,
            ],
            "Bloom-Composite",
            "composite_main",
            None,
            &defs,
        )?;

        Ok(Self {
            params_uniform,
            params_bind_group,
            brightness_pipeline,
            downsampling_pipeline,
            horizontal_blurring_pipeline,
            vertical_blurring_pipeline,
            upsampling_pipeline,
            composite_pipeline,
        })
    }

    pub fn update_bloom_params(&self, queue: &wgpu::Queue, params: BloomParams) {
        queue.write_buffer(&self.params_uniform, 0, bytemuck::cast_slice(&[params]));
    }

    pub fn encode_bloom(&self, encoder: &mut wgpu::CommandEncoder, framebuffer: &Framebuffer) {
        encoder.push_debug_group("Bloom");
        {
            // 1. brightness + downsample
            // input: current framebuffer color
            // output: downsample_buffer[0]
            self.encode_brightness(encoder, framebuffer);

            for i in 1..framebuffer.downsample_level() {
                // 2. downsample
                // input: downsample_buffer[i-1]
                // output: downsample_buffer[i]
                self.encode_downsample(encoder, framebuffer, i);
            }

            for i in 0..framebuffer.downsample_level() {
                // 3. blur
                // horizontal
                // input: downsample_buffer[i]
                // output: blur_buffer[i]
                // vertical
                // input: blur_buffer[i]
                // output: downsample_buffer[i]
                self.encode_blur(encoder, framebuffer, i);
            }

            for i in (1..framebuffer.downsample_level()).rev() {
                // 4. upsample
                // input: downsample_buffer[i]
                // output: downsample_buffer[i-1]
                self.encode_upsample(encoder, framebuffer, i);
            }

            // 5. composite
            // input0: framebuffer input
            // input1: downsample_buffer[0]
            // output: framebuffer output
            self.encode_composite(encoder, framebuffer);
        }
        encoder.pop_debug_group();
    }

    fn encode_brightness(&self, encoder: &mut wgpu::CommandEncoder, framebuffer: &Framebuffer) {
        encoder.push_debug_group("Bloom-Brightness");
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom-Brightness"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: framebuffer.bloom_downsample_target(0).texture().view(),
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            render_pass.set_pipeline(&self.brightness_pipeline);
            framebuffer.encode_input(&mut render_pass, 0);
            render_pass.set_bind_group(1, &self.params_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
        encoder.pop_debug_group();
    }

    fn encode_downsample(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        framebuffer: &Framebuffer,
        index: usize,
    ) {
        encoder.push_debug_group("Bloom-downsample");
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom-downsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: framebuffer.bloom_downsample_target(index).texture().view(),
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            render_pass.set_pipeline(&self.downsampling_pipeline);
            framebuffer.encode_bloom_downsample(&mut render_pass, (index - 1).max(0), 0);
            render_pass.set_bind_group(1, &self.params_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
        encoder.pop_debug_group();
    }

    fn encode_blur(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        framebuffer: &Framebuffer,
        index: usize,
    ) {
        encoder.push_debug_group("Bloom-blur");
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom-blur-horizontal"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: framebuffer.bloom_blur_target(index).texture().view(),
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            render_pass.set_pipeline(&self.horizontal_blurring_pipeline);
            framebuffer.encode_bloom_downsample(&mut render_pass, index, 0);
            render_pass.set_bind_group(1, &self.params_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom-blur-vertical"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: framebuffer.bloom_downsample_target(index).texture().view(),
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            render_pass.set_pipeline(&self.vertical_blurring_pipeline);
            framebuffer.encode_bloom_blur(&mut render_pass, index, 0);
            render_pass.set_bind_group(1, &self.params_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
        encoder.pop_debug_group();
    }

    fn encode_upsample(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        framebuffer: &Framebuffer,
        index: usize,
    ) {
        encoder.push_debug_group("Bloom-downsample");
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom-downsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: framebuffer
                        .bloom_downsample_target(index - 1)
                        .texture()
                        .view(),
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            render_pass.set_pipeline(&self.upsampling_pipeline);
            framebuffer.encode_bloom_downsample(&mut render_pass, index, 0);
            render_pass.set_bind_group(1, &self.params_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
        encoder.pop_debug_group();
    }

    pub fn encode_composite(&self, encoder: &mut wgpu::CommandEncoder, framebuffer: &Framebuffer) {
        encoder.push_debug_group("Bloom-composite");
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom-composite"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: framebuffer.output_target().texture().view(),
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            render_pass.set_pipeline(&self.composite_pipeline);
            framebuffer.encode_input(&mut render_pass, 0);
            render_pass.set_bind_group(1, &self.params_bind_group, &[]);
            framebuffer.encode_bloom_downsample(&mut render_pass, 0, 2);
            render_pass.draw(0..3, 0..1);
        }
        encoder.pop_debug_group();
    }

    fn create_uniform_buffer_with_default_params(device: &wgpu::Device) -> wgpu::Buffer {
        let params = BloomParams::default();
        device.create_buffer_init(&BufferInitDescriptor {
            label: Some("BloomParams"),
            contents: bytemuck::bytes_of(&params),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
    }

    fn create_bloom_params_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("BloomParams"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn create_bloom_params_bind_group(
        device: &wgpu::Device,
        bloom_params_bind_group_layout: &wgpu::BindGroupLayout,
        bloom_params_uniform: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BloomParams"),
            layout: bloom_params_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bloom_params_uniform.as_entire_binding(),
            }],
        })
    }

    fn create_pipeline(
        graphics_instance: &GraphicsInstance,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        label: &str,
        entry_point: &str,
        blend_state: Option<BlendState>,
        defs: &HashMap<String, ShaderValue>,
    ) -> anyhow::Result<wgpu::RenderPipeline> {
        let device = graphics_instance.device();
        let mut preprocessor = Preprocessor::default();

        preprocessor.add_include_module(
            "postproc::types",
            include_str!("../shader/postproc/types.wgsl"),
        );
        let vertex_descriptor = preprocessor.build(
            include_str!("../shader/postproc/simple_quad.wgsl"),
            defs,
            Some("simple_quad.wgsl"),
        )?;
        let fragment_descriptor = preprocessor.build(
            include_str!("../shader/postproc/bloom.wgsl"),
            defs,
            Some("bloom.wgsl"),
        )?;

        let vertex_module = device.create_shader_module(vertex_descriptor);
        let fragment_module = device.create_shader_module(fragment_descriptor);
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts,
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(label),
            vertex: wgpu::VertexState {
                module: &vertex_module,
                entry_point: None,
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_module,
                entry_point: Some(entry_point),
                targets: &[Some(wgpu::ColorTargetState {
                    format: Constant::INTERMEDIATE_RENDER_FORMAT,
                    blend: blend_state,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            depth_stencil: None,
            layout: Some(&pipeline_layout),
            primitive: wgpu::PrimitiveState::default(),
            multisample: wgpu::MultisampleState::default(),
            multiview: graphics_instance.multiview(),
            cache: graphics_instance.pipeline_cache(),
        });

        Ok(pipeline)
    }
}

impl Default for BloomParams {
    fn default() -> Self {
        Self {
            thredhold: 1.0,
            intensity: 0.2,
            knee_width: 1.0,
            _padding: 0,
        }
    }
}
