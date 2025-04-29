use core::f32;
use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

use glam::{Vec3, Vec4};
use gltf::khr_lights_punctual::Kind;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use uuid::Uuid;
use wgpu::SamplerDescriptor;
use xrds_core::Transform;

use crate::{
    asset::types::{AssetHandle, AssetId},
    pbr::{self, AlphaMode},
    AssetServer, BufferAssetInfo, CameraComponent, CameraInfo, Component, Entity, Fov, GltfScene,
    GraphicsInstance, IndexFormat, LightComponent, LightDescription, LightType, MaterialAssetInfo,
    MaterialTextureInfo, MeshComponent, PbrMaterialInfo, PointLightDescription, PostRenderAction,
    ProjectionType, RenderTargetType, SpotLightDescription, TextureAssetInfo, TransformComponent,
    XrdsBuffer, XrdsBufferType, XrdsIndexBuffer, XrdsMaterialInstance, XrdsMesh, XrdsPrimitive,
    XrdsTexture, XrdsVertexBuffer,
};

use super::Gltf;

pub struct GltfLoader {
    graphics_instance: GraphicsInstance,
    asset_path: PathBuf,
}

struct GltfLoadContext<'a> {
    gltf_name: Cow<'a, str>,
    raw_buffers: Vec<Vec<u8>>,
    textures: Vec<AssetHandle<XrdsTexture>>,
    samplers: Vec<wgpu::Sampler>,
    default_sampler: wgpu::Sampler,
}

enum LoadImageResult {
    Cached(AssetHandle<XrdsTexture>),
    Loaded {
        id: AssetId,
        data: Vec<u8>,
        width: u32,
        height: u32,
        depth_or_array: u32,
    },
}

impl GltfLoader {
    pub fn new(graphics_instance: GraphicsInstance, asset_path: &Path) -> Self {
        Self {
            graphics_instance,
            asset_path: asset_path.to_path_buf(),
        }
    }

    pub async fn load_from_file<P>(
        &self,
        path: P,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<Gltf>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let gltf_file_name = path
            .file_name()
            .ok_or(anyhow::Error::msg("Invalid file name"))?
            .to_string_lossy();
        let buf = Self::read_file(path).await?;
        self.load(&buf, &gltf_file_name, asset_server).await
    }

