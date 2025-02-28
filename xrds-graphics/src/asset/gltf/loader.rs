use std::{
    borrow::Cow,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use glam::{Vec3, Vec4};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use wgpu::SamplerDescriptor;

use crate::{
    asset::types::{AssetHandle, AssetId},
    pbr::{self, AlphaMode},
    AssetServer, BufferAssetInfo, MaterialAssetInfo, TextureAssetInfo, XrdsBuffer, XrdsBufferType,
    XrdsIndexBuffer, XrdsMaterial, XrdsMesh, XrdsPrimitive, XrdsScene, XrdsTexture,
    XrdsVertexBuffer,
};

use super::Gltf;

pub struct GltfLoader {
    asset_server: Arc<AssetServer>,
    asset_path: PathBuf,
}

struct GltfLoadContext<'a> {
    gltf_name: Cow<'a, str>,
    raw_buffers: Vec<Vec<u8>>,
    textures: Vec<AssetHandle<XrdsTexture>>,
}

impl GltfLoader {
    pub fn new(asset_server: Arc<AssetServer>, asset_path: &Path) -> Self {
        Self {
            asset_server,
            asset_path: asset_path.to_path_buf(),
        }
    }

    pub async fn load_from_file(&self, path: &Path) -> anyhow::Result<Gltf> {
        let gltf_file_name = path
            .file_name()
            .ok_or(anyhow::Error::msg("Invalid file name"))?
            .to_string_lossy();
        let buf = Self::read_file(path).await?;
        self.load(&buf, &gltf_file_name).await
    }

    pub async fn load(&self, data: &[u8], name: &str) -> anyhow::Result<Gltf> {
        let gltf = gltf::Gltf::from_slice(data)?;

        let gltf_key = self.asset_path.join(name).to_string_lossy().to_string();
        log::debug!("Load gltf {}", &gltf_key);

        // Phase1. load gltf files into gltf structure
        let instant = Instant::now();
        let buffers: Vec<_> = gltf.buffers().collect();
        let raw_buffers: Vec<_> = buffers
            .par_iter()
            .map(|buffer| Self::load_buffer(&gltf, buffer, &self.asset_path))
            .filter_map(|res| res.ok())
            .collect();
        log::debug!(
            "Load {} raw buffers in {} secs",
            raw_buffers.len(),
            instant.elapsed().as_secs_f32()
        );
        let instant = Instant::now();
        let images: Vec<_> = gltf.images().collect();
        let textures: Vec<_> = images
            .par_iter()
            .map(|image| {
                Self::load_image(
                    image,
                    &raw_buffers,
                    &self.asset_path,
                    &gltf_key,
                    self.asset_server.clone(),
                )
            })
            .filter_map(|res| res.ok())
            .collect();
        log::debug!(
            "Load {} xrds textures in {} secs",
            textures.len(),
            instant.elapsed().as_secs_f32()
        );
        let context = GltfLoadContext {
            gltf_name: Cow::Borrowed(&gltf_key),
            raw_buffers,
            textures,
        };
        let instant = Instant::now();
        let mut scenes = Vec::new();
        for scene in gltf.scenes() {
            scenes.push(self.load_scene(&scene, &context)?);
        }
        log::debug!(
            "Load {} scenes in {} secs",
            scenes.len(),
            instant.elapsed().as_secs_f32()
        );

        Ok(Gltf::default())
    }

    fn load_scene(
        &self,
        scene: &gltf::Scene,
        context: &GltfLoadContext,
    ) -> anyhow::Result<XrdsScene> {
        let instant = Instant::now();
        let scene_name = Self::name_from_scene(scene, &context.gltf_name);
        let mut nodes = Vec::new();
        for node in scene.nodes() {
            nodes.push(self.load_node(&node, &scene_name, context)?);
        }
        log::debug!(
            "Load {} nodes in {} secs",
            nodes.len(),
            instant.elapsed().as_secs_f32()
        );

        Ok(XrdsScene {})
    }

    fn load_node(
        &self,
        node: &gltf::Node,
        scene_name: &str,
        context: &GltfLoadContext,
    ) -> anyhow::Result<()> {
        if let Some(mesh) = node.mesh() {
            let instant = Instant::now();
            self.load_mesh(&mesh, context)?;
            log::debug!("Load mesh in {} secs", instant.elapsed().as_secs_f32());
        }
        Ok(())
    }

