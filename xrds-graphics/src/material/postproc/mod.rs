mod copy_swapchain;
mod deferred_lighting;
mod taa;

pub use copy_swapchain::*;
pub use deferred_lighting::*;
pub use taa::*;

use wgpu::RenderPass;

#[derive(Debug, Clone)]
pub struct Binding {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    binding_index: u32,
    offsets: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Postproc {
    pipeline: wgpu::RenderPipeline,
    input_binding: Option<Binding>,
    output_binding: Option<Binding>,
}

impl Postproc {
    pub fn new(
        pipeline: wgpu::RenderPipeline,
        input_binding: Option<Binding>,
        output_binding: Option<Binding>,
    ) -> Self {
        Self {
            pipeline,
            input_binding,
            output_binding,
        }
    }

    pub fn encode(&self, render_pass: &mut RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipeline);
        if let Some(input_binding) = &self.input_binding {
            render_pass.set_bind_group(
                input_binding.binding_index,
                &input_binding.bind_group,
                &input_binding.offsets,
            );
        }
        render_pass.draw(0..3, 0..1);
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn input_binding(&self) -> Option<&Binding> {
        self.input_binding.as_ref()
    }

    pub fn output_bindding(&self) -> Option<&Binding> {
        self.output_binding.as_ref()
    }

    pub fn input_binding_mut(&mut self) -> Option<&mut Binding> {
        self.input_binding.as_mut()
    }

    pub fn output_binding_mut(&mut self) -> Option<&mut Binding> {
        self.output_binding.as_mut()
    }

    pub fn create_output_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}
