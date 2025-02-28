use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{Arc, RwLock},
};

use uuid::Uuid;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType,
    ColorTargetState, ColorWrites, DepthBiasState, DepthStencilState, FragmentState,
    MultisampleState, PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState,
    RenderPipelineDescriptor, SamplerBindingType, ShaderStages, StencilState, TextureDescriptor,
    TextureSampleType, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexState,
};

use crate::{
    buffer::XrdsBufferType,
    pbr::{Options, PbrMaterialInputOption, PbrShaderBuilder},
    GraphicsInstance, TextureFormat, XrdsBuffer, XrdsMaterial, XrdsTexture, XrdsVertexBuffer,
};

use super::types::{AssetHandle, AssetId, AssetStrongHandle};

pub struct AssetServer {
    graphics_instance: Arc<GraphicsInstance>,
    resource_buffer: Arc<RwLock<ResourceBuffer>>,
    shader_builder: PbrShaderBuilder,
}

#[derive(Default)]
pub struct ResourceBuffer {
    textures: HashMap<AssetId, AssetStrongHandle<XrdsTexture>>,
    buffers: HashMap<AssetId, AssetStrongHandle<XrdsBuffer>>,
    materials: HashMap<AssetId, AssetStrongHandle<XrdsMaterial>>,
}

#[derive(Debug)]
pub struct TextureAssetInfo<'a> {
    pub id: &'a AssetId,
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub depth_or_array: u32,
}

#[derive(Debug)]
pub struct BufferAssetInfo<'a> {
    pub id: &'a AssetId,
    pub data: &'a [u8],
    pub ty: XrdsBufferType,
    pub stride: u64,
}

pub struct MaterialAssetInfo<'a> {
    pub id: &'a AssetId,
    pub options: &'a Options,
    pub vertex_buffers: &'a [XrdsVertexBuffer],
}

impl AssetServer {
    pub fn new(graphics_instance: Arc<GraphicsInstance>) -> anyhow::Result<Self> {
        let resource_buffer = Arc::new(RwLock::new(ResourceBuffer::default()));
        let shader_builder = PbrShaderBuilder::new()?;

        Ok(Self {
            graphics_instance,
            resource_buffer,
            shader_builder,
        })
    }

    /// Register new texture to asset server.
    /// Return existing or new weak handle
    /// ```
    /// let id: AssetId = asset_server.generate_id();
    /// let handle: AssetHandle<XrdsTexture> = asset_server.register_texture(&id, data, width, height, depth_or_array);
    ///
    /// let texture: Option<XrdsTexture> = asset_server.get_texture(&handle);
    /// ```
    pub fn register_texture(
        &self,
        info: &TextureAssetInfo,
    ) -> anyhow::Result<AssetHandle<XrdsTexture>> {
        {
            let lock = self.resource_buffer.read().unwrap();
            if let Some(handle) = lock.textures.get(info.id) {
                log::debug!(
                    "id '{:?}' already exists. Skip loading and return exsiting handle",
                    info.id
                );
                return Ok(handle.as_weak_handle());
            }
        }

        let label = match info.id {
            AssetId::Key(s) => s.clone(),
            AssetId::Uuid(u) => u.to_string(),
        };
        let size = wgpu::Extent3d {
            width: info.width,
            height: info.height,
            depth_or_array_layers: info.depth_or_array,
        };
        let texture = self.graphics_instance.device().create_texture_with_data(
            self.graphics_instance.queue(),
            &TextureDescriptor {
                label: Some(&label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: if info.depth_or_array == 1 {
                    wgpu::TextureDimension::D2
                } else {
                    wgpu::TextureDimension::D3
                },
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            info.data,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let xrds_texture = XrdsTexture::new(
            texture,
            TextureFormat::from(wgpu::TextureFormat::Rgba8Unorm),
            size,
            Some(view),
        );
        let handle = AssetStrongHandle::new(info.id.clone(), xrds_texture);
        let weak_handle = handle.as_weak_handle();
        let mut lock = self.resource_buffer.write().unwrap();
        lock.textures.insert(info.id.clone(), handle);

        Ok(weak_handle)
    }

    pub fn register_buffer(
        &self,
        info: &BufferAssetInfo,
    ) -> anyhow::Result<AssetHandle<XrdsBuffer>> {
        {
            let lock = self.resource_buffer.read().unwrap();
            if let Some(handle) = lock.buffers.get(info.id) {
                log::debug!(
                    "id '{:?}' already exists. Skip loading and return exsiting handle",
                    info.id
                );
                return Ok(handle.as_weak_handle());
            }
        }

        let label = match info.id {
            AssetId::Key(s) => s.clone(),
            AssetId::Uuid(u) => u.to_string(),
        };

        let buffer = self
            .graphics_instance
            .device()
            .create_buffer_init(&BufferInitDescriptor {
                label: Some(&label),
                contents: info.data,
                usage: info.ty.into(),
            });

        let xrds_buffer = XrdsBuffer::new(buffer, info.ty, info.stride);
        let handle = AssetStrongHandle::new(info.id.clone(), xrds_buffer);
        let weak_handle = handle.as_weak_handle();
        let mut lock = self.resource_buffer.write().unwrap();
        lock.buffers.insert(info.id.clone(), handle);

        Ok(weak_handle)
    }

    pub fn register_material(
        &self,
        info: &MaterialAssetInfo,
    ) -> anyhow::Result<AssetHandle<XrdsMaterial>> {
        {
            let lock = self.resource_buffer.read().unwrap();
            if let Some(handle) = lock.materials.get(info.id) {
                log::debug!(
                    "id '{:?}' already exists. Skip loading and return exsiting handle",
                    info.id
                );
                return Ok(handle.as_weak_handle());
            }
        }

        let label = match info.id {
            AssetId::Key(s) => s.clone(),
            AssetId::Uuid(u) => u.to_string(),
        };
        let device = self.graphics_instance.device().clone();

        let mut bind_group_layouts = Vec::new();
        bind_group_layouts.push(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: NonZeroU32::new(2),
            }],
        }));

