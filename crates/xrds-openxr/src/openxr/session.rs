use std::ptr::{self, null, null_mut};

use bevy::{
    camera::{visibility::NoCpuCulling, ManualTextureViewHandle, RenderTarget},
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_resource::TextureUsages,
        texture::ManualTextureView,
        view::NoIndirectDrawing,
    },
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
            OpenXrEnvironmentBlendModes, OpenXrFrameStream, OpenXrInstance, OpenXrPassthrough,
            OpenXrPassthroughEnabled, OpenXrPassthroughLayerHandle, OpenXrRenderResources,
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
    pub fn attach_action_sets(&self, sets: &[&openxr::ActionSet]) -> openxr::Result<()> {
        openxr_graphics!(&self.0; inner => inner.attach_action_sets(sets))
    }

    #[inline]
    pub fn sync_actions<'a>(&self, action_sets: &[openxr::ActiveActionSet<'a>]) -> openxr::Result<()> {
        openxr_graphics!(&self.0; inner => inner.sync_actions(action_sets))
    }

    #[inline]
    pub fn action_state_f32(
        &self,
        action: &openxr::Action<f32>,
        path:   openxr::Path,
    ) -> openxr::Result<openxr::ActionState<f32>> {
        openxr_graphics!(&self.0; inner => action.state(inner, path))
    }

    #[inline]
    pub fn action_state_bool(
        &self,
        action: &openxr::Action<bool>,
        path:   openxr::Path,
    ) -> openxr::Result<openxr::ActionState<bool>> {
        openxr_graphics!(&self.0; inner => action.state(inner, path))
    }

    #[inline]
    pub fn action_state_vec2f(
        &self,
        action: &openxr::Action<openxr::Vector2f>,
        path:   openxr::Path,
    ) -> openxr::Result<openxr::ActionState<openxr::Vector2f>> {
        openxr_graphics!(&self.0; inner => action.state(inner, path))
    }

    /// Create a hand tracker for one hand. Requires `XR_EXT_hand_tracking`.
    #[inline]
    pub fn create_hand_tracker(&self, hand: openxr::Hand) -> openxr::Result<openxr::HandTracker> {
        openxr_graphics!(&self.0; inner => inner.create_hand_tracker(hand))
    }

    /// Create a reference space and return it as an owned `openxr::Space`.
    /// Used for hand-joint location, which requires the same `SessionInner` as the hand tracker.
    #[inline]
    pub fn create_owned_reference_space(
        &self,
        ty:   openxr::ReferenceSpaceType,
        pose: openxr::Posef,
    ) -> openxr::Result<openxr::Space> {
        openxr_graphics!(&self.0; inner => inner.create_reference_space(ty, pose))
    }

    // --- XR_FB_render_model ---

    pub fn enumerate_render_model_paths_fb(&self) -> openxr::Result<Vec<openxr::Path>> {
        openxr_graphics!(&self.0; inner => inner.enumerate_render_model_paths_fb())
    }

    pub fn get_render_model_properties_fb(
        &self,
        path:  openxr::Path,
        flags: openxr::RenderModelFlagsFB,
    ) -> openxr::Result<openxr::RenderModelPropertiesFB> {
        openxr_graphics!(&self.0; inner => inner.get_render_model_properties_fb(path, flags))
    }

    pub fn load_render_model_fb(
        &self,
        model_key: openxr::sys::RenderModelKeyFB,
    ) -> openxr::Result<Vec<u8>> {
        openxr_graphics!(&self.0; inner => inner.load_render_model_fb(model_key))
    }

    #[inline]
    pub fn create_action_space(
        &self,
        action:         &openxr::Action<openxr::Posef>,
        subaction_path: openxr::Path,
    ) -> openxr::Result<openxr::Space> {
        openxr_graphics!(&self.0; inner => {
            // create_space takes Session<G> by value in openxr 0.19 (Arc clone — cheap)
            action.create_space(
                inner.clone(),
                subaction_path,
                openxr::Posef {
                    orientation: openxr::Quaternionf { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
                    position:    openxr::Vector3f    { x: 0.0, y: 0.0, z: 0.0 },
                },
            )
        })
    }

    pub fn apply_haptic_feedback(
        &self,
        action:    &openxr::Action<openxr::Haptic>,
        path:      openxr::Path,
        amplitude: f32,
        duration:  openxr::Duration,
        frequency: f32,
    ) -> openxr::Result<()> {
        openxr_graphics!(&self.0; inner => {
            unsafe {
                let action_info = openxr::sys::HapticActionInfo {
                    ty:              openxr::sys::HapticActionInfo::TYPE,
                    next:            null(),
                    action:          action.as_raw(),
                    subaction_path:  path,
                };
                let vibration = openxr::sys::HapticVibration {
                    ty:        openxr::sys::HapticVibration::TYPE,
                    next:      null(),
                    duration,
                    frequency,
                    amplitude,
                };
                cvt((inner.instance().fp().apply_haptic_feedback)(
                    inner.as_raw(),
                    &action_info,
                    &vibration as *const openxr::sys::HapticVibration
                        as *const openxr::sys::HapticBaseHeader,
                ))?;
                Ok(())
            }
        })
    }

    #[allow(dead_code)]
    pub fn stop_haptic_feedback(
        &self,
        action: &openxr::Action<openxr::Haptic>,
        path:   openxr::Path,
    ) -> openxr::Result<()> {
        openxr_graphics!(&self.0; inner => {
            unsafe {
                let action_info = openxr::sys::HapticActionInfo {
                    ty:             openxr::sys::HapticActionInfo::TYPE,
                    next:           null(),
                    action:         action.as_raw(),
                    subaction_path: path,
                };
                cvt((inner.instance().fp().stop_haptic_feedback)(
                    inner.as_raw(),
                    &action_info,
                ))?;
                Ok(())
            }
        })
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
                unsafe {cvt(
                    (inner.instance().fp().create_reference_space)(
                        inner.as_raw(), &ReferenceSpaceCreateInfo {
                            ty: StructureType::REFERENCE_SPACE_CREATE_INFO,
                            next: null(),
                            reference_space_type,
                            pose_in_reference_space
                        },
                        &mut space
                    )
                )?;}
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
    log::info!("XR: initialize_view_and_blend_mode start");
    let openxr_instance = world.resource::<OpenXrInstance>();

    let view_configurations = match openxr_instance.enumerate_view_configurations() {
        Ok(v) => v,
        Err(e) => {
            log::error!("XR: enumerate_view_configurations failed: {e:?}");
            return;
        }
    };
    let view_configuration_type = match view_configurations.first() {
        Some(v) => v,
        None => {
            log::error!("XR: no view configuration types");
            return;
        }
    };
    let view_configuration_views = match openxr_instance
        .enumerate_view_configuration_views(view_configuration_type)
    {
        Ok(v) => v,
        Err(e) => {
            log::error!("XR: enumerate_view_configuration_views failed: {e:?}");
            return;
        }
    };
    let blend_modes = match openxr_instance
        .enumerate_environment_blend_modes(view_configuration_type)
    {
        Ok(v) => v,
        Err(e) => {
            log::error!("XR: enumerate_environment_blend_modes failed: {e:?}");
            return;
        }
    };
    // `environmentBlendMode` is a mandatory, GLOBAL `xrEndFrame` parameter deciding
    // how the *whole frame* blends with the real world. Taking index 0 is wrong:
    // a runtime that enumerates ALPHA_BLEND or ADDITIVE first makes reality show
    // through everywhere our content's alpha is below 1.0, whatever the app
    // intended — and plenty of content is incidentally non-opaque (unlit panels,
    // particle trails, text atlases).
    //
    // Prefer OPAQUE explicitly. Passthrough is **not** meant to be reached through
    // this enum: on Quest it is an `XR_FB_passthrough` composition layer submitted
    // beneath an alpha-blended projection layer, with the environment mode left
    // OPAQUE. Verified against a shipped Quest 3 passthrough app; the recipe is in
    // `docs/small-phases-plan.md` S4.
    let blend_mode = match blend_modes
        .iter()
        .find(|&&m| m == openxr::EnvironmentBlendMode::OPAQUE)
        .or_else(|| blend_modes.first())
    {
        Some(v) => v,
        None => {
            log::error!("XR: no environment blend modes");
            return;
        }
    };

    log::info!(
        "XR: view_config={:?} views={} blend={:?}",
        view_configuration_type,
        view_configuration_views.len(),
        blend_mode
    );

    let openxr_views = OpenXrViews(
        (0..view_configuration_views.len())
            .map(|_| openxr::View {
                pose: openxr::Posef::IDENTITY,
                fov: openxr::Fovf {
                    angle_left:  -std::f32::consts::FRAC_PI_4,
                    angle_right:  std::f32::consts::FRAC_PI_4,
                    angle_up:     std::f32::consts::FRAC_PI_4,
                    angle_down:  -std::f32::consts::FRAC_PI_4,
                },
            })
            .collect()
    );

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
    // Inserted at 0 *after* the projection, which pushes projection to 1 and leaves
    // the order [passthrough, projection] — index 0 composites furthest back, and
    // passthrough must sit beneath the scene for the scene's alpha to reveal it.
    //
    // Registered here, before the session exists, because this is where the layer
    // list is assembled; the builder yields nothing until the handle resource
    // appears (created in `initialize_openxr_session`) and stays silent whenever
    // `OpenXrPassthroughEnabled` is false.
    if openxr_instance.passthrough_supported {
        openxr_layer_builder.insert_layer(
            0,
            Box::new(crate::openxr::layers::fb::OpenXrCompositionLayerPassthroughFBBuilder),
        );
    }

    // TODO: Create action set here

    log::info!("OpenXR system initialized");
    world.insert_resource(openxr_views);
    world.insert_resource(openxr_view_configurations);
    world.insert_resource(openxr_blend_modes);
    world.insert_resource(openxr_layer_builder);
}

fn initialize_openxr_session(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    log::info!("XR: initialize_openxr_session start");
    let openxr_instance = world.resource::<OpenXrInstance>();
    let graphics_backends = world.resource::<OpenXrGraphicsBackends>();

    let session_create_info = match graphics_backends.get_session_create_info() {
        Ok(info) => {
            log::info!("XR: session_create_info obtained");
            info
        }
        Err(e) => {
            log::error!("XR: get_session_create_info failed: {e:?}");
            return;
        }
    };

    log::info!("XR: calling xrCreateSession...");
    let (session, frame_waiter, frame_stream) =
        match openxr_instance.create_session(&session_create_info) {
            Ok(v) => v,
            Err(e) => {
                log::error!("XR: xrCreateSession failed: {e:?}");
                return;
            }
        };
    log::info!("OpenXR session created");

    create_passthrough(world, &session);

    world.insert_resource(session);
    world.insert_resource(frame_waiter);
    world.insert_resource(frame_stream);
}

/// Creates the passthrough feature and layer, once, for the session's lifetime.
///
/// Created **already running** via `IS_RUNNING_AT_CREATION` rather than by calling
/// `start()`/`resume()` afterwards: in openxr 0.19 those wrappers invoke the wrong
/// entry point, so a layer started that way never produces imagery. Learned from a
/// shipped Quest 3 passthrough app — see `docs/small-phases-plan.md` S4.
///
/// Whether the passthrough is *visible* is a separate question answered per frame by
/// `OpenXrPassthroughEnabled`; creating it here costs a handle and nothing more
/// until the layer is actually submitted.
fn create_passthrough(world: &mut World, session: &OpenXrSession) {
    world.init_resource::<OpenXrPassthroughEnabled>();

    let supported = world
        .get_resource::<OpenXrInstance>()
        .is_some_and(|i| i.passthrough_supported);
    if !supported {
        log::info!("XR: passthrough unsupported by this runtime; scenes requesting it stay opaque");
        return;
    }

    let created = openxr_graphics!(
        &session.0;
        inner => {
            inner
                .create_passthrough(openxr::PassthroughFlagsFB::IS_RUNNING_AT_CREATION)
                .and_then(|feature| {
                    let layer = inner.create_passthrough_layer(
                        &feature,
                        openxr::PassthroughFlagsFB::IS_RUNNING_AT_CREATION,
                        openxr::PassthroughLayerPurposeFB::RECONSTRUCTION,
                    )?;
                    Ok((feature, layer))
                })
        }
    );

    match created {
        Ok((feature, layer)) => {
            log::info!("XR: passthrough layer created");
            world.insert_resource(OpenXrPassthroughLayerHandle(*layer.inner()));
            world.insert_resource(OpenXrPassthrough { feature, layer });
        }
        Err(e) => {
            // Not fatal: the scene renders opaque, which is the same outcome as a
            // device without the extension.
            log::error!("XR: could not create passthrough ({e:?}); scenes will stay opaque");
        }
    }
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

    log::info!(
        "XR: begin_openxr_session view_type={:?}",
        view_configurations.view_configuration_type
    );

    match openxr_session.begin(view_configurations.view_configuration_type) {
        Ok(_) => log::info!("XR: xrBeginSession succeeded"),
        Err(e) => {
            log::error!("XR: xrBeginSession failed: {e:?}");
            return;
        }
    }

    world.insert_resource(OpenXrSessionState::Running);
    log::info!("XR: session state -> Running");
}

fn end_openxr_session(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");

    let openxr_session = world.resource::<OpenXrSession>();
    openxr_session.end().expect("Could not end OpenXR session");
    world.insert_resource(OpenXrSessionState::Idle);
}

fn handle_events(world: &mut World) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static FRAME: AtomicU64 = AtomicU64::new(0);
    let n = FRAME.fetch_add(1, Ordering::Relaxed);
    if n < 20 || n % 90 == 0 {
        log::info!("XR: handle_events tick frame={}", n);
    }

    let openxr_instance = world.resource::<OpenXrInstance>();

    let mut storage = openxr::EventDataBuffer::new();
    let event = match openxr_instance.poll_event(&mut storage) {
        Ok(e) => e,
        Err(e) => {
            log::error!("XR: poll_event failed: {e:?}");
            return;
        }
    };
    trace!("handle_events");
    if let Some(event) = event {
        match event {
            openxr::Event::SessionStateChanged(state) => {
                log::info!(
                    "XR: session state changed: {:?}",
                    state.state()
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
                log::info!(
                    "XR: reference space change pending: type={:?}",
                    reference_space_change_pending.reference_space_type()
                );
            }
            openxr::Event::EventsLost(event_lost) => {
                log::warn!("XR: events lost: {}", event_lost.lost_event_count());
            }
            openxr::Event::InstanceLossPending(instance_loss_pending) => {
                log::warn!(
                    "XR: instance loss pending: {:?}",
                    instance_loss_pending.loss_time()
                );
            }
            openxr::Event::InteractionProfileChanged(_) => {
                log::info!("XR: interaction profile changed");
            }
            _ => {
                log::warn!("XR: unimplemented event");
            }
        }
    }
}

fn init_render_resources(world: &mut World) {
    log::info!("XR: init_render_resources start");
    let frame_stream = match world.remove_resource::<OpenXrFrameStream>() {
        Some(r) => r,
        None => {
            log::error!("XR: init_render_resources: OpenXrFrameStream missing — session create failed");
            return;
        }
    };
    let swapchain = match world.remove_resource::<OpenXrSwapchain>() {
        Some(r) => r,
        None => {
            log::error!("XR: init_render_resources: OpenXrSwapchain missing — swapchain create failed (or ordering issue)");
            return;
        }
    };
    let layer_builder = match world.remove_resource::<OpenXrCompositionLayerBuilder>() {
        Some(r) => r,
        None => {
            log::error!("XR: init_render_resources: OpenXrCompositionLayerBuilder missing");
            return;
        }
    };

    let render_resources = OpenXrRenderResources {
        frame_stream,
        swapchain,
        layer_builder,
    };
    world.insert_resource(render_resources);
    log::info!("XR: init_render_resources done");
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
        // Each eye must get its own depth texture from Bevy's TextureCache.
        // Without this, both cameras share the same depth buffer (same size/format/usage
        // → same cache key). Camera 0 (order=0) fills it; Camera 1's geometry then fails
        // the GREATER depth test at identical depths (small IPD at distance) → only clear.
        // Adding COPY_SRC to Camera 1 changes the cache key without affecting rendering.
        let depth_texture_usages = if i == 0 {
            TextureUsages::RENDER_ATTACHMENT
        } else {
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC
        };
        commands.spawn((
            Camera {
                target: RenderTarget::TextureView(handle),
                order: i as isize,
                ..Default::default()
            },
            OpenXrCameraIndex(i as u32),
            Projection::custom(OpenXrViewProjection::default()),
            NoCpuCulling,
            // Bevy 0.17's GPU indirect preprocessing (GpuPreprocessingMode::Culling) mishandles
            // multi-camera XR on Android: work items for the two eye cameras share global offsets
            // and interfere, causing one eye to lose all geometry. NoIndirectDrawing forces
            // PreprocessingOnly (CPU batching) which bypasses the broken indirect dispatch.
            NoIndirectDrawing,
            Camera3d {
                depth_texture_usages: depth_texture_usages.into(),
                ..Default::default()
            },
        ));
    }
}

fn sync_actions(_world: &mut World) {
    debug_span!("OpenXrSessionPlugin");
    trace!("sync_actions")
}
