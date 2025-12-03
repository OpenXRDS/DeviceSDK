use bevy::{
    camera::{ManualTextureViewHandle, RenderTarget},
    prelude::*,
    render::{
        extract_resource::ExtractResourcePlugin, texture::ManualTextureView, view::ExtractedView,
        MainWorld, Render, RenderApp, RenderSystems,
    },
};
use wgpu::TextureViewDescriptor;

use crate::{
    backends::OpenXrGraphicsBackends,
    openxr::{
        camera::{OpenXrCameraIndex, OpenXrViewProjection},
        frame::OpenXrFrameWaiter,
        layers::builder::OpenXrCompositionLayerBuilder,
        resources::{
            OpenXrEnvironmentBlendModes, OpenXrFrameState, OpenXrFrameStream,
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
                (
                    openxr_acquire_swapchain_image,
                    openxr_update_render_views,
                    openxr_wait_swapchain_image,
                )
                    .chain()
                    .in_set(OpenXrRenderSystems::PreRender)
                    // .run_if(openxr_in_state_visible)
                    .run_if(resource_equals(OpenXrSessionState::Running)),
            )
            .add_systems(
                Render,
                (
                    openxr_release_swapchain_image, //.run_if(openxr_in_state_visible),
                    openxr_end_frame,
                )
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
        info!("OpenXR render resources extracted");
    }
}

fn openxr_wait_frame(world: &mut World) {
    debug_span!("OpenXrSessionPlugin");

    let mut frame_waiter = world.resource_mut::<OpenXrFrameWaiter>();

    let frame_state = frame_waiter.wait().expect("Could not wait frame");
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

    let (flags, views) = session
        .locate_views(
            view_configurations.view_configuration_type,
            frame_state.0.predicted_display_time,
            &primary_reference_space.0,
        )
        .expect("Could not locate views");

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
    views: Res<OpenXrViews>,
    graphics_backends: Res<OpenXrGraphicsBackends>,
) {
    debug_span!("OpenXrRenderPlugin");
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

        *transform = get_transform(view);
        trace!("update_camera transform={:?}", *transform);
    }
    trace!("update_camera")
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
    mut query: Query<&mut Transform, With<OpenXrCamera>>,
    views: Res<OpenXrViews>,
) {
    debug_span!("OpenXrRenderPlugin");
    for mut transform in query.iter_mut() {
        // TODO: Check condition (left or right)
        *transform = get_transform(&views.0[0]);
        trace!("update_user_camera");
    }
}

fn openxr_begin_frame(
    mut frame_stream: ResMut<OpenXrFrameStream>,
    frame_state: Res<OpenXrFrameState>,
) {
    debug_span!("OpenXrRenderPlugin");

    frame_stream.begin().expect("Could not begin OpenXR frame");
    trace!(
        "begin_frame. display_time={:?}, period={:?}, render={:?}",
        frame_state.0.predicted_display_time,
        frame_state.0.predicted_display_period,
        frame_state.0.should_render
    )
}

fn openxr_update_render_views(
    views: Res<OpenXrViews>,
    mut query: Query<(&mut ExtractedView, &OpenXrCameraIndex)>,
) {
    for (mut extracted_view, camera_index) in query.iter_mut() {
        let view = &views.0[camera_index.0 as usize];
        extracted_view.world_from_view =
            GlobalTransform::default().mul_transform(get_transform(view));
        // TODO: Make global transform locatable
        trace!(
            "update_views: world_from_view={:?}, viewport={:?}",
            extracted_view.world_from_view,
            extracted_view.viewport
        );
    }
}

fn openxr_acquire_swapchain_image(
    mut swapchain: ResMut<OpenXrSwapchain>,
    swapchain_images: Res<OpenXrSwapchainImages>,
    swapchain_info: Res<OpenXrSwapchainInfo>,
    mut manual_texture_views: ResMut<ManualTextureViews>,
) {
    debug_span!("OpenXrRenderPlugin");

    let idx = swapchain
        .acquire_image()
        .expect("Could not acquire swapchain image");

    let swapchain_image = &swapchain_images.0[idx as usize];
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

    trace!("acquire_swapchain_image. index={}", idx);
}

fn openxr_wait_swapchain_image(mut swapchain: ResMut<OpenXrSwapchain>) {
    debug_span!("OpenXrRenderPlugin");

    swapchain
        .wait_image(openxr::Duration::INFINITE)
        .expect("Could not wait swapchain image");

    trace!("wait_swapchain_image");
}

fn openxr_release_swapchain_image(mut swapchain: ResMut<OpenXrSwapchain>) {
    debug_span!("OpenXrRenderPlugin");

    swapchain
        .release_image()
        .expect("Could not release swapchain image");

    trace!("release_swapchain_image");
}

fn openxr_end_frame(world: &mut World) {
    debug_span!("OpenXrRenderPlugin");

    world.resource_scope::<OpenXrFrameStream, ()>(|world, mut frame_stream| {
        let frame_state = world.resource::<OpenXrFrameState>();
        let blend_modes = world.resource::<OpenXrEnvironmentBlendModes>();
        let builder = world.resource::<OpenXrCompositionLayerBuilder>();
        let layers = if frame_state.0.should_render {
            builder.build(world)
        } else {
            vec![]
        };
        let layers_ref: Vec<_> = layers.iter().map(Box::as_ref).collect();
        frame_stream
            .end(
                frame_state.0.predicted_display_time,
                blend_modes.current_blend_mode,
                &layers_ref,
            )
            .expect("Could not end frame");
        trace!(
            "end_frame. display_time={:?}, period={:?}, render={:?}",
            frame_state.0.predicted_display_time,
            frame_state.0.predicted_display_period,
            frame_state.0.should_render
        );
    })
}