        bind_group_layouts.push(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &Self::into_bind_group_layout_entries(&info.options.material_input),
        }));

        if info.options.vertex_input.weights_joints_0 {
            bind_group_layouts.push(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            }));
        };
        let bind_group_layouts_ref: Vec<_> = bind_group_layouts.iter().collect();

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            bind_group_layouts: &bind_group_layouts_ref,
            ..Default::default()
        });

        let mut vertex_layouts: Vec<_> = info
            .vertex_buffers
            .iter()
            .map(|vb| VertexBufferLayout {
                array_stride: vb.buffer.stride(),
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vb.vertex_attributes,
            })
            .collect();
        // Instance buffer layout
        vertex_layouts.push(VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 16]>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
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
        });

        let pipeline =
            self.graphics_instance
                .device()
                .create_render_pipeline(&RenderPipelineDescriptor {
                    label: Some(&label),
                    layout: Some(&pipeline_layout),
                    vertex: VertexState {
                        module: &self
                            .shader_builder
                            .build_vertex_module(&device, info.options)?,
                        buffers: &vertex_layouts,
                        compilation_options: PipelineCompilationOptions::default(),
                        entry_point: None,
                    },
                    fragment: Some(FragmentState {
                        module: &self
                            .shader_builder
                            .build_fragment_module(&device, info.options)?,
                        targets: &[
                            Some(ColorTargetState {
                                // position_metallic
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                blend: None,
                                write_mask: ColorWrites::all(),
                            }),
                            Some(ColorTargetState {
                                // normal_roughness
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                blend: None,
                                write_mask: ColorWrites::all(),
                            }),
                            Some(ColorTargetState {
                                // albedo_occlusion
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                blend: None,
                                write_mask: ColorWrites::all(),
                            }),
                            Some(ColorTargetState {
                                // emissive
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                blend: None,
                                write_mask: ColorWrites::all(),
                            }),
                        ],
                        compilation_options: PipelineCompilationOptions::default(),
                        entry_point: None,
                    }),
                    depth_stencil: Some(DepthStencilState {
                        format: wgpu::TextureFormat::Depth24PlusStencil8,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: StencilState::default(),
                        bias: DepthBiasState::default(),
                    }),
                    cache: self.graphics_instance.pipeline_cache(),
                    primitive: PrimitiveState {
                        cull_mode: if info.options.material_input.double_sided {
                            None
                        } else {
                            Some(wgpu::Face::Back)
                        },
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        front_face: wgpu::FrontFace::Ccw,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        ..Default::default()
                    },
                    multisample: MultisampleState {
                        ..Default::default()
                    },
                    multiview: NonZeroU32::new(info.options.view_count),
                });

        let xrds_material = XrdsMaterial { pipeline };
        let handle = AssetStrongHandle::new(info.id.clone(), xrds_material);
        let weak_handle = handle.as_weak_handle();
        let mut lock = self.resource_buffer.write().unwrap();
        lock.materials.insert(info.id.clone(), handle);

        Ok(weak_handle)
    }

    fn into_bind_group_layout_entries(
        material_option: &PbrMaterialInputOption,
    ) -> Vec<BindGroupLayoutEntry> {
        let mut res = Vec::new();
        // pbr_params uniform buffer
        res.push(BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        let add_texture_and_sampler =
            |res: &mut Vec<BindGroupLayoutEntry>,
             binding_start: u32,
             dimension: TextureViewDimension| {
                res.push(BindGroupLayoutEntry {
                    binding: binding_start,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: dimension,
                        multisampled: false,
                    },
                    count: None,
                });
                res.push(BindGroupLayoutEntry {
                    binding: binding_start + 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                });
            };
        if material_option.base_color {
            add_texture_and_sampler(&mut res, 1, TextureViewDimension::D2);
        }
        if material_option.emissive {
            add_texture_and_sampler(&mut res, 5, TextureViewDimension::D2);
        }
        if material_option.metallic_roughness {
            add_texture_and_sampler(&mut res, 7, TextureViewDimension::D2);
        }
        if material_option.normal {
            add_texture_and_sampler(&mut res, 9, TextureViewDimension::D2);
        }
        if material_option.occlusion {
            add_texture_and_sampler(&mut res, 11, TextureViewDimension::D2);
        }
        #[cfg(feature = "material_spec_gloss")]
        if material_option.diffuse {
            add_texture_and_sampler(&mut res, 3, TextureViewDimension::D2);
        }
        #[cfg(feature = "material_spec_gloss")]
        if material_option.specular_glossiness {
            add_texture_and_sampler(&mut res, 13, TextureViewDimension::D2);
        }
        #[cfg(feature = "material_ibl")]
        if material_option.ibl {
            // ibl diffuse
            add_texture_and_sampler(&mut res, 15, TextureViewDimension::Cube);
            // ibl specular
            add_texture_and_sampler(&mut res, 17, TextureViewDimension::Cube);
        }
        #[cfg(feature = "material_ibl")]
        if material_option.brdf {
            add_texture_and_sampler(&mut res, 19, TextureViewDimension::D2);
        }

        res
    }

    pub fn get_texture(&self, handle: &AssetHandle<XrdsTexture>) -> Option<XrdsTexture> {
        self.get_texture_by_id(handle.id())
    }

    pub fn get_texture_by_id(&self, id: &AssetId) -> Option<XrdsTexture> {
        let lock = self.resource_buffer.read().unwrap();
        lock.textures.get(id).map(|h| h.asset().clone())
    }

    pub fn get_texture_handle(&self, id: &AssetId) -> Option<AssetHandle<XrdsTexture>> {
        let lock = self.resource_buffer.read().unwrap();
        lock.textures.get(id).map(|h| h.as_weak_handle())
    }

    pub fn get_buffer(&self, handle: &AssetHandle<XrdsBuffer>) -> Option<XrdsBuffer> {
        self.get_buffer_by_id(handle.id())
    }

    pub fn get_buffer_by_id(&self, id: &AssetId) -> Option<XrdsBuffer> {
        let lock = self.resource_buffer.read().unwrap();
        lock.buffers.get(id).map(|h| h.asset().clone())
    }

    pub fn get_buffer_handle(&self, id: &AssetId) -> Option<AssetHandle<XrdsBuffer>> {
        let lock = self.resource_buffer.read().unwrap();
        lock.buffers.get(id).map(|h| h.as_weak_handle())
    }

    pub fn get_material(&self, handle: &AssetHandle<XrdsMaterial>) -> Option<XrdsMaterial> {
        self.get_material_by_id(handle.id())
    }

    pub fn get_material_by_id(&self, id: &AssetId) -> Option<XrdsMaterial> {
        let lock = self.resource_buffer.read().unwrap();
        lock.materials.get(id).map(|h| h.asset().clone())
    }

    pub fn get_material_handle(&self, id: &AssetId) -> Option<AssetHandle<XrdsMaterial>> {
        let lock = self.resource_buffer.read().unwrap();
        lock.materials.get(id).map(|h| h.as_weak_handle())
    }

    pub fn generate_id(&self) -> AssetId {
        let uuid = Uuid::new_v4();
        AssetId::Uuid(uuid)
    }
}
