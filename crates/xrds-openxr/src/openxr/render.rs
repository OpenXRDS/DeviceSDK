use bevy::{
    camera::{ManualTextureViewHandle, RenderTarget},
    prelude::*,
    render::{
        extract_resource::ExtractResourcePlugin, texture::ManualTextureView, MainWorld, Render,
        RenderApp, RenderSystems,
    },
};
use wgpu::TextureViewDescriptor;

use crate::{
    backends::OpenXrGraphicsBackends,
    openxr::{
        camera::{OpenXrCameraIndex, OpenXrPlayerRoot, OpenXrViewProjection},
        frame::OpenXrFrameWaiter,
        layers::builder::OpenXrCompositionLayerBuilder,
        resources::{
            OpenXrEnvironmentBlendModes, OpenXrFrameState, OpenXrFrameStream,
            OpenXrPassthroughEnabled, OpenXrPassthroughLayerHandle,
            OpenXrPrimaryReferenceSpace, OpenXrRenderResources, OpenXrSwapchain,
            OpenXrSwapchainImages, OpenXrSwapchainInfo, OpenXrViewConfigurations, OpenXrViews,
        },
        schedule::{
            openxr_in_state_synchronized, OpenXrDeviceState, OpenXrRenderSystems,
            OpenXrRuntimeSystems, OpenXrSchedules, OpenXrSessionState,
        },
        session::OpenXrSession,
        swapchain::view_index,
    },
    OpenXrCamera,
};

pub struct OpenXrRenderPlugin;

impl Plugin for OpenXrRenderPlugin {
    fn build(&self, app: &mut App) {
        // Define resources to extracted to render app
        app.add_plugins((
            ExtractResourcePlugin::<OpenXrFrameState>::default(),
            ExtractResourcePlugin::<OpenXrSessionState>::default(),
            ExtractResourcePlugin::<OpenXrDeviceState>::default(),
            ExtractResourcePlugin::<OpenXrViews>::default(),
            ExtractResourcePlugin::<OpenXrSession>::default(),
            ExtractResourcePlugin::<OpenXrSwapchainImages>::default(),
            ExtractResourcePlugin::<OpenXrViewConfigurations>::default(),
            ExtractResourcePlugin::<OpenXrEnvironmentBlendModes>::default(),
            ExtractResourcePlugin::<OpenXrPrimaryReferenceSpace>::default(),
            ExtractResourcePlugin::<OpenXrSwapchainInfo>::default(),
            // Both are read by layer builders, which run in the render world.
            ExtractResourcePlugin::<OpenXrPassthroughEnabled>::default(),
            ExtractResourcePlugin::<OpenXrPassthroughLayerHandle>::default(),
        ))
        .add_systems(
            OpenXrSchedules::Update,
            openxr_wait_frame
                .in_set(OpenXrRuntimeSystems::WaitFrame)
                .run_if(resource_equals(OpenXrSessionState::Running)),
        )
        .add_systems(
            OpenXrSchedules::Update,
            openxr_update_camera
                .after(OpenXrRuntimeSystems::WaitFrame)
                .in_set(OpenXrRuntimeSystems::FrameLoop)
                .run_if(openxr_in_state_synchronized),
        )
        .add_systems(
            PostUpdate,
            (
                openxr_locate_views,
                openxr_update_view_projection,
                #[cfg(feature = "preview_window")]
                openxr_update_preview_camera,
            )
                .chain()
                .before(TransformSystems::Propagate)
                .run_if(openxr_in_state_synchronized),
        );

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            // Add temporal resource for prevent error before extract resource
            .init_resource::<OpenXrSessionState>()
            .init_resource::<OpenXrDeviceState>()
            .add_systems(ExtractSchedule, extract_render_resources)
            .add_systems(
                Render,
                openxr_begin_frame
                    .in_set(OpenXrRenderSystems::BeginFrame)
                    .run_if(resource_equals(OpenXrSessionState::Running)),
            )
            .add_systems(
                Render,
                (openxr_acquire_swapchain_image, openxr_wait_swapchain_image)
                    .chain()
                    .in_set(OpenXrRenderSystems::PreRender)
                    .run_if(resource_equals(OpenXrSessionState::Running)),
            )
            .add_systems(
                Render,
                (openxr_release_swapchain_image, openxr_end_frame)
                    .chain()
                    .in_set(OpenXrRenderSystems::PostRender)
                    .run_if(resource_equals(OpenXrSessionState::Running)),
            )
            .configure_sets(
                Render,
                (
                    OpenXrRenderSystems::BeginFrame,
                    OpenXrRenderSystems::PreRender,
                    OpenXrRenderSystems::PostRender,
                )
                    .chain(),
            )
            .configure_sets(
                Render,
                OpenXrRenderSystems::BeginFrame.after(RenderSystems::ExtractCommands),
            )
            .configure_sets(
                Render,
                OpenXrRenderSystems::PreRender
                    .before(RenderSystems::ManageViews)
                    .before(RenderSystems::PrepareAssets),
            )
            .configure_sets(
                Render,
                OpenXrRenderSystems::PostRender
                    .after(RenderSystems::Render)
                    .before(RenderSystems::Cleanup),
            );
    }
}

