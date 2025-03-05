use std::{
    borrow::Cow,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use glam::{Mat4, Vec4};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use wgpu::SamplerDescriptor;

use crate::{
    asset::types::{AssetHandle, AssetId},
    pbr::{self, AlphaMode},
    AssetServer, BufferAssetInfo, MaterialAssetInfo, MaterialTextureInfo, PbrMaterialInfo,
    TextureAssetInfo, Transform, XrdsBuffer, XrdsBufferType, XrdsIndexBuffer, XrdsMaterialInstance,
    XrdsMesh, XrdsObject, XrdsPrimitive, XrdsTexture, XrdsVertexBuffer,
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
    samplers: Vec<wgpu::Sampler>,
    default_sampler: wgpu::Sampler,
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
        let samplers: Vec<_> = gltf
            .samplers()
            .map(|sampler| self.load_sampler(&sampler))
            .collect();
        let default_sampler = self
            .asset_server
            .device()
            .create_sampler(&SamplerDescriptor {
                label: None,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
        log::debug!(
            "Load {} xrds textures in {} secs",
            textures.len(),
            instant.elapsed().as_secs_f32()
        );
        let context = GltfLoadContext {
            gltf_name: Cow::Borrowed(&gltf_key),
            raw_buffers,
            textures,
            samplers,
            default_sampler,
        };
        let instant = Instant::now();
        let mut scenes = Vec::new();
        for scene in gltf.scenes() {
            scenes.push(Arc::new(self.load_scene_as_object(&scene, &context)?));
        }
        log::debug!(
            "Load {} scenes in {} secs",
            scenes.len(),
            instant.elapsed().as_secs_f32()
        );
        let mut res = Gltf::default().with_scenes(&scenes);
        if let Some(default_scene) = gltf.default_scene() {
            res = res.with_default_scene(default_scene.index());
        }

        Ok(res)
    }

    fn load_scene_as_object(
        &self,
        scene: &gltf::Scene,
        context: &GltfLoadContext,
    ) -> anyhow::Result<XrdsObject> {
        let instant = Instant::now();
        let scene_name = Self::name_from_scene(scene, &context.gltf_name);
        let mut nodes = Vec::new();
        let scene_transform = Mat4::default();
        for node in scene.nodes() {
            nodes.push(self.load_node_as_object(&node, &scene_name, &scene_transform, context)?);
        }
        log::debug!(
            "Load {} nodes in {} secs",
            nodes.len(),
            instant.elapsed().as_secs_f32()
        );
        let scene_object = XrdsObject::default()
            .with_name(&scene_name)
            .with_childs(&nodes);

        Ok(scene_object)
    }

    fn load_node_as_object(
        &self,
        node: &gltf::Node,
        parent_name: &str,
        parent_transform: &Mat4,
        context: &GltfLoadContext,
    ) -> anyhow::Result<XrdsObject> {
        let name = Self::name_from_node(node, parent_name);
        let transform =
            parent_transform.mul_mat4(&glam::Mat4::from_cols_array_2d(&node.transform().matrix()));
        let mut object = XrdsObject::default()
            .with_name(&name)
            .with_transform(Transform::from_matrix(&transform));
        if let Some(mesh) = node.mesh() {
            let instant = Instant::now();
            let mesh = self.load_mesh(&mesh, context)?;
            log::debug!("Load mesh in {} secs", instant.elapsed().as_secs_f32());
            object.set_mesh(mesh);
        }
        for child in node.children() {
            let child_object = self.load_node_as_object(&child, &name, &transform, context)?;
            object.add_child(child_object);
        }
        Ok(object)
    }

    fn load_mesh(&self, mesh: &gltf::Mesh, context: &GltfLoadContext) -> anyhow::Result<XrdsMesh> {
        let name = Self::name_from_mesh(mesh, &context.gltf_name);

        let instant = Instant::now();
        let mut primitives = Vec::new();
        for primitive in mesh.primitives() {
            primitives.push(self.load_primitive(&primitive, context)?);
        }
        log::debug!(
            "Load {} primitives in {} secs",
            primitives.len(),
            instant.elapsed().as_secs_f32()
        );

        let mesh = XrdsMesh::default()
            .with_name(&name)
            .with_primitives(primitives);

        Ok(mesh)
    }

    fn load_primitive(
        &self,
        primitive: &gltf::Primitive,
        context: &GltfLoadContext,
    ) -> anyhow::Result<XrdsPrimitive> {
        let (vertex_buffers, options) =
            self.load_vertex_buffers_from_primitive(primitive, context)?;
        let index_buffer = if let Some(indices) = primitive.indices() {
            if let Some(view) = indices.view() {
                let handle = self.load_buffer_from_view(&view, XrdsBufferType::Index, context)?;
                let buffer = self.asset_server.get_buffer(&handle).unwrap();

                Some(XrdsIndexBuffer {
                    buffer,
                    index_format: data_type_to_index_format(indices.data_type()),
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
            &primitive.material(),
            primitive.mode(),
            context,
        )?;

        let xrds_primitive = XrdsPrimitive {
            vertices: vertex_buffers,
            indices: index_buffer,
            material,
        };

        Ok(xrds_primitive)
    }

    fn load_material(
        &self,
        vertex_buffers: &[XrdsVertexBuffer],
        vertex_input_options: &pbr::PbrVertexInputOption,
        material: &gltf::Material,
        mode: gltf::mesh::Mode,
        context: &GltfLoadContext,
    ) -> anyhow::Result<XrdsMaterialInstance> {
        // Material instance name
        let name = Self::name_from_material(material, &context.gltf_name);
        let handle = if let Some(handle) = self
            .asset_server
            .get_material_instance_handle(&AssetId::Key(name.clone()))
        {
            handle
        } else {
            let mut material_input_option = pbr::PbrMaterialInputOption::default();
            Self::update_material_input_options(&mut material_input_option, material);
            material_input_option.primitive_mode = match mode {
                gltf::mesh::Mode::Points => pbr::PrimitiveMode::PointList,
                gltf::mesh::Mode::Lines => pbr::PrimitiveMode::LineList,
                gltf::mesh::Mode::LineLoop => {
                    log::warn!("Unsupported primitive mode {:?}", mode);
                    pbr::PrimitiveMode::LineList
                }
                gltf::mesh::Mode::LineStrip => pbr::PrimitiveMode::LineStrip,
                gltf::mesh::Mode::Triangles => pbr::PrimitiveMode::TriangleList,
                gltf::mesh::Mode::TriangleStrip => pbr::PrimitiveMode::TriangleStrip,
                gltf::mesh::Mode::TriangleFan => {
                    log::warn!("Unsupported primitive mode {:?}", mode);
                    pbr::PrimitiveMode::TriangleList
                }
            };

            let options = pbr::Options {
                vertex_input: *vertex_input_options,
                material_input: material_input_option,
                view_count: 2,
            };
            // Get material unique id from its options.
            let material_id = AssetId::Key(options.as_hash());
            let xrds_material =
                if let Some(material) = self.asset_server.get_material_by_id(&material_id) {
                    material
                } else {
                    // load new material
                    let handle = self.asset_server.register_material(&MaterialAssetInfo {
                        id: &material_id,
                        options: &options,
                        vertex_buffers,
                    })?;
                    self.asset_server.get_material(&handle).unwrap().clone()
                };

            let get_texture = |index: usize| {
                context
                    .textures
                    .get(index)
                    .map(|handle| self.asset_server.get_texture(handle).unwrap().clone())
                    .unwrap()
            };
            let get_sampler = |index: Option<usize>| {
                index
                    .map(|i| {
                        context
                            .samplers
                            .get(i)
                            .cloned()
                            .unwrap_or(context.default_sampler.clone())
                    })
                    .unwrap_or(context.default_sampler.clone())
            };

            // Create material instance
            let id = AssetId::Key(name);
            let mut material_input = PbrMaterialInfo::new(&id);
            material_input.id = &id;
            material_input.params.base_color_factor =
                material.pbr_metallic_roughness().base_color_factor().into();
            if let Some(base_color_texture) = material.pbr_metallic_roughness().base_color_texture()
            {
                material_input.params.texcoord_base_color = base_color_texture.tex_coord();
                material_input.base_color_texture = Some(MaterialTextureInfo {
                    texture: get_texture(base_color_texture.texture().index()),
                    sampler: get_sampler(base_color_texture.texture().sampler().index()),
                });
            }
            material_input.params.metallic_factor =
                material.pbr_metallic_roughness().metallic_factor();
            material_input.params.roughness_factor =
                material.pbr_metallic_roughness().roughness_factor();
            if let Some(metallic_roughness_texture) = material
                .pbr_metallic_roughness()
                .metallic_roughness_texture()
            {
                material_input.params.texcoord_metallic_roughness =
                    metallic_roughness_texture.tex_coord();
                material_input.metallic_roughness_texture = Some(MaterialTextureInfo {
                    texture: get_texture(metallic_roughness_texture.texture().index()),
                    sampler: get_sampler(metallic_roughness_texture.texture().sampler().index()),
                });
            }
            let emissive_factor = material.emissive_factor();

            material_input.params.emissive_factor = Vec4::new(
                emissive_factor[0],
                emissive_factor[1],
                emissive_factor[2],
                1.0,
            );
            if let Some(normal_texture) = material.normal_texture() {
                material_input.params.normal_scale = normal_texture.scale();
                material_input.params.texcoord_normal = normal_texture.tex_coord();
                material_input.normal_texture = Some(MaterialTextureInfo {
                    texture: get_texture(normal_texture.texture().index()),
                    sampler: get_sampler(normal_texture.texture().sampler().index()),
                });
            }
            if let Some(occlusion_texture) = material.occlusion_texture() {
                material_input.params.texcoord_occlusion = occlusion_texture.tex_coord();
                material_input.params.occlusion_strength = occlusion_texture.strength();
                material_input.occlusion_texture = Some(MaterialTextureInfo {
                    texture: get_texture(occlusion_texture.texture().index()),
                    sampler: get_sampler(occlusion_texture.texture().sampler().index()),
                });
            }
            if let Some(emissive_texture) = material.emissive_texture() {
                material_input.params.texcoord_emissive = emissive_texture.tex_coord();
                material_input.emissive_texture = Some(MaterialTextureInfo {
                    texture: get_texture(emissive_texture.texture().index()),
                    sampler: get_sampler(emissive_texture.texture().sampler().index()),
                });
            }
            material_input.params.alpha_cutoff = material.alpha_cutoff().unwrap_or(0.5);

            self.asset_server
                .register_material_instance(&xrds_material, &material_input)?
        };

        let material_instance = self
            .asset_server
            .get_material_instance(&handle)
            .unwrap()
            .clone(); // must be exists

        Ok(material_instance)
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
                asset_server.register_texture(&TextureAssetInfo {
                    id: &AssetId::Key(name),
                    data: &data,
                    width,
                    height,
                    depth_or_array,
                })?
            }
        };

        Ok(handle)
    }

    fn load_sampler(&self, sampler: &gltf::texture::Sampler) -> wgpu::Sampler {
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
        let wgpu_sampler = self
            .asset_server
            .device()
            .create_sampler(&wgpu::SamplerDescriptor {
                label: None,
                address_mode_u,
                address_mode_v,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter,
                min_filter,
                mipmap_filter,
                ..Default::default()
            });

        wgpu_sampler
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

    fn name_from_mesh(mesh: &gltf::Mesh, gltf_name: &str) -> String {
        if let Some(name) = mesh.name() {
            format!("{}.{}", gltf_name, name)
        } else {
            format!("{}.node.{}", gltf_name, mesh.index())
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
        } else if let Some(index) = material.index() {
            format!("{}.material.{}", gltf_name, index)
        } else {
            format!("{}.material.default", gltf_name)
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
        option.base_color = pbr_metallic_roughness.base_color_texture().is_some();
        option.metallic_roughness = pbr_metallic_roughness
            .metallic_roughness_texture()
            .is_some();
        option.normal = material.normal_texture().is_some();
        option.emissive = material.emissive_texture().is_some();
        option.occlusion = material.occlusion_texture().is_some();
        option.double_sided = material.double_sided();
        option.alpha_mode = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask,
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
fn data_type_to_index_format(data_type: gltf::accessor::DataType) -> wgpu::IndexFormat {
    match data_type {
        gltf::accessor::DataType::U16 => wgpu::IndexFormat::Uint16,
        gltf::accessor::DataType::U32 => wgpu::IndexFormat::Uint32,
        _ => {
            log::warn!(
                "Unsupported index format type {:?}. Assume uint32",
                data_type
            );
            wgpu::IndexFormat::Uint32
        }
    }
}