    pub async fn load(
        &self,
        data: &[u8],
        name: &str,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<Gltf> {
        let instant = Instant::now();
        let gltf = gltf::Gltf::from_slice(data)?;
        let gltf_key = self.asset_path.join(name).to_string_lossy().to_string();
        log::trace!("+ Load gltf {}", &gltf_key);

        let raw_buffers = {
            let instant = Instant::now();
            log::trace!("  + Load raw buffers");
            let buffers: Vec<_> = gltf.buffers().collect();
            let raw_buffers: Vec<_> = buffers
                .par_iter()
                .map(|buffer| Self::load_buffer(&gltf, buffer, &self.asset_path))
                .filter_map(|res| res.ok())
                .collect();
            log::trace!(
                "  - Load raw buffers in {} secs",
                instant.elapsed().as_secs_f32()
            );
            raw_buffers
        };
        let textures = {
            let instant = Instant::now();
            log::trace!("  + Load images");
            let images: Vec<_> = gltf.images().collect();
            let load_image_results: Vec<_> = images
                .par_iter()
                .map(|image| {
                    Self::load_image(
                        image,
                        &raw_buffers,
                        &self.asset_path,
                        &gltf_key,
                        asset_server,
                    )
                })
                .filter_map(|res| res.ok())
                .collect();
            let mut textures = Vec::new();
            for result in load_image_results {
                let handle = match result {
                    LoadImageResult::Cached(handle) => handle,
                    LoadImageResult::Loaded {
                        id,
                        data,
                        width,
                        height,
                        depth_or_array,
                    } => asset_server.register_texture(&TextureAssetInfo {
                        id: &id,
                        data: &data,
                        width,
                        height,
                        depth_or_array,
                    })?,
                };
                textures.push(handle);
            }
            log::trace!(
                "  - Load images in {} secs",
                instant.elapsed().as_secs_f32()
            );
            textures
        };
        let samplers: Vec<_> = gltf
            .samplers()
            .map(|sampler| self.load_sampler(&sampler))
            .collect();
        let default_sampler = self
            .graphics_instance
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
        let context = GltfLoadContext {
            gltf_name: Cow::Borrowed(&gltf_key),
            raw_buffers,
            textures,
            samplers,
            default_sampler,
        };

        let gltf = {
            let instant = Instant::now();
            log::trace!("  + Load scenes");

            let mut scenes = Vec::new();
            for scene in gltf.scenes() {
                let scene_entities = self.load_scene(&scene, &context, asset_server)?;
                asset_server.register_entities(&scene_entities);
                scenes.push(GltfScene {
                    name: scene.name().map(|n| n.to_string()),
                    id: scene_entities[0].id().clone(),
                });
            }
            let mut res = Gltf::default().with_scenes(scenes);
            if let Some(default_scene) = gltf.default_scene() {
                res = res.with_default_scene(default_scene.index());
            }
            log::trace!(
                "  - Load scenes in {} secs",
                instant.elapsed().as_secs_f32()
            );

            res
        };

        log::debug!(
            "- Load gltf '{}' in {} secs",
            gltf_key,
            instant.elapsed().as_secs_f32()
        );

        Ok(gltf)
    }

    fn load_scene(
        &self,
        scene: &gltf::Scene,
        context: &GltfLoadContext,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<Vec<Entity>> {
        let instant = Instant::now();
        log::trace!("    + Load scene #{}", scene.index());
        let mut node_entity_map: HashMap<usize, Entity> = HashMap::new();

        let scene_name = Self::name_from_scene(scene, &context.gltf_name);
        let scene_id = Uuid::new_v4();
        let mut scene_entity = Entity::default().with_id(&scene_id).with_name(&scene_name);
        let mut scene_transform_component = TransformComponent::default();

        // Phase1. create entities
        // iterate all nodes in scene graph
        for node in scene.nodes() {
            let entity = self.load_node(
                &node,
                &scene_name,
                &glam::Mat4::IDENTITY,
                context,
                &mut node_entity_map,
                asset_server,
            )?;
            node_entity_map.insert(node.index(), entity);
        }

        // Phase2. build entitiy relationships
        for node in scene.nodes() {
            if let Some(entity) = node_entity_map.get(&node.index()) {
                scene_transform_component.add_child(entity.id());
            }
            Self::build_node_relationsip(&node, &scene_id, &mut node_entity_map)?;
        }
        scene_entity.add_component(Component::Transform(scene_transform_component));

        let mut entities: Vec<_> = node_entity_map
            .into_iter()
            .map(|(_, entity)| entity)
            .collect();
        entities.insert(0, scene_entity);

        log::trace!(
            "    - Load scene #{} in {} secs",
            scene.index(),
            instant.elapsed().as_secs_f32()
        );

        Ok(entities)
    }

    /// Load node as entity from gltf node
    /// * Requires parent transform matrix for pre-calculated local transform.
    fn load_node(
        &self,
        node: &gltf::Node,
        parent_name: &str,
        parent_transform: &glam::Mat4,
        context: &GltfLoadContext,
        node_entity_map: &mut HashMap<usize, Entity>,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<Entity> {
        let instant = Instant::now();

        let name: String = Self::name_from_node(node, parent_name);
        let transform =
            parent_transform.mul_mat4(&glam::Mat4::from_cols_array_2d(&node.transform().matrix()));
        let entity_id = Uuid::new_v4();
        let mut entity = Entity::default().with_id(&entity_id).with_name(&name);
        let mut transform_component =
            TransformComponent::default().with_local_transform(Transform::from_matrix(&transform));

        if let Some(mesh) = node.mesh() {
            let mesh = self.load_mesh(&mesh, &name, context, asset_server)?;
            entity.add_component(Component::Mesh(MeshComponent::new(mesh)));
        }
        if let Some(camera) = node.camera() {
            let camera_component = self.load_camera(&camera, &name)?;
            entity.add_component(Component::Camera(camera_component));
        }
        if let Some(light) = node.light() {
            let light_component = self.load_light(&light)?;
            entity.add_component(Component::Light(light_component));
        }
        if node.children().len() > 0 {
            let instant = Instant::now();
            log::trace!("    + Load children of node #{}", node.index());
            for child in node.children() {
                let child_entity = self.load_node(
                    &child,
                    &name,
                    &transform,
                    context,
                    node_entity_map,
                    asset_server,
                )?;
                transform_component.add_child(child_entity.id());
                node_entity_map.insert(child.index(), child_entity);
            }
            log::trace!(
                "    + Load children of node #{} in {} secs",
                node.index(),
                instant.elapsed().as_secs_f32()
            );
        }
        entity.add_component(Component::Transform(transform_component));
        log::trace!(
            "    - Load node #{} in {} secs",
            node.index(),
            instant.elapsed().as_secs_f32()
        );

        Ok(entity)
    }

    fn build_node_relationsip(
        node: &gltf::Node,
        parent_uuid: &Uuid,
        node_entity_map: &mut HashMap<usize, Entity>,
    ) -> anyhow::Result<()> {
        if let Some(entity) = node_entity_map.get_mut(&node.index()) {
            let uuid = *entity.id();
            let entity_transform_component = entity.get_transform_component_mut().unwrap();
            entity_transform_component.set_parent(parent_uuid);

            for child in node.children() {
                Self::build_node_relationsip(&child, &uuid, node_entity_map)?;
            }
        }

        Ok(())
    }

    // fn load_scene_as_object(
    //     &self,
    //     scene: &gltf::Scene,
    //     context: &GltfLoadContext,
    // ) -> anyhow::Result<Renderable> {
    //     let instant = Instant::now();
    //     log::trace!("    + Load scene #{}", scene.index());
    //     let scene_name = Self::name_from_scene(scene, &context.gltf_name);
    //     let mut nodes = Vec::new();
    //     let scene_transform = Mat4::default();
    //     for node in scene.nodes() {
    //         if let Some(child_node) =
    //             self.load_node_as_object(&node, &scene_name, &scene_transform, context)?
    //         {
    //             nodes.push(child_node);
    //         }
    //     }
    //     let scene_object = Renderable::default()
    //         .with_name(&scene_name)
    //         .with_childs(&nodes);

    //     log::log::trace!(
    //         "    - Load scene #{} in {} secs",
    //         scene.index(),
    //         instant.elapsed().as_secs_f32()
    //     );
    //     Ok(scene_object)
    // }

    // fn load_node_as_object(
    //     &self,
    //     node: &gltf::Node,
    //     parent_name: &str,
    //     parent_transform: &Mat4,
    //     context: &GltfLoadContext,
    // ) -> anyhow::Result<Option<Renderable>> {
    //     let instant = Instant::now();
    //     log::trace!("    + Load node #{}", node.index());

    //     if node.mesh().is_none() && node.children().len() == 0 {
    //         log::warn!(
    //             "    - Node #{} has no mesh and children. Maybe utility node. Skip loading",
    //             node.index()
    //         );
    //         Ok(None)
    //     } else {
    //         let name = Self::name_from_node(node, parent_name);
    //         let transform = parent_transform
    //             .mul_mat4(&glam::Mat4::from_cols_array_2d(&node.transform().matrix()));
    //         let mut object = Renderable::default()
    //             .with_name(&name)
    //             .with_transform(Transform::from_matrix(&transform));
    //         if let Some(mesh) = node.mesh() {
    //             let mesh = self.load_mesh(&mesh, context)?;
    //             object.set_mesh(mesh);
    //         }
    //         if node.children().len() > 0 {
    //             let instant = Instant::now();
    //             log::trace!("    + Load children of node #{}", node.index());
    //             for child in node.children() {
    //                 if let Some(child_object) =
    //                     self.load_node_as_object(&child, &name, &transform, context)?
    //                 {
    //                     object.add_child(child_object);
    //                 }
    //             }
    //             log::trace!(
    //                 "    + Load children of node #{} in {} secs",
    //                 node.index(),
    //                 instant.elapsed().as_secs_f32()
    //             );
    //         }
    //         log::log::trace!(
    //             "    - Load node #{} in {} secs",
    //             node.index(),
    //             instant.elapsed().as_secs_f32()
    //         );

    //         Ok(Some(object))
    //     }
    // }

    fn load_camera(
        &self,
        camera: &gltf::Camera,
        parent_name: &str,
    ) -> anyhow::Result<CameraComponent> {
        log::trace!("    + Load camera #{}", camera.index());

        let _name = Self::name_from_camera(camera, parent_name);

        let info = match camera.projection() {
            gltf::camera::Projection::Orthographic(o) => CameraInfo {
                projection_type: ProjectionType::Orthographic,
                fov: Fov {
                    up: (o.ymag() * 45.0f32).to_radians(),
                    down: (o.ymag() * 45.0f32).to_radians(),
                    left: (o.xmag() * 45.0f32).to_radians(),
                    right: (o.xmag() * 45.0f32).to_radians(),
                },
                far: o.zfar(),
                near: o.znear(),
            },
            gltf::camera::Projection::Perspective(p) => {
                p.aspect_ratio();
                p.zfar();
                p.znear();

                CameraInfo {
                    projection_type: ProjectionType::Perspective,
                    fov: Fov {
                        up: p.yfov() * 0.5,
                        down: p.yfov() * 0.5,
                        left: p.yfov() * p.aspect_ratio().unwrap_or(1.0f32) * 0.5,
                        right: p.yfov() * p.aspect_ratio().unwrap_or(1.0f32) * 0.5,
                    },
                    far: p.zfar().unwrap_or(f32::MAX),
                    near: p.znear(),
                }
            }
        };

        Ok(CameraComponent::default()
            .with_camera(&[info])
            .with_render_target_type(RenderTargetType::Texture2D)
            .with_post_render_action(PostRenderAction::None))
    }

    fn load_light(
        &self,
        light: &gltf::khr_lights_punctual::Light,
    ) -> anyhow::Result<LightComponent> {
        let light_type = match light.kind() {
            Kind::Directional => LightType::Directional,
            Kind::Point => LightType::Point(PointLightDescription {
                range: light.range().unwrap_or(f32::MAX),
            }),
            Kind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            } => LightType::Spot(SpotLightDescription {
                inner_cons_cos: inner_cone_angle.cos(),
                outer_cons_cos: outer_cone_angle.cos(),
                range: light.range().unwrap_or(f32::MAX),
            }),
        };
        let color = Vec3::from_array(light.color());
        let intensity = light.intensity();

        Ok(LightComponent::new(&LightDescription {
            ty: light_type,
            color,
            intensity,
            cast_shadow: true,
        }))
    }

    fn load_mesh(
        &self,
        mesh: &gltf::Mesh,
        parent_name: &str,
        context: &GltfLoadContext,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<XrdsMesh> {
        let instant = Instant::now();
        log::trace!("      + Load mesh #{}", mesh.index());

        let name = Self::name_from_mesh(mesh, parent_name);

        let mut primitives = Vec::new();
        for primitive in mesh.primitives() {
            primitives.push(self.load_primitive(&primitive, context, asset_server)?);
        }

        let xrds_mesh = XrdsMesh::default()
            .with_name(&name)
            .with_primitives(primitives);

        log::trace!(
            "      - Load mesh #{} in {} secs",
            mesh.index(),
            instant.elapsed().as_secs_f32()
        );
        Ok(xrds_mesh)
    }

    fn load_primitive(
        &self,
        primitive: &gltf::Primitive,
        context: &GltfLoadContext,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<XrdsPrimitive> {
        let instant = Instant::now();
        log::trace!("        + Load primitive #{}", primitive.index());
        let (vertex_buffers, options, position_index) =
            self.load_vertex_buffers_from_primitive(primitive, context, asset_server)?;
        let index_buffer = if let Some(indices) = primitive.indices() {
            let instant = Instant::now();
            log::trace!(
                "          + Load index buffer of primitive #{}",
                primitive.index(),
            );
            let index_buffer = if let Some(view) = indices.view() {
                let format = data_type_to_index_format(indices.data_type());
                let handle = self.load_buffer_from_view(
                    &view,
                    XrdsBufferType::Index(format),
                    context,
                    asset_server,
                )?;
                let buffer = asset_server.get_buffer(&handle).unwrap();
                let index_format = if let XrdsBufferType::Index(fmt) = buffer.ty() {
                    log::trace!(
                        "            + range={:?}, format={:?}",
                        indices.offset()
                            ..indices.offset()
                                + indices.count()
                                    * view.stride().unwrap_or(format.byte_size() as usize),
                        fmt
                    );
                    fmt
                } else {
                    panic!("Invalid index buffer type")
                };

                Some(XrdsIndexBuffer {
                    buffer,
                    index_format,
                    offset: indices.offset(),
                    count: indices.count(),
                })
            } else {
                todo!("Make empty index?");
            };
            log::trace!(
                "          + Load index buffer of primitive #{} in {} secs",
                primitive.index(),
                instant.elapsed().as_secs_f32()
            );
            index_buffer
        } else {
            None
        };
        let material = self.load_material(
            &vertex_buffers,
            &options,
            &primitive.material(),
            primitive.mode(),
            context,
            asset_server,
        )?;

        let xrds_primitive = XrdsPrimitive {
            vertices: vertex_buffers,
            indices: index_buffer,
            material,
            position_index,
        };
        log::trace!(
            "        + Load primitive #{} in {} secs",
            primitive.index(),
            instant.elapsed().as_secs_f32()
        );
        Ok(xrds_primitive)
    }

    fn load_material(
        &self,
        vertex_buffers: &[XrdsVertexBuffer],
        vertex_input_options: &pbr::PbrVertexInputOption,
        material: &gltf::Material,
        mode: gltf::mesh::Mode,
        context: &GltfLoadContext,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<AssetHandle<XrdsMaterialInstance>> {
        // Material instance name
        let instant = Instant::now();
        let name = Self::name_from_material(material, &context.gltf_name);
        let handle = if let Some(handle) =
            asset_server.get_material_instance_handle(&AssetId::Key(name.clone()))
        {
            log::trace!("        + Load material '{}' from cache", name);
            handle
        } else {
            log::trace!("        + Load material '{}'", name);
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
            };
            // Get material unique id from its options.
            let material_id = AssetId::Key(options.as_hash());
            let xrds_material =
                if let Some(material) = asset_server.get_material_by_id(&material_id) {
                    material
                } else {
                    // load new material
                    let handle = asset_server.register_material(&MaterialAssetInfo {
                        id: &material_id,
                        options: &options,
                        vertex_buffers,
                    })?;
                    asset_server.get_material(&handle).unwrap().clone()
                };

            let get_texture = |index: usize| {
                context
                    .textures
                    .get(index)
                    .map(|handle| {
                        log::trace!("          + Get texture #{}", index);
                        asset_server.get_texture(handle).unwrap().clone()
                    })
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
            let id = AssetId::Key(name.clone());
            let mut material_input = PbrMaterialInfo::new(&id);
            material_input.id = &id;
            material_input.params.base_color_factor =
                material.pbr_metallic_roughness().base_color_factor().into();
            if let Some(base_color_texture) = material.pbr_metallic_roughness().base_color_texture()
            {
                material_input.params.texcoord_base_color = base_color_texture.tex_coord();
                material_input.base_color_texture = Some(MaterialTextureInfo {
                    texture: get_texture(base_color_texture.texture().source().index()),
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
                    texture: get_texture(metallic_roughness_texture.texture().source().index()),
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
                    texture: get_texture(normal_texture.texture().source().index()),
                    sampler: get_sampler(normal_texture.texture().sampler().index()),
                });
            }
            if let Some(occlusion_texture) = material.occlusion_texture() {
                material_input.params.texcoord_occlusion = occlusion_texture.tex_coord();
                material_input.params.occlusion_strength = occlusion_texture.strength();
                material_input.occlusion_texture = Some(MaterialTextureInfo {
                    texture: get_texture(occlusion_texture.texture().source().index()),
                    sampler: get_sampler(occlusion_texture.texture().sampler().index()),
                });
            }
            if let Some(emissive_texture) = material.emissive_texture() {
                material_input.params.texcoord_emissive = emissive_texture.tex_coord();
                material_input.emissive_texture = Some(MaterialTextureInfo {
                    texture: get_texture(emissive_texture.texture().source().index()),
                    sampler: get_sampler(emissive_texture.texture().sampler().index()),
                });
            }
            material_input.params.alpha_cutoff = material.alpha_cutoff().unwrap_or(0.5);

            let handle =
                asset_server.register_material_instance(&xrds_material, &material_input)?;

            log::trace!(
                "        + Load material '{}' in {} secs",
                name,
                instant.elapsed().as_secs_f32()
            );

            handle
        };

        // let material_instance = asset_server.get_material_instance(&handle).unwrap().clone(); // must be exists

        Ok(handle)
    }

    fn load_vertex_buffers_from_primitive(
        &self,
        primitive: &gltf::Primitive,
        context: &GltfLoadContext,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<(
        Vec<XrdsVertexBuffer>,
        pbr::PbrVertexInputOption,
        Option<usize>,
    )> {
        let mut vertex_input_option = pbr::PbrVertexInputOption::default();

        let mut res = Vec::new();
        let mut position_index = None;
        for (i, (semantic, accessor)) in primitive.attributes().enumerate() {
            let instant = Instant::now();
            Self::update_vertex_input_options(&mut vertex_input_option, &semantic, &accessor);

            if semantic == gltf::mesh::Semantic::Positions {
                position_index = Some(i);
            }

            let format = vertex_format_from_data_type(accessor.data_type(), accessor.dimensions())?;
            log::trace!(
                "          + Load vertex buffer #{} ty={:?}, format={:?}",
                i,
                semantic,
                format
            );
            let vertex_attribute = wgpu::VertexAttribute {
                offset: 0, // discreted vertex must be started at offset 0
                format,
                shader_location: pbr::PbrVertexSemantic::from(semantic).location(),
            };
            if let Some(view) = accessor.view() {
                let handle = self.load_buffer_from_view(
                    &view,
                    XrdsBufferType::Vertex(format),
                    context,
                    asset_server,
                )?;
                let buffer = XrdsVertexBuffer {
                    buffer: asset_server.get_buffer(&handle).unwrap(),
                    vertex_attributes: [vertex_attribute],
                    offset: accessor.offset(),
                    count: accessor.count(),
                };
                log::trace!(
                    "            + range={:?}, format={:?}",
                    accessor.offset()
                        ..accessor.offset()
                            + accessor.count() * view.stride().unwrap_or(format.size() as usize),
                    format
                );
                res.push(buffer);
            } else {
                todo!("Generate empty buffer")
            }
            log::trace!(
                "          - Load vertex buffers #{} in {} secs",
                i,
                instant.elapsed().as_secs_f32()
            );
        }

        Ok((res, vertex_input_option, position_index))
    }

    fn load_buffer_from_view(
        &self,
        view: &gltf::buffer::View,
        buffer_type: XrdsBufferType,
        context: &GltfLoadContext,
        asset_server: &mut AssetServer,
    ) -> anyhow::Result<AssetHandle<XrdsBuffer>> {
        let name = Self::name_from_buffer_view(view, &context.gltf_name);

        let handle =
            if let Some(buffer) = asset_server.get_buffer_handle(&AssetId::Key(name.clone())) {
                log::trace!("            # Load buffer '{}' from cache", name);
                buffer
            } else {
                let instant = Instant::now();
                let raw_buffer: &Vec<u8> = &context.raw_buffers[view.buffer().index()];
                let offset = view.offset();
                let length = view.length();
                log::trace!("            + Load buffer '{}'", name);

                let is_index_and_u8 = match buffer_type {
                    XrdsBufferType::Index(fmt) => match fmt {
                        IndexFormat::U8 => true,
                        _ => false,
                    },
                    _ => false,
                };

                let registerd_buffer = if is_index_and_u8 {
                    let converted = raw_buffer[offset..(offset + length)]
                        .iter()
                        .map(|i| *i as u16)
                        .collect::<Vec<_>>();
                    asset_server.register_buffer(&BufferAssetInfo {
                        id: &AssetId::Key(name.clone()),
                        data: bytemuck::cast_slice(&converted),
                        ty: XrdsBufferType::Index(IndexFormat::U16),
                        stride: view.stride().map(|v| v as u64),
                    })?
                } else {
                    asset_server.register_buffer(&BufferAssetInfo {
                        id: &AssetId::Key(name.clone()),
                        data: &raw_buffer[offset..(offset + length)],
                        ty: buffer_type,
                        stride: view.stride().map(|v| v as u64),
                    })?
                };

                log::trace!(
                    "            - load buffer '{}' in {} secs",
                    name,
                    instant.elapsed().as_secs_f32()
                );
                registerd_buffer
            };
        Ok(handle)
    }

    fn load_buffer(
        gltf: &gltf::Gltf,
        buffer: &gltf::Buffer,
        asset_path: &Path,
    ) -> anyhow::Result<Vec<u8>> {
        let instant = Instant::now();
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

        log::trace!(
            "    + Load raw buffer #{} in {} secs",
            buffer.index(),
            instant.elapsed().as_secs_f32()
        );
        Ok(data)
    }

    fn load_image(
        image: &gltf::Image<'_>,
        raw_buffers: &[Vec<u8>],
        asset_path: &Path,
        super_key: &str,
        asset_server: &AssetServer,
    ) -> anyhow::Result<LoadImageResult> {
        let name = Self::name_from_image(image, super_key)?;
        let handle = if let Some(handle) =
            asset_server.get_texture_handle(&AssetId::Key(name.clone()))
        {
            log::trace!("    + Load image '{}' from cache", name);
            LoadImageResult::Cached(handle)
        } else {
            let instant = Instant::now();
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
                    let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
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
            let res = LoadImageResult::Loaded {
                id: AssetId::Key(name.clone()),
                data,
                width,
                height,
                depth_or_array,
            };

            log::trace!(
                "    + Load image '{}' in {} secs",
                name,
                instant.elapsed().as_secs_f32()
            );
            res
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
            wgpu::FilterMode::Linear
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
        let anisotropy_clamp = if mag_filter == wgpu::FilterMode::Linear
            && min_filter == wgpu::FilterMode::Linear
            && mipmap_filter == wgpu::FilterMode::Linear
        {
            16
        } else {
            1
        };
        let wgpu_sampler =
            self.graphics_instance
                .device()
                .create_sampler(&wgpu::SamplerDescriptor {
                    label: None,
                    address_mode_u,
                    address_mode_v,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter,
                    min_filter,
                    mipmap_filter,
                    anisotropy_clamp,
                    ..Default::default()
                });

        wgpu_sampler
    }

    async fn read_file<P>(path: P) -> anyhow::Result<Vec<u8>>
    where
        P: AsRef<Path>,
    {
        let mut buf = Vec::new();
        let mut f = File::open(path)?;
        f.read_to_end(&mut buf)?;

        Ok(buf)
    }
}

/// Utilities
impl GltfLoader {
    fn name_from_image(image: &gltf::Image, parent_name: &str) -> anyhow::Result<String> {
        let name = match image.source() {
            gltf::image::Source::View { view, mime_type: _ } => {
                if let Some(name) = view.name() {
                    format!("{}.{}", parent_name, name)
                } else {
                    format!("{}.image.{}", parent_name, image.index())
                }
            }
            gltf::image::Source::Uri { uri, mime_type: _ } => {
                let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
                format!("{}.{}", parent_name, decoded_uri)
            }
        };
        Ok(name)
    }

    fn name_from_scene(scene: &gltf::Scene, parent_name: &str) -> String {
        if let Some(name) = scene.name() {
            format!("{}.{}", parent_name, name)
        } else {
            format!("{}.scene.{}", parent_name, scene.index())
        }
    }

    fn name_from_node(node: &gltf::Node, parent_name: &str) -> String {
        if let Some(name) = node.name() {
            format!("{}.{}", parent_name, name)
        } else {
            format!("{}.node.{}", parent_name, node.index())
        }
    }

    fn name_from_mesh(mesh: &gltf::Mesh, parent_name: &str) -> String {
        if let Some(name) = mesh.name() {
            format!("{}.{}", parent_name, name)
        } else {
            format!("{}.mesh.{}", parent_name, mesh.index())
        }
    }

    fn name_from_camera(camera: &gltf::Camera, parent_name: &str) -> String {
        if let Some(name) = camera.name() {
            format!("{}.{}", parent_name, name)
        } else {
            format!("{}.camera.{}", parent_name, camera.index())
        }
    }

    fn name_from_buffer_view(view: &gltf::buffer::View, parent_name: &str) -> String {
        if let Some(name) = view.name() {
            format!("{}.{}", parent_name, name)
        } else {
            format!("{}.buffer_view.{}", parent_name, view.index(),)
        }
    }

    fn name_from_material(material: &gltf::Material, parent_name: &str) -> String {
        if let Some(name) = material.name() {
            format!("{}.{}", parent_name, name)
        } else if let Some(index) = material.index() {
            format!("{}.material.{}", parent_name, index)
        } else {
            format!("{}.material.default", parent_name)
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
fn data_type_to_index_format(data_type: gltf::accessor::DataType) -> IndexFormat {
    match data_type {
        gltf::accessor::DataType::U8 => IndexFormat::U8,
        gltf::accessor::DataType::U16 => IndexFormat::U16,
        gltf::accessor::DataType::U32 => IndexFormat::U32,
        _ => {
            log::warn!(
                "Unsupported index format type {:?}. Assume uint16",
                data_type
            );
            IndexFormat::U16
        }
    }
}