fn extract_render_resources(mut commands: Commands, mut world: ResMut<MainWorld>) {
    debug_span!("OpenXrRenderPlugin");
    if let Some(OpenXrRenderResources {
        frame_stream,
        swapchain,
        layer_builder,
    }) = world.remove_resource::<OpenXrRenderResources>()
    {
        commands.insert_resource(frame_stream);
        commands.insert_resource(swapchain);
        commands.insert_resource(layer_builder);
        log::info!("XR: render resources extracted to render world");
    }
}

fn openxr_wait_frame(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");

    use std::sync::atomic::{AtomicU64, Ordering};
    static WF_CALL: AtomicU64 = AtomicU64::new(0);
    let wf_n = WF_CALL.fetch_add(1, Ordering::Relaxed);
    let verbose = wf_n < 10 || wf_n % 90 == 0;
    // if verbose { log::info!("XR: xrWaitFrame start call={}", wf_n); }

    let mut frame_waiter = world.resource_mut::<OpenXrFrameWaiter>();

    let frame_state = match frame_waiter.wait() {
        Ok(s) => s,
        Err(e) => {
            log::error!("XR: xrWaitFrame failed call={}: {e:?}", wf_n);
            return;
        }
    };
    // if verbose {
    //     log::info!(
    //         "XR: xrWaitFrame returned call={} render={}",
    //         wf_n,
    //         frame_state.should_render
    //     );
    // }
    world.insert_resource(OpenXrFrameState(frame_state));

    trace!(
        "wait_frame. display_time={:?}, period={:?}, render={:?}",
        frame_state.predicted_display_time,
        frame_state.predicted_display_period,
        frame_state.should_render
    );
}

fn openxr_locate_views(
    view_configurations: Res<OpenXrViewConfigurations>,
    frame_state: Res<OpenXrFrameState>,
    primary_reference_space: Res<OpenXrPrimaryReferenceSpace>,
    session: Res<OpenXrSession>,
    mut openxr_views: ResMut<OpenXrViews>,
) {
    debug_span!("OpenXrRenderPlugin");

    use std::sync::atomic::{AtomicU64, Ordering};
    static LOCATE_CALL: AtomicU64 = AtomicU64::new(0);
    let locate_n = LOCATE_CALL.fetch_add(1, Ordering::Relaxed);
    let diag = locate_n < 300 || locate_n % 90 == 0;

    let (flags, views) = session
        .locate_views(
            view_configurations.view_configuration_type,
            frame_state.0.predicted_display_time,
            &primary_reference_space.0,
        )
        .expect("Could not locate views");

    if diag {
        log::info!(
            "[XR-DIAG] locate_views#{}: returned {} views, flags=POSITION_VALID:{} ORIENTATION_VALID:{}",
            locate_n,
            views.len(),
            flags.intersects(openxr::ViewStateFlags::POSITION_VALID),
            flags.intersects(openxr::ViewStateFlags::ORIENTATION_VALID),
        );
    }

    for (i, view) in views.iter().enumerate() {
        let out = &mut openxr_views.0[i];

        out.fov = view.fov;
        if flags.intersects(openxr::ViewStateFlags::POSITION_VALID) {
            // Update current position
            out.pose.position = views[i].pose.position;
        }
        if flags.intersects(openxr::ViewStateFlags::ORIENTATION_VALID) {
            // Update current orientation
            out.pose.orientation = views[i].pose.orientation;
        }

        if diag {
            log::info!(
                "[XR-DIAG] locate_views#{} view[{}]: pos=({:.3},{:.3},{:.3}) orient=({:.3},{:.3},{:.3},{:.3}) fov(L/R/U/D)=({:.1},{:.1},{:.1},{:.1})°",
                locate_n, i,
                out.pose.position.x, out.pose.position.y, out.pose.position.z,
                out.pose.orientation.x, out.pose.orientation.y, out.pose.orientation.z, out.pose.orientation.w,
                out.fov.angle_left.to_degrees(), out.fov.angle_right.to_degrees(),
                out.fov.angle_up.to_degrees(), out.fov.angle_down.to_degrees(),
            );
        }

        trace!(
            "locate_views: fov={:?}, pose={:?}, orientation={:?}",
            out.fov,
            out.pose,
            out.pose.orientation
        )
    }
}

