use std::{collections::HashMap, num::NonZeroU32, sync::Arc};

use glam::{quat, vec3, Quat, Vec3};
use log::{debug, info, warn};
use openxr::{Posef, ViewConfigurationType};
use xrds_core::Transform;
use xrds_graphics::{Fov, GraphicsApi, GraphicsInstance, TextureFormat, XrdsTexture};

use crate::OpenXrError;

use super::{
    api::{self, OpenXrContextApi},
    BlendMode, FormFactor, View, ViewConfiguration,
};

pub struct OpenXrContext {
    instance: openxr::Instance,
    #[allow(dead_code)]
    system_id: openxr::SystemId,
    inner: Box<dyn OpenXrContextApi>,
    frame_waiter: openxr::FrameWaiter,
    session: openxr::Session<openxr::AnyGraphics>,
    swapchain_textures: Vec<XrdsTexture>,
    view_configurations: Vec<ViewConfiguration>,
    space_map: HashMap<i32, Arc<openxr::Space>>,
    event_buffer: openxr::EventDataBuffer,
    graphics_instance: GraphicsInstance,
    state: State,
}

struct State {
    is_running: bool,
    frame_state: openxr::FrameState,
    views: Vec<openxr::View>,
    selected_view_configuration: ViewConfiguration,
    selected_blend_mode: BlendMode,
    selected_space_type: openxr::ReferenceSpaceType,
}

struct OpenXrContextParams {
    name: String,
    form_factor: FormFactor,
    graphics_api: GraphicsApi,
}

#[derive(Default)]
pub struct OpenXrContextBuilder {
    params: OpenXrContextParams,
}

#[derive(Debug)]
pub struct XrCameraInfo {
    pub fov: Fov,
    /// Relative translation from center
    pub translation: glam::Vec3,
}

#[derive(Debug)]
pub struct XrRenderParams {
    pub swapchain_texture: XrdsTexture,
    pub xr_camera_infos: Vec<XrCameraInfo>,
    pub hmd_transform: Transform,
}

pub enum OpenXrOnPreRenderResult {
    DoRender(XrRenderParams),
    SkipRender,
    Exit,
}

