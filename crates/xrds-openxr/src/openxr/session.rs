use std::ptr::{self, null, null_mut};

use bevy::{
    camera::{ManualTextureViewHandle, RenderTarget},
    prelude::*,
    render::{extract_resource::ExtractResource, texture::ManualTextureView},
};
use openxr::{sys::ReferenceSpaceCreateInfo, Posef, SpaceLocation, StructureType, ViewStateFlags};

use crate::{
    backends::OpenXrGraphicsBackends,
    openxr::{
        camera::{OpenXrCameraIndex, OpenXrViewProjection},
        graphics::{
            openxr_graphics, OpenXrGraphicsExtend, OpenXrGraphicsFamily, OpenXrGraphicsWrap,
        },
        helper::{cvt, get_arr_init},
        layers::{
            builder::OpenXrCompositionLayerBuilder,
            projection::OpenXrCompositionLayerProjectionBuilder,
        },
        resources::{
            OpenXrEnvironmentBlendModes, OpenXrFrameStream, OpenXrInstance, OpenXrRenderResources,
            OpenXrSpace, OpenXrSwapchain, OpenXrSwapchainImages, OpenXrViewConfigurations,
            OpenXrViews,
        },
        schedule::{
            openxr_in_state_focused, OpenXrDeviceState, OpenXrRuntimeSystems, OpenXrSchedules,
            OpenXrSessionState, OpenXrSystemState,
        },
        swapchain::view_index,
    },
};

pub struct OpenXrSessionCreateInfo(pub OpenXrGraphicsWrap<Self>);

impl OpenXrGraphicsFamily for OpenXrSessionCreateInfo {
    type Inner<G: OpenXrGraphicsExtend> = G::SessionCreateInfo;
}

impl OpenXrSessionCreateInfo {
    pub fn from_inner<G: OpenXrGraphicsExtend>(session_create_info: G::SessionCreateInfo) -> Self {
        Self(G::wrap(session_create_info))
    }
}

#[derive(Resource, ExtractResource, Clone)]
pub struct OpenXrSession(pub OpenXrGraphicsWrap<Self>);

impl OpenXrGraphicsFamily for OpenXrSession {
    type Inner<G: OpenXrGraphicsExtend> = openxr::Session<G>;
}

impl OpenXrSession {
    pub fn from_inner<G: OpenXrGraphicsExtend>(session: openxr::Session<G>) -> Self {
        Self(G::wrap(session))
    }

    #[inline]
    pub fn begin(
        &self,
        view_configuration_type: openxr::ViewConfigurationType,
    ) -> openxr::Result<openxr::sys::Result> {
        openxr_graphics!(
            &self.0;
            inner => {
                inner.begin(view_configuration_type)
            }
        )
    }

    #[inline]
    pub fn end(&self) -> openxr::Result<openxr::sys::Result> {
        openxr_graphics!(
            &self.0;
            inner => {
                inner.end()
            }
        )
    }

    #[inline]
    pub fn locate_views(
        &self,
        view_configuration_type: openxr::ViewConfigurationType,
        display_time: openxr::Time,
        space: &OpenXrSpace,
    ) -> openxr::Result<(openxr::ViewStateFlags, Vec<openxr::View>)> {
        openxr_graphics!(
            &self.0;
            inner => {
                let info = openxr::sys::ViewLocateInfo {
                    ty: openxr::sys::ViewLocateInfo::TYPE,
                    next: null(),
                    view_configuration_type,
                    display_time,
                    space: openxr::sys::Space::from_raw(space.0),
                };
                let (flags, raw) = unsafe {
                    let mut out = openxr::sys::ViewState::out(null_mut());
                    let raw = get_arr_init(openxr::sys::View::out(null_mut()), |cap, count, buf| {
                        (inner.instance().fp().locate_views)(
                            inner.as_raw(),
                            &info,
                            out.as_mut_ptr(),
                            cap,
                            count,
                            buf as _,
                        )
                    })?;
                    (out.assume_init().view_state_flags, raw)
                };
                Ok((
                    flags,
                    raw.iter()
                        .map(|x| unsafe {
                            let ptr = x.as_ptr();
                            openxr::View {
                                pose: Posef {
                                    orientation: if flags.contains(ViewStateFlags::ORIENTATION_VALID) {
                                        *std::ptr::addr_of!((*ptr).pose.orientation)
                                    } else {
                                        Default::default()
                                    },
                                    position: if flags.contains(ViewStateFlags::POSITION_VALID) {
                                        *std::ptr::addr_of!((*ptr).pose.position)
                                    } else {
                                        Default::default()
                                    },
                                },
                                fov: *std::ptr::addr_of!((*ptr).fov),
                        } })
                        .collect(),
                ))
            }
        )
    }