    fn load_mesh(&self, mesh: &gltf::Mesh, context: &GltfLoadContext) -> anyhow::Result<XrdsMesh> {
        let instant = Instant::now();
        let mut primitives = Vec::new();
        for primitive in mesh.primitives() {
            let (vertex_buffers, options) =
                self.load_vertex_buffers_from_primitive(&primitive, context)?;
            let index_buffer = if let Some(indices) = primitive.indices() {
                if let Some(view) = indices.view() {
                    let handle =
                        self.load_buffer_from_view(&view, XrdsBufferType::Index, context)?;
                    let buffer = self.asset_server.get_buffer(&handle).unwrap();

                    Some(XrdsIndexBuffer {
                        buffer,
                        index_format: data_type_to_vertex_format(indices.data_type()),
                    })
                } else {
                    todo!("Make empty index?")
                }
            } else {
                None
            };
            let material = self.load_material(
                &vertex_buffers,
                &options,
                index_buffer.as_ref(),
                &primitive.material(),
                primitive.mode(),
                context,
            )?;

            let xrds_primitive = XrdsPrimitive {
                vertices: vertex_buffers,
                indices: index_buffer,
                material: material,
            };

            primitives.push(xrds_primitive);
        }
        log::debug!(
            "Load {} primitives in {} secs",
            primitives.len(),
            instant.elapsed().as_secs_f32()
        );

        Ok(XrdsMesh {
            name: "".to_owned(),
            primitives,
        })
    }

    fn load_material(
        &self,
        vertex_buffers: &[XrdsVertexBuffer],
        vertex_input_options: &pbr::PbrVertexInputOption,
        index_buffer: Option<&XrdsIndexBuffer>,
        material: &gltf::Material,
        mode: gltf::mesh::Mode,
        context: &GltfLoadContext,
    ) -> anyhow::Result<AssetHandle<XrdsMaterial>> {
        let name = Self::name_from_material(material, &context.gltf_name);
        let handle = if let Some(handle) = self
            .asset_server
            .get_material_handle(&AssetId::Key(name.clone()))
        {
            handle
        } else {
            let mut material_input_option = pbr::PbrMaterialInputOption::default();
            Self::update_material_input_options(&mut material_input_option, material);

            let options = pbr::Options {
                vertex_input: *vertex_input_options,
                material_input: material_input_option,
                view_count: 2,
            };

            // load new material
            let handle = self.asset_server.register_material(&MaterialAssetInfo {
                id: &AssetId::Key(name.clone()),
                options: &options,
                vertex_buffers,
            })?;
            handle
        };

        Ok(handle)
    }

    fn load_vertex_buffers_from_primitive(
        &self,
        primitive: &gltf::Primitive,
        context: &GltfLoadContext,
    ) -> anyhow::Result<(Vec<XrdsVertexBuffer>, pbr::PbrVertexInputOption)> {
        let mut vertex_input_option = pbr::PbrVertexInputOption::default();

        let mut res = Vec::new();
        for (semantic, accessor) in primitive.attributes() {
            Self::update_vertex_input_options(&mut vertex_input_option, &semantic, &accessor);
            let vertex_attribute = wgpu::VertexAttribute {
                offset: 0, // discreted vertex must be started at offset 0
                format: vertex_format_from_data_type(accessor.data_type(), accessor.dimensions())?,
                shader_location: pbr::PbrVertexSemantic::from(semantic).location(),
            };
            if let Some(view) = accessor.view() {
                let handle = self.load_buffer_from_view(&view, XrdsBufferType::Vertex, context)?;
                let buffer = XrdsVertexBuffer {
                    buffer: self.asset_server.get_buffer(&handle).unwrap(),
                    vertex_attributes: [vertex_attribute],
                };
                res.push(buffer);
            } else {
                todo!("Generate empty buffer")
            }
        }

        Ok((res, vertex_input_option))
    }

    fn load_buffer_from_view(
        &self,
        view: &gltf::buffer::View,
        buffer_type: XrdsBufferType,
        context: &GltfLoadContext,
    ) -> anyhow::Result<AssetHandle<XrdsBuffer>> {
        let name = Self::name_from_buffer_view(view, &context.gltf_name);

        let handle = if let Some(buffer) = self
            .asset_server
            .get_buffer_handle(&AssetId::Key(name.clone()))
        {
            buffer
        } else {
            log::debug!("'{}' not exists in asset server. Try loading", name);
            let raw_buffer = &context.raw_buffers[view.buffer().index()];
            let offset = view.offset();
            let length = view.length();
            let stride = view.stride().unwrap_or(1) as u64;

            log::debug!(
                "  Load buffer '{}': length={}, stride={}",
                name,
                length,
                stride
            );

            self.asset_server.register_buffer(&BufferAssetInfo {
                id: &AssetId::Key(name.clone()),
                data: &raw_buffer[offset..(offset + length)],
                ty: buffer_type,
                stride,
            })?
        };

        Ok(handle)
    }