/// implementation of public methods
impl OpenXrContext {
    /// Prepare openxr context for rendering.
    /// This method should be called before rendering.
    ///  1. Poll openxr events and begin session if openxr system is ready or stop session
    ///  2. Locate view and sync actions
    ///  3. Wait swapchain and return swapchain texture to render
    pub fn on_pre_render(&mut self) -> anyhow::Result<OpenXrOnPreRenderResult> {
        // Consume all events
        while let Some(event) = self.instance.poll_event(&mut self.event_buffer)? {
            match event {
                openxr::Event::SessionStateChanged(s) => {
                    info!("Session state changed: {:?}", s.state());

                    match s.state() {
                        openxr::SessionState::READY => {
                            self.session.begin(self.view_configuration_type())?;
                            self.state.is_running = true;
                        }
                        openxr::SessionState::STOPPING => {
                            self.session.end()?;
                            self.state.is_running = false;
                        }
                        openxr::SessionState::EXITING => {
                            self.session.end()?;
                            self.state.is_running = false;
                            return Ok(OpenXrOnPreRenderResult::Exit);
                        }
                        _ => {}
                    }
                }
                openxr::Event::ReferenceSpaceChangePending(r) => {
                    debug!(
                        "Reference space change pending: {:?}",
                        r.reference_space_type()
                    );
                }
                openxr::Event::InstanceLossPending(_) => {
                    return Ok(OpenXrOnPreRenderResult::SkipRender)
                }
                openxr::Event::EventsLost(e) => {
                    warn!("OpenXr lost {} events", e.lost_event_count());
                }
                _ => {
                    //Do extension event processing
                    // ex: self.fb_event_handle.on_xr_event(event)
                }
            }
        }

        // Block until previous frame is finished displaying
        let frame_state = self.frame_waiter.wait()?;
        self.state.frame_state = frame_state;
        self.inner.stream_begin()?;

        if !frame_state.should_render {
            let blend_mode = self.state.selected_blend_mode.into();
            let rect = self.get_view_rect()?;

            self.inner.stream_end(
                frame_state.predicted_display_time,
                blend_mode,
                rect,
                self.reference_space()?,
                &[],
            )?;
            return Ok(OpenXrOnPreRenderResult::SkipRender);
        }

        let image_index = self.inner.swapchain_wait()? as usize;
        let swapchain_texture = self
            .swapchain_textures
            .get(image_index)
            .ok_or(OpenXrError::IndexOutOfBounds {
                index: image_index,
                max: self.swapchain_textures.len(),
            })?
            .clone();

        let reference_space = self.reference_space()?;
        let (_flags, views) = self.session.locate_views(
            self.view_configuration_type(),
            frame_state.predicted_display_time,
            &reference_space,
        )?;

        let (transform, xr_camera_infos) = if views.len() > 1 {
            // Stereo case
            let (left_pos, orientation) = Self::to_engine_pos_and_orientation(&views[0].pose);
            let (right_pos, _) = Self::to_engine_pos_and_orientation(&views[1].pose);

            let center_pos = (left_pos + right_pos) * 0.5;
            let center_transform = Transform::default()
                .with_translation(center_pos)
                .with_rotation(orientation);

            let left_offset = left_pos - center_pos;
            let right_offset = right_pos - center_pos;

            let relative_left_pos = orientation.inverse() * left_offset;
            let relative_right_pos = orientation.inverse() * right_offset;

            let left_fov = Fov {
                left: views[0].fov.angle_left,
                right: views[0].fov.angle_right,
                up: views[0].fov.angle_up,
                down: views[0].fov.angle_down,
            };
            let right_fov = Fov {
                left: views[1].fov.angle_left,
                right: views[1].fov.angle_right,
                up: views[1].fov.angle_up,
                down: views[1].fov.angle_down,
            };

            let xr_camera_infos = vec![
                XrCameraInfo {
                    fov: left_fov,
                    translation: relative_left_pos,
                },
                XrCameraInfo {
                    fov: right_fov,
                    translation: relative_right_pos,
                },
            ];
            (center_transform, xr_camera_infos)
        } else if views.len() == 1 {
            // Mono case
            let (pos, orientation) = Self::to_engine_pos_and_orientation(&views[0].pose);

            let center_transform = Transform::default()
                .with_translation(pos)
                .with_rotation(orientation);

            let fov = Fov {
                left: views[0].fov.angle_left,
                right: views[0].fov.angle_right,
                up: views[0].fov.angle_up,
                down: views[0].fov.angle_down,
            };
            let xr_camera_infos = vec![XrCameraInfo {
                fov,
                translation: Vec3::ZERO,
            }];
            (center_transform, xr_camera_infos)
        } else {
            (Transform::default(), vec![])
        };

        self.state.views = views;

        Ok(OpenXrOnPreRenderResult::DoRender(XrRenderParams {
            swapchain_texture,
            xr_camera_infos,
            hmd_transform: transform,
        }))
    }

    fn to_engine_pos_and_orientation(pose: &openxr::Posef) -> (glam::Vec3, glam::Quat) {
        let pos: openxr::Vector3f = pose.position;
        let ori = pose.orientation;

        let position = vec3(-pos.x, pos.y, -pos.z);
        let orientation =
            Quat::from_rotation_x(180.0f32.to_radians()) * quat(ori.w, ori.z, ori.y, ori.x);

        (position, orientation)
    }

    pub fn on_post_render(&mut self) -> anyhow::Result<()> {
        self.inner.swapchain_release_image()?;
        self.inner.stream_end(
            self.state.frame_state.predicted_display_time,
            self.state.selected_blend_mode.into(),
            self.get_view_rect()?,
            self.reference_space()?,
            &self.state.views,
        )?;
        Ok(())
    }

    pub fn swapchain_format(&self) -> anyhow::Result<TextureFormat> {
        self.inner.swapchain_format()
    }

    pub fn swapchain_extent(&self) -> anyhow::Result<wgpu::Extent3d> {
        self.inner.swapchain_extent()
    }

    pub fn is_running(&self) -> bool {
        self.state.is_running
    }