#[allow(unused)]
fn openxr_locate_space(_world: &mut World) {
    debug_span!("OpenXrRenderPlugin");

    // let session = world.resource::<OpenXrSession>();
    // session.locate_space(left_controller_space, primary_space, time);

    trace!("locate_space")
}

fn openxr_update_camera(
    mut cameras: Query<(&mut Camera, &OpenXrCameraIndex)>,
    frame_state: Res<OpenXrFrameState>,
) {
    for (mut camera, camera_index) in cameras.iter_mut() {
        let view_index = view_index(camera_index.0);
        camera.target = RenderTarget::TextureView(ManualTextureViewHandle(view_index));
        trace!("New camera target: {:?}", camera.target);
        if frame_state.is_changed() {
            camera.is_active = frame_state.0.should_render;
        }
    }
}

fn openxr_update_view_projection(
    mut query: Query<(&mut Transform, &mut Projection, &OpenXrCameraIndex)>,
    root_q: Query<&Transform, (With<OpenXrPlayerRoot>, Without<OpenXrCameraIndex>)>,
    views: Res<OpenXrViews>,
    graphics_backends: Res<OpenXrGraphicsBackends>,
) {
    debug_span!("OpenXrRenderPlugin");
    // Read Transform directly — avoids the one-frame GlobalTransform propagation lag
    // since this system runs in PostUpdate before TransformSystems::Propagate.
    let root = root_q.single().ok().copied();

    use std::sync::atomic::{AtomicU64, Ordering};
    static PROJ_CALL: AtomicU64 = AtomicU64::new(0);
    let proj_n = PROJ_CALL.fetch_add(1, Ordering::Relaxed);
    let diag = proj_n < 300 || proj_n % 90 == 0;

    if diag {
        let cam_count = query.iter().count();
        log::info!(
            "[XR-DIAG] update_view_proj#{}: cam_count={}, root_pos={:?}",
            proj_n,
            cam_count,
            root.map(|r| (r.translation.x, r.translation.y, r.translation.z)),
        );
    }

    for (mut transform, mut projection, camera_index) in query.iter_mut() {
        let view = &views.0[camera_index.0 as usize];
        trace!("view: pose={:?}, fov={:?}", view.pose, view.fov);
        if let Projection::Custom(custom) = projection.as_mut() {
            let view_projection = custom
                .get_mut::<OpenXrViewProjection>()
                .expect("Could not get mutable openxr projection");

            let projection_matrix =
                graphics_backends.calculate_projection_matrix(view_projection.near, view.fov);
            view_projection.projection_matrix = projection_matrix;
            trace!(
                "projection_matrix for camera #{}={:?}",
                camera_index.0,
                projection_matrix
            );
        } else {
            panic!("Unexpected projection type for OpenXR camera. Must be Projection::Custom");
        }

        let head_pose = get_transform(view);
        *transform = apply_root_to_pose(root.as_ref(), &head_pose);

        if diag {
            let hp = head_pose.translation;
            let tp = transform.translation;
            log::info!(
                "[XR-DIAG] update_view_proj#{} cam[{}]: stage_pos=({:.3},{:.3},{:.3}) bevy_pos=({:.3},{:.3},{:.3})",
                proj_n, camera_index.0,
                hp.x, hp.y, hp.z,
                tp.x, tp.y, tp.z,
            );
        }

        trace!("update_camera transform={:?}", *transform);
    }
    trace!("update_camera")
}