    fn load_sampler_descriptor<'a>(sampler: &'a gltf::texture::Sampler) -> SamplerDescriptor<'a> {
        let to_address_mode = |wrapping_mode: gltf::texture::WrappingMode| match wrapping_mode {
            gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
            gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
        };
        let address_mode_u = to_address_mode(sampler.wrap_s());
        let address_mode_v = to_address_mode(sampler.wrap_t());
        let mag_filter = if let Some(sampler_mag) = sampler.mag_filter() {
            match sampler_mag {
                gltf::texture::MagFilter::Linear => wgpu::FilterMode::Linear,
                gltf::texture::MagFilter::Nearest => wgpu::FilterMode::Nearest,
            }
        } else {
            wgpu::FilterMode::Nearest
        };
        let (min_filter, mipmap_filter) = if let Some(sampler_min) = sampler.min_filter() {
            match sampler_min {
                gltf::texture::MinFilter::Linear => {
                    (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::LinearMipmapLinear => {
                    (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::LinearMipmapNearest => {
                    (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
                }
                gltf::texture::MinFilter::Nearest => {
                    (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::NearestMipmapLinear => {
                    (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::NearestMipmapNearest => {
                    (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest)
                }
            }
        } else {
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
        };
        wgpu::SamplerDescriptor {
            label: sampler.name(),
            address_mode_u,
            address_mode_v,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        }
    }

    fn load_buffer(
        gltf: &gltf::Gltf,
        buffer: &gltf::Buffer,
        asset_path: &Path,
    ) -> anyhow::Result<Vec<u8>> {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => {
                if let Some(blob) = gltf.blob.as_deref() {
                    blob.to_vec()
                } else {
                    anyhow::bail!("Buffer reference blob but gltf file not has blob data")
                }
            }
            gltf::buffer::Source::Uri(uri) => {
                let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
                if let Ok(uri) = UriData::parse(&decoded_uri) {
                    uri.data
                } else {
                    let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
                    let path = asset_path.to_path_buf().join(decoded_uri.to_string());
                    let mut file = File::open(path)?;
                    let mut res = Vec::new();
                    file.read_to_end(&mut res)?;

                    res
                }
            }
        };

        Ok(data)
    }

    fn load_image(
        image: &gltf::Image<'_>,
        raw_buffers: &[Vec<u8>],
        asset_path: &Path,
        super_key: &str,
        asset_server: Arc<AssetServer>,
    ) -> anyhow::Result<AssetHandle<XrdsTexture>> {
        let name = Self::name_from_image(image, super_key)?;

        let handle = match asset_server.get_texture_handle(&AssetId::Key(name.clone())) {
            Some(h) => h,
            None => {
                log::debug!("'{}' not exists in asset server. Try loading", name);
                let loaded_image = match image.source() {
                    gltf::image::Source::View { view, mime_type: _ } => {
                        let buffer_index = view.buffer().index();
                        let buffer = &raw_buffers[buffer_index];
                        let offset = view.offset();
                        let length = view.length();
                        let slice = &buffer[offset..offset + length];

                        image::load_from_memory(slice)?
                    }
                    gltf::image::Source::Uri { uri, mime_type: _ } => {
                        let decoded_uri =
                            percent_encoding::percent_decode_str(uri).decode_utf8()?;
                        let blob = if let Ok(uri) = UriData::parse(&decoded_uri) {
                            // data in uri
                            uri.data
                        } else {
                            // path
                            let path = asset_path.to_path_buf().join(decoded_uri.to_string());
                            let mut file = File::open(path)?;
                            let mut res = Vec::new();
                            file.read_to_end(&mut res)?;

                            res
                        };
                        image::load_from_memory(&blob)?
                    }
                };

                let data = loaded_image.to_rgba8().into_vec();
                let width = loaded_image.width();
                let height = loaded_image.height();
                let depth_or_array = 1;
                let handle = asset_server.register_texture(&TextureAssetInfo {
                    id: &AssetId::Key(name),
                    data: &data,
                    width,
                    height,
                    depth_or_array,
                })?;
                handle
            }
        };

        Ok(handle)
    }

    async fn read_file(path: &Path) -> anyhow::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut f = File::open(path)?;
        f.read_to_end(&mut buf)?;

        Ok(buf)
    }
}

/// Utilities
impl GltfLoader {
    fn name_from_image(image: &gltf::Image, gltf_name: &str) -> anyhow::Result<String> {
        let name = match image.source() {
            gltf::image::Source::View { view, mime_type: _ } => {
                if let Some(name) = view.name() {
                    format!("{}.{}", gltf_name, name)
                } else {
                    format!("{}.image.{}", gltf_name, image.index())
                }
            }
            gltf::image::Source::Uri { uri, mime_type: _ } => {
                let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
                format!("{}.{}", gltf_name, decoded_uri)
            }
        };
        Ok(name)
    }

    fn name_from_scene(scene: &gltf::Scene, gltf_name: &str) -> String {
        if let Some(name) = scene.name() {
            format!("{}.{}", gltf_name, name)
        } else {
            format!("{}.scene.{}", gltf_name, scene.index())
        }
    }

    fn name_from_node(node: &gltf::Node, scene_name: &str) -> String {
        if let Some(name) = node.name() {
            format!("{}.{}", scene_name, name)
        } else {
            format!("{}.node.{}", scene_name, node.index())
        }
    }

    fn name_from_buffer_view(view: &gltf::buffer::View, gltf_name: &str) -> String {
        if let Some(name) = view.name() {
            format!("{}.{}", gltf_name, name)
        } else {
            format!("{}.buffer_view.{}", gltf_name, view.index())
        }
    }

    fn name_from_material(material: &gltf::Material, gltf_name: &str) -> String {
        if let Some(name) = material.name() {
            format!("{}.{}", gltf_name, name)
        } else {
            if let Some(index) = material.index() {
                format!("{}.material.{}", gltf_name, index)
            } else {
                format!("{}.material.default", gltf_name)
            }
        }
    }

    fn update_vertex_input_options(
        vertex_option: &mut pbr::PbrVertexInputOption,
        semantic: &gltf::Semantic,
        accessor: &gltf::Accessor,
    ) {
        match *semantic {
            gltf::Semantic::Positions => vertex_option.position = true,
            gltf::Semantic::Normals => vertex_option.normal = true,
            gltf::Semantic::Tangents => vertex_option.tangent = true,
            gltf::Semantic::TexCoords(n) => {
                if n == 0 {
                    vertex_option.texcoord_0 = true;
                } else if n == 1 {
                    vertex_option.texcoord_1 = true;
                }
            }
            gltf::Semantic::Colors(_) => {
                if accessor.dimensions().multiplicity() == 3 {
                    vertex_option.color = Some(pbr::ColorChannel::Ch3)
                } else {
                    vertex_option.color = Some(pbr::ColorChannel::Ch4)
                }
            }
            gltf::Semantic::Weights(n) => {
                if n == 0 {
                    vertex_option.weights_joints_0 = true;
                } else if n == 1 {
                    vertex_option.weights_joints_1 = true;
                }
            }
            gltf::Semantic::Joints(n) => {
                if n == 0 {
                    vertex_option.weights_joints_0 = true;
                } else if n == 1 {
                    vertex_option.weights_joints_1 = true;
                }
            }
        }
    }

    fn update_material_input_options(
        option: &mut pbr::PbrMaterialInputOption,
        material: &gltf::Material,
    ) {
        let pbr_metallic_roughness = material.pbr_metallic_roughness();
        if let Some(base_color) = pbr_metallic_roughness.base_color_texture() {
            option.base_color = true;
            option.base_color_texcoord = base_color.tex_coord();
        }
        option.base_color_factor = Vec4::from_array(pbr_metallic_roughness.base_color_factor());
        if let Some(metallic_roughness) = pbr_metallic_roughness.metallic_roughness_texture() {
            option.metallic_roughness = true;
            option.metallic_roughness_texcoord = metallic_roughness.tex_coord();
        }
        option.metallic_factor = pbr_metallic_roughness.metallic_factor();
        option.roughness_factor = pbr_metallic_roughness.roughness_factor();
        if let Some(normal_texture) = material.normal_texture() {
            option.normal = true;
            option.normal_scale = normal_texture.scale();
        }
        if let Some(emissive) = material.emissive_texture() {
            option.emissive = true;
            option.emissive_texcoord = emissive.tex_coord();
        }
        option.emissive_factor = Vec3::from_array(material.emissive_factor());
        if let Some(occlusion) = material.occlusion_texture() {
            option.occlusion = true;
            option.occlusion_strength = occlusion.strength();
            option.occlusion_texcoord = occlusion.tex_coord();
        }
        option.double_sided = material.double_sided();
        option.alpha_mode = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask {
                alpha_cutoff: material
                    .alpha_cutoff()
                    .unwrap_or(0.5 /* default value from gltf spec */),
            },
        };
    }
}

#[derive(Debug)]
struct UriData<'a> {
    #[allow(dead_code)]
    mime_type: &'a str,
    data: Vec<u8>,
}

impl<'a> UriData<'a> {
    fn parse(decoded_uri: &'a str) -> anyhow::Result<UriData<'a>> {
        if let Some(striped_uri) = decoded_uri.strip_prefix("data:") {
            if let Some((mime_type, data)) = striped_uri.split_once(',') {
                let (mime_type, is_base64) = match mime_type.strip_suffix(";base64") {
                    Some(m) => (m, true),
                    None => (mime_type, false),
                };
                let data = if is_base64 {
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)?
                } else {
                    data.as_bytes().to_owned()
                };
                Ok(UriData { mime_type, data })
            } else {
                anyhow::bail!("Invalid data uri format")
            }
        } else {
            // TODO: Define error
            anyhow::bail!("Not a data uri")
        }
    }
}

impl From<gltf::Semantic> for pbr::PbrVertexSemantic {
    fn from(value: gltf::Semantic) -> Self {
        match value {
            gltf::Semantic::Positions => pbr::PbrVertexSemantic::Position,
            gltf::Semantic::Normals => pbr::PbrVertexSemantic::Normal,
            gltf::Semantic::Tangents => pbr::PbrVertexSemantic::Tangent,
            gltf::Semantic::TexCoords(n) => pbr::PbrVertexSemantic::Texcoord(n),
            gltf::Semantic::Colors(n) => pbr::PbrVertexSemantic::Color(n),
            gltf::Semantic::Weights(n) => pbr::PbrVertexSemantic::Weights(n),
            gltf::Semantic::Joints(n) => pbr::PbrVertexSemantic::Joints(n),
        }
    }
}

#[inline]
fn vertex_format_from_data_type(
    data_type: gltf::accessor::DataType,
    dimensions: gltf::accessor::Dimensions,
) -> anyhow::Result<wgpu::VertexFormat> {
    let ty = match dimensions {
        gltf::accessor::Dimensions::Vec2 => match data_type {
            gltf::accessor::DataType::U8 => Some(wgpu::VertexFormat::Uint8x2),
            gltf::accessor::DataType::U16 => Some(wgpu::VertexFormat::Uint16x2),
            gltf::accessor::DataType::U32 => Some(wgpu::VertexFormat::Uint32x2),
            gltf::accessor::DataType::I8 => Some(wgpu::VertexFormat::Sint8x2),
            gltf::accessor::DataType::I16 => Some(wgpu::VertexFormat::Sint16x2),
            gltf::accessor::DataType::F32 => Some(wgpu::VertexFormat::Float32x2),
        },
        gltf::accessor::Dimensions::Vec3 => match data_type {
            gltf::accessor::DataType::U32 => Some(wgpu::VertexFormat::Uint32x3),
            gltf::accessor::DataType::F32 => Some(wgpu::VertexFormat::Float32x3),
            _ => None,
        },
        gltf::accessor::Dimensions::Vec4 => match data_type {
            gltf::accessor::DataType::U8 => Some(wgpu::VertexFormat::Uint8x4),
            gltf::accessor::DataType::U16 => Some(wgpu::VertexFormat::Uint16x4),
            gltf::accessor::DataType::U32 => Some(wgpu::VertexFormat::Uint32x4),
            gltf::accessor::DataType::I8 => Some(wgpu::VertexFormat::Sint8x4),
            gltf::accessor::DataType::I16 => Some(wgpu::VertexFormat::Sint16x4),
            gltf::accessor::DataType::F32 => Some(wgpu::VertexFormat::Float32x4),
        },
        gltf::accessor::Dimensions::Scalar => match data_type {
            gltf::accessor::DataType::U32 => Some(wgpu::VertexFormat::Uint32),
            gltf::accessor::DataType::F32 => Some(wgpu::VertexFormat::Float32),
            _ => None,
        },
        _ => None,
    };

    if let Some(format) = ty {
        Ok(format)
    } else {
        anyhow::bail!("Unsupported format")
    }
}

#[inline]
fn data_type_to_vertex_format(data_type: gltf::accessor::DataType) -> wgpu::VertexFormat {
    match data_type {
        gltf::accessor::DataType::I8 => wgpu::VertexFormat::Sint8,
        gltf::accessor::DataType::U8 => wgpu::VertexFormat::Uint8,
        gltf::accessor::DataType::I16 => wgpu::VertexFormat::Sint16,
        gltf::accessor::DataType::U16 => wgpu::VertexFormat::Uint16,
        gltf::accessor::DataType::U32 => wgpu::VertexFormat::Uint32,
        gltf::accessor::DataType::F32 => wgpu::VertexFormat::Float32,
    }
}