    pub fn enumerate_view_configurations(
        &self,
    ) -> std::iter::Enumerate<std::slice::Iter<'_, ViewConfiguration>> {
        self.view_configurations.iter().enumerate()
    }

    pub fn switch_view_configuration(&mut self, idx: usize) -> anyhow::Result<()> {
        self.state.selected_view_configuration = self
            .view_configurations
            .get(idx)
            .ok_or(OpenXrError::NoViewTypeAvailable)?
            .clone();
        Ok(())
    }
}

/// implementation of private methods
impl OpenXrContext {
    fn get_view_rect(&self) -> anyhow::Result<openxr::Rect2Di> {
        let view = self
            .state
            .selected_view_configuration
            .views
            .first()
            .ok_or(OpenXrError::NoViewTypeAvailable)?;
        Ok(openxr::Rect2Di {
            offset: openxr::Offset2Di { x: 0, y: 0 },
            extent: openxr::Extent2Di {
                width: view.recommended_image_size.width as _,
                height: view.recommended_image_size.height as _,
            },
        })
    }

    fn view_configuration_type(&self) -> ViewConfigurationType {
        self.state.selected_view_configuration.ty.into()
    }
}

impl OpenXrContext {
    pub fn builder() -> OpenXrContextBuilder {
        OpenXrContextBuilder::default()
    }

    fn new(params: OpenXrContextParams) -> anyhow::Result<OpenXrContext> {
        let entry = openxr::Entry::linked();

        let (instance, system_id) = Self::create_openxr_instance(&entry, &params)?;

        let res = match params.graphics_api {
            GraphicsApi::Vulkan => api::vulkan::OpenXrVulkanContext::create(&instance, system_id)?,
            GraphicsApi::D3d12 | GraphicsApi::OpenGles => {
                todo!()
            }
        };
        let mut inner = res.context;
        let frame_waiter = res.frame_waiter;
        let graphics_instance = res.graphics_instance;
        let session = inner.session();

        let view_configurations = Self::enumerate_views(&instance, system_id)?;
        let selected_view_configuration = view_configurations
            .first()
            .ok_or(OpenXrError::NoViewTypeAvailable)?
            .clone();
        let selected_blend_mode = selected_view_configuration
            .blend_modes
            .first()
            .ok_or(OpenXrError::NoBlendModeAvailable)?
            .to_owned();

        // Enable multiview if openxr has multiple views
        let graphics_instance = if selected_view_configuration.views.len() > 1 {
            graphics_instance
                .with_multiview(NonZeroU32::new(selected_view_configuration.views.len() as _))
        } else {
            graphics_instance
        };

        let swapchain_textures = inner
            .as_mut()
            .create_swapchain(&selected_view_configuration, &graphics_instance)?;

        log::info!(
            "OpenXR swapchain texture size={}x{}, format={:?}",
            swapchain_textures[0].size().width,
            swapchain_textures[0].size().height,
            swapchain_textures[0].format()
        );

        let mut space_map = HashMap::new();
        for ty in session.enumerate_reference_spaces()? {
            let space = Arc::new(session.create_reference_space(ty, Posef::IDENTITY)?);
            space_map.insert(ty.into_raw(), space);
        }
        let selected_space_type = openxr::ReferenceSpaceType::STAGE;
        let event_buffer = openxr::EventDataBuffer::new();

        debug!("OpenXr context created");

        Ok(OpenXrContext {
            instance,
            system_id,
            inner,
            frame_waiter,
            session,
            swapchain_textures,
            view_configurations,
            event_buffer,
            graphics_instance,
            space_map,
            state: State {
                is_running: false,
                frame_state: openxr::FrameState {
                    predicted_display_period: openxr::Duration::NONE,
                    predicted_display_time: openxr::Time::from_nanos(0),
                    should_render: false,
                },
                views: vec![],
                selected_view_configuration,
                selected_blend_mode,
                selected_space_type,
            },
        })
    }