/// Apply an optional player root to a raw STAGE-space head pose.
/// Only the root's XZ position and yaw are applied — Y comes entirely from
/// physical head tracking so standing height is never double-counted.
fn apply_root_to_pose(root: Option<&Transform>, head_pose: &Transform) -> Transform {
    match root {
        Some(r) => {
            let yaw = r.rotation.to_euler(EulerRot::YXZ).0;
            let yaw_rot = Quat::from_rotation_y(yaw);
            let origin = Vec3::new(r.translation.x, 0.0, r.translation.z);
            Transform::from_translation(origin + yaw_rot * head_pose.translation)
                .with_rotation(yaw_rot * head_pose.rotation)
        }
        None => *head_pose,
    }
}

fn get_transform(view: &openxr::View) -> Transform {
    Transform::from_translation(Vec3::new(
        view.pose.position.x,
        view.pose.position.y,
        view.pose.position.z,
    ))
    .with_rotation(quat(
        view.pose.orientation.x,
        view.pose.orientation.y,
        view.pose.orientation.z,
        view.pose.orientation.w,
    ))
}

fn openxr_update_preview_camera(
    mut query: Query<&mut Transform, (With<OpenXrCamera>, Without<OpenXrCameraIndex>)>,
    root_q: Query<
        &Transform,
        (
            With<OpenXrPlayerRoot>,
            Without<OpenXrCameraIndex>,
            Without<OpenXrCamera>,
        ),
    >,
    views: Res<OpenXrViews>,
) {
    debug_span!("OpenXrRenderPlugin");
    let root = root_q.single().ok().copied();
    let head_pose = get_transform(&views.0[0]);
    for mut transform in query.iter_mut() {
        *transform = apply_root_to_pose(root.as_ref(), &head_pose);
        trace!("update_user_camera");
    }
}

fn openxr_begin_frame(
    mut frame_stream: ResMut<OpenXrFrameStream>,
    frame_state: Res<OpenXrFrameState>,
) {
    debug_span!("OpenXrRenderPlugin");

    use std::sync::atomic::{AtomicU64, Ordering};
    static BF_CALL: AtomicU64 = AtomicU64::new(0);
    let bf_n = BF_CALL.fetch_add(1, Ordering::Relaxed);
    let verbose = bf_n < 10 || bf_n % 90 == 0;
    if verbose {
        log::info!(
            "XR: xrBeginFrame start call={} render={}",
            bf_n,
            frame_state.0.should_render
        );
    }
    if let Err(e) = frame_stream.begin() {
        log::error!("XR: xrBeginFrame failed call={}: {e:?}", bf_n);
        return;
    }
    if verbose {
        log::info!("XR: xrBeginFrame done call={}", bf_n);
    }
    trace!(
        "begin_frame. display_time={:?}, period={:?}, render={:?}",
        frame_state.0.predicted_display_time,
        frame_state.0.predicted_display_period,
        frame_state.0.should_render
    )
}


fn openxr_acquire_swapchain_image(
    mut swapchain: ResMut<OpenXrSwapchain>,
    swapchain_images: Res<OpenXrSwapchainImages>,
    swapchain_info: Res<OpenXrSwapchainInfo>,
    mut manual_texture_views: ResMut<ManualTextureViews>,
) {
    debug_span!("OpenXrRenderPlugin");

    use std::sync::atomic::{AtomicU64, Ordering};
    static ACQ_CALL: AtomicU64 = AtomicU64::new(0);
    let acq_n = ACQ_CALL.fetch_add(1, Ordering::Relaxed);
    let verbose = acq_n < 10 || acq_n % 90 == 0;

    let idx = match swapchain.acquire_image() {
        Ok(i) => i,
        Err(e) => {
            log::error!("XR: acquire_image failed: {e:?}");
            return;
        }
    };

    let swapchain_image = &swapchain_images.0[idx as usize];
    let eye_count = swapchain_image.1.len();
    for (i, _) in swapchain_image.1.iter().enumerate() {
        let texture_view = swapchain_image.0.create_view(&TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            array_layer_count: Some(1),
            base_array_layer: i as _,
            ..Default::default()
        });
        let view = ManualTextureView {
            texture_view: texture_view.into(),
            size: UVec2 {
                x: swapchain_info.size.width,
                y: swapchain_info.size.height,
            },
            format: swapchain_info.format,
        };
        let handle = ManualTextureViewHandle(view_index(i as u32));
        trace!(
            "New handle for current swapchain index={:?}, handle={:?}, format={:?}",
            idx,
            handle,
            swapchain_info.format
        );
        manual_texture_views.insert(handle, view);
    }

    if verbose {
        log::info!(
            "[XR-DIAG] acquire#{}: swapchain_idx={} eye_count={} size={}x{} fmt={:?}",
            acq_n,
            idx,
            eye_count,
            swapchain_info.size.width,
            swapchain_info.size.height,
            swapchain_info.format,
        );
    }
    trace!("acquire_swapchain_image. index={}", idx);
}