    #[inline]
    #[allow(unused)]
    pub fn locate_space(
        &self,
        space: &OpenXrSpace,
        base: &OpenXrSpace,
        time: openxr::Time,
    ) -> openxr::Result<openxr::SpaceLocation> {
        openxr_graphics!(
            &self.0;
            inner => {
                let mut out = openxr::sys::SpaceLocation::out(null_mut());
                unsafe {
                    cvt((inner.instance().fp().locate_space)(
                        openxr::sys::Space::from_raw(space.0),
                        openxr::sys::Space::from_raw(base.0),
                        time,
                        out.as_mut_ptr()
                    ))?;
                    let ptr = out.as_ptr();
                    let flags = *ptr::addr_of!((*ptr).location_flags);
                    Ok(SpaceLocation {
                        location_flags: flags,
                        pose: Posef {
                            orientation: if flags.contains(openxr::sys::SpaceLocationFlags::ORIENTATION_VALID) {
                                     *ptr::addr_of!((*ptr).pose.orientation)
                                } else {
                                    Default::default()
                                },
                            position: if flags.contains(openxr::sys::SpaceLocationFlags::POSITION_VALID) {
                                    *ptr::addr_of!((*ptr).pose.position)
                                } else {
                                    Default::default()
                                }
                            }
                        }
                    )
                }
            }
        )
    }

    #[inline]
    pub fn enumerate_reference_space_types(
        &self,
    ) -> openxr::Result<Vec<openxr::ReferenceSpaceType>> {
        openxr_graphics!(
            &self.0;
            inner => {
                inner.enumerate_reference_spaces()
            }
        )
    }

    #[inline]
    pub fn create_reference_space(
        &self,
        reference_space_type: openxr::ReferenceSpaceType,
        pose_in_reference_space: openxr::Posef,
    ) -> openxr::Result<OpenXrSpace> {
        openxr_graphics!(
            &self.0;
            inner => {
                let mut space = openxr::sys::Space::NULL;
                unsafe {
                    (inner.instance().fp().create_reference_space)(
                        inner.as_raw(), &ReferenceSpaceCreateInfo {
                            ty: StructureType::REFERENCE_SPACE_CREATE_INFO,
                            next: null(),
                            reference_space_type,
                            pose_in_reference_space
                        },
                        &mut space
                    )
                };
                Ok(OpenXrSpace(space.into_raw()))
            }
        )
    }

    #[inline]
    #[allow(unused)]
    pub fn reference_space_bounds_rect(
        &self,
        ty: openxr::ReferenceSpaceType,
    ) -> openxr::Result<Option<openxr::Extent2Df>> {
        openxr_graphics!(
            &self.0;
            inner => {
                inner.reference_space_bounds_rect(ty)
            }
        )
    }
}

pub struct OpenXrSessionPlugin;

impl Plugin for OpenXrSessionPlugin {
    fn build(&self, app: &mut App) {
        // Start session create schedule when app startup
        app.add_systems(Startup, |world: &mut World| {
            world.run_schedule(OpenXrSchedules::SessionCreate);
        });

        // Session create schedule
        app.add_systems(
            OpenXrSchedules::SessionCreate,
            (
                initialize_view_and_blend_mode,
                initialize_action_space,
                initialize_openxr_session,
            )
                .in_set(OpenXrRuntimeSystems::SessionCreate),
        )
        .add_systems(
            OpenXrSchedules::SessionCreate,
            (init_render_resources, finish_session_create, spawn_camera)
                .chain()
                .in_set(OpenXrRuntimeSystems::PostSessionCreate),
        );

        // Session update schedule
        app.add_systems(
            OpenXrSchedules::Update,
            handle_events.in_set(OpenXrRuntimeSystems::HandleEvents),
        )
        .add_systems(
            OpenXrSchedules::Update,
            (
                begin_openxr_session.run_if(resource_equals(OpenXrSessionState::Ready)),
                end_openxr_session.run_if(resource_equals(OpenXrSessionState::Stopping)),
            )
                .in_set(OpenXrRuntimeSystems::UpdateSessionStates),
        )
        .add_systems(
            OpenXrSchedules::Update,
            (
                sync_actions
                // xrSyncActions
                // xrGetActionStateBoolean
                // xrGetActionStateFloat
                // xrGetActionStateVector2f
                // xrGetActionStatePose
                // xrLocateSpace
                // xrApplyHapticFeedback
                // xrStopHapticFeedback
                // xrRequestExitSession
            )
                .in_set(OpenXrRuntimeSystems::PreFrameLoop)
                .run_if(openxr_in_state_focused),
        );
    }
}