    fn create_openxr_instance(
        entry: &openxr::Entry,
        params: &OpenXrContextParams,
    ) -> anyhow::Result<(openxr::Instance, openxr::SystemId)> {
        // Enumerate extensions and layers
        let _available_extensions = entry.enumerate_extensions()?;
        let _available_layers = entry.enumerate_layers()?;
        debug!("Available layers: {:?}", _available_layers);

        // Initialize OpenXr instance and get system id
        let mut enabled_extensions = openxr::ExtensionSet::default();
        match params.graphics_api {
            GraphicsApi::Vulkan => {
                enabled_extensions.khr_vulkan_enable = true;
                enabled_extensions.khr_vulkan_enable2 = true;
            }
            GraphicsApi::D3d12 => {
                enabled_extensions.khr_d3d12_enable = true;
            }
            GraphicsApi::OpenGles => {
                enabled_extensions.khr_opengl_enable = true;
                enabled_extensions.khr_opengl_es_enable = true;
            }
        }
        #[cfg(debug_assertions)]
        {
            enabled_extensions.ext_debug_utils = true;
        }

        let xr_instance = entry.create_instance(
            &openxr::ApplicationInfo {
                application_name: &params.name,
                ..Default::default()
            },
            &enabled_extensions,
            &[],
        )?;

        let xr_system = xr_instance.system(params.form_factor.into())?;

        Ok((xr_instance, xr_system))
    }

    fn enumerate_views(
        instance: &openxr::Instance,
        system_id: openxr::SystemId,
    ) -> anyhow::Result<Vec<ViewConfiguration>> {
        let view_configurations = instance.enumerate_view_configurations(system_id)?;

        let mut results = Vec::new();
        for ty in view_configurations {
            let view_configuration_views =
                instance.enumerate_view_configuration_views(system_id, ty)?;
            let properties = instance.view_configuration_properties(system_id, ty)?;

            let views: Vec<_> = view_configuration_views
                .iter()
                .map(|view| View {
                    recommended_image_size: wgpu::Extent3d {
                        width: view.recommended_image_rect_width,
                        height: view.recommended_image_rect_height,
                        depth_or_array_layers: 1,
                    },
                    recommended_swapchain_sample_count: view.recommended_swapchain_sample_count,
                    max_image_size: wgpu::Extent3d {
                        width: view.max_image_rect_width,
                        height: view.max_image_rect_height,
                        depth_or_array_layers: 1,
                    },
                    max_swapchain_sample_count: view.max_swapchain_sample_count,
                })
                .collect();

            let blend_modes: Vec<_> = instance
                .enumerate_environment_blend_modes(system_id, ty)?
                .iter()
                .map(|b| b.to_owned().into())
                .collect();

            results.push(ViewConfiguration {
                ty: ty.into(),
                views,
                blend_modes,
                fov_mutable: properties.fov_mutable,
            });
        }

        Ok(results)
    }

    pub fn graphics_instance(&self) -> &GraphicsInstance {
        &self.graphics_instance
    }

    fn reference_space(&self) -> anyhow::Result<Arc<openxr::Space>> {
        Ok(self
            .space_map
            .get(&self.state.selected_space_type.into_raw())
            .ok_or(OpenXrError::ReferenceSpaceNotAvailable(
                self.state.selected_space_type.into_raw(),
            ))?
            .clone())
    }
}

impl Default for OpenXrContextParams {
    fn default() -> Self {
        Self {
            name: "Unnamed openxr application".to_owned(),
            form_factor: FormFactor::HeadMountedDisplay,
            graphics_api: GraphicsApi::Vulkan,
        }
    }
}

impl OpenXrContextBuilder {
    pub fn with_application_name(mut self, name: &str) -> Self {
        self.params.name = name.to_owned();
        self
    }

    pub fn with_form_factor(mut self, form_factor: FormFactor) -> Self {
        self.params.form_factor = form_factor;
        self
    }

    pub fn with_graphics_api(mut self, graphics_api: GraphicsApi) -> Self {
        if graphics_api != GraphicsApi::Vulkan {
            panic!("Currently support vulkan api only");
        }
        self.params.graphics_api = graphics_api;
        self
    }

    pub fn build(self) -> anyhow::Result<OpenXrContext> {
        OpenXrContext::new(self.params)
    }
}

impl std::fmt::Debug for OpenXrContext {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