fn openxr_wait_swapchain_image(mut swapchain: ResMut<OpenXrSwapchain>) {
    debug_span!("OpenXrRenderPlugin");

    use std::sync::atomic::{AtomicU64, Ordering};
    static WI_CALL: AtomicU64 = AtomicU64::new(0);
    let wi_n = WI_CALL.fetch_add(1, Ordering::Relaxed);
    let verbose = wi_n < 10 || wi_n % 90 == 0;
    if verbose {
        log::info!("XR: xrWaitSwapchainImage start call={}", wi_n);
    }
    if let Err(e) = swapchain.wait_image(openxr::Duration::INFINITE) {
        log::error!("XR: xrWaitSwapchainImage failed call={}: {e:?}", wi_n);
    }
    if verbose {
        log::info!("XR: xrWaitSwapchainImage done call={}", wi_n);
    }

    trace!("wait_swapchain_image");
}

fn openxr_release_swapchain_image(mut swapchain: ResMut<OpenXrSwapchain>) {
    debug_span!("OpenXrRenderPlugin");

    if let Err(e) = swapchain.release_image() {
        log::error!("XR: release_image failed: {e:?}");
    }

    trace!("release_swapchain_image");
}

fn openxr_end_frame(world: &mut World) {
    debug_span!("OpenXrRenderPlugin");

    use std::sync::atomic::{AtomicU64, Ordering};
    static EF_CALL: AtomicU64 = AtomicU64::new(0);
    let ef_n = EF_CALL.fetch_add(1, Ordering::Relaxed);
    let verbose = ef_n < 10 || ef_n % 90 == 0;
    if verbose {
        log::info!("XR: xrEndFrame start call={}", ef_n);
    }
    world.resource_scope::<OpenXrFrameStream, ()>(|world, mut frame_stream| {
        let frame_state = world.resource::<OpenXrFrameState>();
        let blend_modes = world.resource::<OpenXrEnvironmentBlendModes>();
        let builder = world.resource::<OpenXrCompositionLayerBuilder>();
        let is_synchronized = matches!(
            world.resource::<OpenXrDeviceState>(),
            OpenXrDeviceState::Synchronized
                | OpenXrDeviceState::Visible
                | OpenXrDeviceState::Focused
        );
        let layers = if frame_state.0.should_render && is_synchronized {
            if verbose {
                log::info!("XR: xrEndFrame building layers call={}", ef_n);
            }
            builder.build(world)
        } else {
            if verbose {
                log::info!(
                    "XR: xrEndFrame empty layers call={} (should_render={} synchronized={})",
                    ef_n,
                    frame_state.0.should_render,
                    is_synchronized
                );
            }
            vec![]
        };
        let layers_ref: Vec<_> = layers.iter().map(Box::as_ref).collect();
        if let Err(e) = frame_stream.end(
            frame_state.0.predicted_display_time,
            blend_modes.current_blend_mode,
            &layers_ref,
        ) {
            log::error!("XR: xrEndFrame failed call={}: {e:?}", ef_n);
        }
        if verbose {
            log::info!("XR: xrEndFrame done call={}", ef_n);
        }
        trace!(
            "end_frame. display_time={:?}, period={:?}, render={:?}",
            frame_state.0.predicted_display_time,
            frame_state.0.predicted_display_period,
            frame_state.0.should_render
        );
    })
}