fn initialize_view_and_blend_mode(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    let openxr_instance = world.resource::<OpenXrInstance>();

    let view_configurations = openxr_instance
        .enumerate_view_configurations()
        .expect("Could not enumerate view configuration types");
    let view_configuration_type = view_configurations
        .first()
        .expect("There is no view configuration types");
    let view_configuration_views = openxr_instance
        .enumerate_view_configuration_views(view_configuration_type)
        .expect("Could not enumerate views of view configuration type");
    let blend_modes = openxr_instance
        .enumerate_environment_blend_modes(view_configuration_type)
        .expect("Could not enumerate environment blend modes");
    let blend_mode = blend_modes.first().expect("There is no blend modes");

    let openxr_views = OpenXrViews(vec![
        openxr::View::default();
        view_configuration_views.len()
    ]);

    let openxr_view_configurations = OpenXrViewConfigurations {
        view_configuration_type: *view_configuration_type,
        view_configuration_views,
    };

    let openxr_blend_modes = OpenXrEnvironmentBlendModes {
        current_blend_mode: *blend_mode,
        blend_modes,
    };

    let mut openxr_layer_builder = OpenXrCompositionLayerBuilder::new();
    openxr_layer_builder.insert_layer(0, Box::new(OpenXrCompositionLayerProjectionBuilder));

    // TODO: Create action set here

    info!("OpenXR system initialized");
    world.insert_resource(openxr_views);
    world.insert_resource(openxr_view_configurations);
    world.insert_resource(openxr_blend_modes);
    world.insert_resource(openxr_layer_builder);
}

fn initialize_openxr_session(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    let openxr_instance = world.resource::<OpenXrInstance>();
    let graphics_backends = world.resource::<OpenXrGraphicsBackends>();

    let session_create_info = graphics_backends
        .get_session_create_info()
        .expect("Could not get openxr session create info");

    let (session, frame_waiter, frame_stream) = openxr_instance
        .create_session(&session_create_info)
        .expect("Could not create OpenXR session");
    info!("OpenXR session created");

    world.insert_resource(session);
    world.insert_resource(frame_waiter);
    world.insert_resource(frame_stream);
}

fn finish_session_create(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    world.insert_resource(OpenXrSystemState::SessionCreated);
}

fn initialize_action_space(_world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    trace!("OpenXR action space and attach created");
}

fn begin_openxr_session(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    let openxr_session = world.resource::<OpenXrSession>();
    let view_configurations = world.resource::<OpenXrViewConfigurations>();

    info!(
        "Begin OpenXR session with view type: {:?}",
        view_configurations.view_configuration_type
    );

    openxr_session
        .begin(view_configurations.view_configuration_type)
        .expect("Could not begin OpenXR session");

    world.insert_resource(OpenXrSessionState::Running);
}

fn end_openxr_session(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");

    let openxr_session = world.resource::<OpenXrSession>();
    openxr_session.end().expect("Could not end OpenXR session");
    world.insert_resource(OpenXrSessionState::Idle);
}

fn handle_events(world: &mut World) {
    let openxr_instance = world.resource::<OpenXrInstance>();

    let mut storage = openxr::EventDataBuffer::new();
    let event = openxr_instance
        .poll_event(&mut storage)
        .expect("Could not poll openxr event");
    trace!("handle_events");
    if let Some(event) = event {
        match event {
            openxr::Event::SessionStateChanged(state) => {
                info!(
                    "  session state changed: {:?}, time: {:?}",
                    state.state(),
                    state.time()
                );
                match state.state() {
                    openxr::SessionState::IDLE => {
                        world.insert_resource(OpenXrSessionState::Idle);
                    }
                    openxr::SessionState::READY => {
                        world.insert_resource(OpenXrSessionState::Ready);
                    }
                    openxr::SessionState::STOPPING => {
                        world.insert_resource(OpenXrSessionState::Stopping);
                    }
                    openxr::SessionState::LOSS_PENDING => {
                        world.insert_resource(OpenXrSessionState::LossPending);
                    }
                    openxr::SessionState::EXITING => {
                        world.insert_resource(OpenXrSessionState::Exiting);
                    }
                    openxr::SessionState::SYNCHRONIZED => {
                        world.insert_resource(OpenXrDeviceState::Synchronized);
                    }
                    openxr::SessionState::VISIBLE => {
                        world.insert_resource(OpenXrDeviceState::Visible);
                    }
                    openxr::SessionState::FOCUSED => {
                        world.insert_resource(OpenXrDeviceState::Focused);
                    }
                    _ => {}
                }
            }
            openxr::Event::ReferenceSpaceChangePending(reference_space_change_pending) => {
                reference_space_change_pending.change_time();
                reference_space_change_pending.pose_in_previous_space();
                reference_space_change_pending.pose_valid();
                reference_space_change_pending.reference_space_type();
                info!(
                    "  reference space change pending: time={:?}, prev_pose={:?}, valid={:?}, type={:?}",
                    reference_space_change_pending.change_time(), reference_space_change_pending.pose_in_previous_space(), reference_space_change_pending.pose_valid(), reference_space_change_pending.reference_space_type()
                );
            }
            openxr::Event::EventsLost(event_lost) => {
                warn!("  events lost: {}", event_lost.lost_event_count());
            }
            openxr::Event::InstanceLossPending(instance_loss_pending) => {
                warn!(
                    "  intsnace loss pending: {:?}",
                    instance_loss_pending.loss_time()
                );
            }
            openxr::Event::InteractionProfileChanged(_interaction_profile_changed) => {
                info!("  Interaction profile has changed");
            }
            _ => {
                warn!("  Unimplemented event");
            }
        }
    }
}

fn init_render_resources(world: &mut World) {
    let frame_stream = world
        .remove_resource::<OpenXrFrameStream>()
        .expect("OpenXrFrameStream resource not exists");
    let swapchain = world
        .remove_resource::<OpenXrSwapchain>()
        .expect("OpenXrSwapchain resource not exists");
    let layer_builder = world
        .remove_resource::<OpenXrCompositionLayerBuilder>()
        .expect("OpenXrCompositionLayerBuilder resource not exists");

    let render_resources = OpenXrRenderResources {
        frame_stream,
        swapchain,
        layer_builder,
    };
    world.insert_resource(render_resources);
}

fn spawn_camera(
    swapchain_images: Res<OpenXrSwapchainImages>,
    mut manual_texture_views: ResMut<ManualTextureViews>,
    mut commands: Commands,
) {
    debug_span!("OpenXrCameraPlugin");

    // Use first texture to initial view creation
    let swapchain_image = swapchain_images.0.first().unwrap();
    trace!("swapchain_image: {:?}", swapchain_image.0);
    let views = &swapchain_image.1;
    trace!("views: {:?}", views);

    // Initialize camera with views of first swapchain
    for (i, view) in views.iter().enumerate() {
        let view_index = view_index(i as u32);
        trace!("view_index: {:?}", view_index);

        let view = ManualTextureView {
            texture_view: view.clone().into(),
            size: UVec2 {
                x: swapchain_image.0.size().width,
                y: swapchain_image.0.size().height,
            },
            format: swapchain_image.0.format(),
        };
        let handle = ManualTextureViewHandle(view_index);
        manual_texture_views.insert(handle, view);

        trace!("view_handle: {:?}", handle);
        commands.spawn((
            Camera {
                target: RenderTarget::TextureView(handle),
                clear_color: ClearColorConfig::Custom(Color::srgb_u8(128, 128, 255)),
                ..Default::default()
            },
            OpenXrCameraIndex(i as u32),
            Projection::custom(OpenXrViewProjection::default()),
        ));
    }
}

fn sync_actions(_world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    trace!("sync_actions")
}
