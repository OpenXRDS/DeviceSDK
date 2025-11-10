use std::collections::HashSet;

#[cfg(feature = "preview_window")]
use bevy::winit::{UpdateMode, WinitSettings};
use bevy::{
    app::MainScheduleOrder,
    ecs::schedule::ExecutorKind,
    prelude::*,
    render::{
        settings::{RenderCreation, WgpuSettings},
        RenderApp, RenderPlugin,
    },
};
use openxr::{ApplicationInfo, Entry, ExtensionSet, FormFactor};

#[cfg(target_os = "windows")]
use crate::windows::try_load_windows_oxr_runtime;
use crate::{
    backends::{GraphicsInner, OpenXrGraphicsBackend, OpenXrGraphicsBackends},
    openxr::{
        resources::OpenXrInstance,
        schedule::{
            OpenXrDeviceState, OpenXrMessageCreateSession, OpenXrRuntimeSystems, OpenXrSchedules,
            OpenXrSessionState, OpenXrSystemState,
        },
    },
};

#[derive(Default)]
pub struct OpenXrInitPlugin {
    pub app_name: String,
    pub wgpu_settings: Option<WgpuSettings>,
}

impl Plugin for OpenXrInitPlugin {
    fn build(&self, app: &mut App) {
        build_schedule(app);
        build_states(app);
        build_system_sets(app);

        // Initialize OpenXR system and graphics backend
        let (openxr_instance, graphics_backends) = self
            .initialize(&self.app_name)
            .expect("Could not initialize OpenXR and WGPU instance");

        let render_resources = graphics_backends
            .get_render_resources()
            .expect("Could not get render resources");

        app.insert_resource(openxr_instance.clone())
            .insert_resource(graphics_backends)
            .add_plugins(RenderPlugin {
                render_creation: RenderCreation::Manual(render_resources),
                ..Default::default()
            });

        #[cfg(feature = "preview_window")]
        app.insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        });

        let render_app = app.sub_app_mut(RenderApp);
        render_app.insert_resource(openxr_instance);

        let world = app.world_mut();
        world.insert_resource(OpenXrSystemState::Available);
        world.write_message(OpenXrMessageCreateSession);
    }

    fn finish(&self, _app: &mut App) {}
}

fn build_schedule(app: &mut App) {
    let mut openxr_first = Schedule::new(OpenXrSchedules::Update);
    openxr_first.set_executor_kind(ExecutorKind::SingleThreaded);

    app.init_schedule(OpenXrSchedules::SessionCreate)
        .add_schedule(openxr_first)
        .init_schedule(OpenXrSchedules::Cleanup);

    app.world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_before(First, OpenXrSchedules::Update);
}

fn build_states(app: &mut App) {
    app.init_resource::<OpenXrSystemState>()
        .init_resource::<OpenXrSessionState>()
        .init_resource::<OpenXrDeviceState>()
        .add_message::<OpenXrMessageCreateSession>();
}

fn build_system_sets(app: &mut App) {
    app.configure_sets(
        OpenXrSchedules::SessionCreate,
        (
            OpenXrRuntimeSystems::PreSessionCreate,
            OpenXrRuntimeSystems::SessionCreate,
            OpenXrRuntimeSystems::PostSessionCreate,
        )
            .chain()
            .run_if(resource_equals(OpenXrSystemState::Available)),
    )
    .configure_sets(
        OpenXrSchedules::Update,
        (
            OpenXrRuntimeSystems::HandleEvents,
            OpenXrRuntimeSystems::UpdateSessionStates,
            OpenXrRuntimeSystems::PreFrameLoop,
            OpenXrRuntimeSystems::WaitFrame,
            OpenXrRuntimeSystems::FrameLoop,
            OpenXrRuntimeSystems::PostFrameLoop,
        )
            .chain()
            .run_if(resource_equals(OpenXrSystemState::SessionCreated)),
    )
    .configure_sets(
        OpenXrSchedules::Update,
        (
            OpenXrRuntimeSystems::PreFrameLoop,
            OpenXrRuntimeSystems::WaitFrame,
            OpenXrRuntimeSystems::FrameLoop,
            OpenXrRuntimeSystems::PostFrameLoop,
        )
            .run_if(resource_equals(OpenXrSessionState::Running)),
    );
}

impl OpenXrInitPlugin {
    fn initialize(
        &self,
        app_name: &str,
    ) -> anyhow::Result<(OpenXrInstance, OpenXrGraphicsBackends)> {
        #[cfg(target_os = "windows")]
        let result = try_load_windows_oxr_runtime();
        #[cfg(not(target_os = "windows"))]
        let result = unsafe { openxr::Entry::load() };

        let (entry, _lib) = result.unwrap();

        let default_settings = self.wgpu_settings.clone().unwrap_or_default();
        let wgpu_backends = default_settings.backends.unwrap_or(wgpu::Backends::VULKAN);

        let mut openxr_extensions = ExtensionSet::default();
        if wgpu_backends.intersects(wgpu::Backends::VULKAN) {
            openxr_extensions.khr_vulkan_enable2 = true;
            openxr_extensions.khr_vulkan_swapchain_format_list = true;
        } else if wgpu_backends.intersects(wgpu::Backends::DX12) {
            #[cfg(target_os = "windows")]
            {
                openxr_extensions.khr_d3d12_enable = true;
            }
        } else if wgpu_backends.intersects(wgpu::Backends::GL) {
            openxr_extensions.khr_opengl_enable = true;
            #[cfg(target_os = "android")]
            {
                openxr_extensions.khr_opengl_es_enable = true;
                openxr_extensions.khr_android_thread_settings = true;
                openxr_extensions.khr_android_surface_swapchain = true;
                openxr_extensions.khr_android_create_instance = true;
                openxr_extensions.khr_loader_init_android = true;
                // oxr_extension.fb_swapchain_update_state_android_surface = true;
                // oxr_extension.oculus_android_session_state_enable = true;
            }
        } else {
            panic!("Unsupported backend");
        };
        openxr_extensions = intersects_extensions(&entry, openxr_extensions)?;

        let application_info = ApplicationInfo {
            application_name: app_name,
            application_version: 1,
            engine_name: "bevy",
            engine_version: 17,
            api_version: openxr::Version::new(1, 1, 49),
        };
        let instance = entry
            .create_instance(&application_info, &openxr_extensions, &[])
            .expect("Could not create OpenXR instance");
        let system_id_res = (
            instance.system(FormFactor::HEAD_MOUNTED_DISPLAY),
            instance.system(FormFactor::HANDHELD_DISPLAY),
        );
        let system_id = match system_id_res {
            (Ok(hmd_system_id), Ok(_)) | (Ok(hmd_system_id), Err(_)) => hmd_system_id,
            (Err(_), Ok(handheld_system_id)) => handheld_system_id,
            (Err(_), Err(_)) => panic!("No xr system found"),
        };

        let system_properties = instance.system_properties(system_id)?;
        let instance_properties = instance.properties()?;
        info!(
            "OpenXR system: {}, runtime: {}-{}",
            system_properties.system_name,
            instance_properties.runtime_name,
            instance_properties.runtime_version
        );

        let graphics_backends = if wgpu_backends.intersects(wgpu::Backends::VULKAN) {
            GraphicsInner::<openxr::Vulkan>::initialize(
                &instance,
                system_id,
                &application_info,
                default_settings,
            )?
        } else if cfg!(target_os = "windows") && wgpu_backends.intersects(wgpu::Backends::DX12) {
            GraphicsInner::<openxr::D3D12>::initialize(
                &instance,
                system_id,
                &application_info,
                default_settings,
            )?
        } else if wgpu_backends.intersects(wgpu::Backends::GL) {
            GraphicsInner::<openxr::OpenGL>::initialize(
                &instance,
                system_id,
                &application_info,
                default_settings,
            )?
        } else {
            panic!("Unsupported backend");
        };

        let openxr_instance = OpenXrInstance {
            instance: instance.clone(),
            system_id,
        };

        Ok((openxr_instance, graphics_backends))
    }
}

fn intersects_extensions(entry: &Entry, extensions: ExtensionSet) -> anyhow::Result<ExtensionSet> {
    let mut extensions = extensions;
    let supported_extensions = entry.enumerate_extensions()?;
    let _span = debug_span!("xrds-openxr::intersects_extensions");

    if extensions.almalence_digital_lens_control
        && !supported_extensions.almalence_digital_lens_control
    {
        log::warn!("almalence_digital_lens_control required. But not supported");
        extensions.almalence_digital_lens_control = false;
    }
    if extensions.bd_controller_interaction && !supported_extensions.bd_controller_interaction {
        log::warn!("bd_controller_interaction required. But not supported");
        extensions.bd_controller_interaction = false;
    }
    if extensions.epic_view_configuration_fov && !supported_extensions.epic_view_configuration_fov {
        log::warn!("epic_view_configuration_fov required. But not supported");
        extensions.epic_view_configuration_fov = false;
    }
    if extensions.ext_performance_settings && !supported_extensions.ext_performance_settings {
        log::warn!("ext_performance_settings required. But not supported");
        extensions.ext_performance_settings = false;
    }
    if extensions.ext_thermal_query && !supported_extensions.ext_thermal_query {
        log::warn!("ext_thermal_query required. But not supported");
        extensions.ext_thermal_query = false;
    }
    if extensions.ext_debug_utils && !supported_extensions.ext_debug_utils {
        log::warn!("ext_debug_utils required. But not supported");
        extensions.ext_debug_utils = false;
    }
    if extensions.ext_eye_gaze_interaction && !supported_extensions.ext_eye_gaze_interaction {
        log::warn!("ext_eye_gaze_interaction required. But not supported");
        extensions.ext_eye_gaze_interaction = false;
    }
    if extensions.ext_view_configuration_depth_range
        && !supported_extensions.ext_view_configuration_depth_range
    {
        log::warn!("ext_view_configuration_depth_range required. But not supported");
        extensions.ext_view_configuration_depth_range = false;
    }
    if extensions.ext_conformance_automation && !supported_extensions.ext_conformance_automation {
        log::warn!("ext_conformance_automation required. But not supported");
        extensions.ext_conformance_automation = false;
    }
    if extensions.ext_hand_tracking && !supported_extensions.ext_hand_tracking {
        log::warn!("ext_hand_tracking required. But not supported");
        extensions.ext_hand_tracking = false;
    }
    #[cfg(windows)]
    if extensions.ext_win32_appcontainer_compatible
        && !supported_extensions.ext_win32_appcontainer_compatible
    {
        log::warn!("ext_win32_appcontainer_compatible required. But not suppored");
        extensions.ext_win32_appcontainer_compatible = false;
    }
    if extensions.ext_dpad_binding && !supported_extensions.ext_dpad_binding {
        log::warn!("ext_dpad_binding required. But not suppored");
        extensions.ext_dpad_binding = false;
    }
    if extensions.ext_hand_joints_motion_range && !supported_extensions.ext_hand_joints_motion_range
    {
        log::warn!("ext_hand_joints_motion_range required. But not suppored");
        extensions.ext_hand_joints_motion_range = false;
    }
    if extensions.ext_samsung_odyssey_controller
        && !supported_extensions.ext_samsung_odyssey_controller
    {
        log::warn!("ext_samsung_odyssey_controller required. But not suppored");
        extensions.ext_samsung_odyssey_controller = false;
    }
    if extensions.ext_hp_mixed_reality_controller
        && !supported_extensions.ext_hp_mixed_reality_controller
    {
        log::warn!("ext_hp_mixed_reality_controller required. But not suppored");
        extensions.ext_hp_mixed_reality_controller = false;
    }
    if extensions.ext_palm_pose && !supported_extensions.ext_palm_pose {
        log::warn!("ext_palm_pose required. But not suppored");
        extensions.ext_palm_pose = false;
    }
    if extensions.ext_uuid && !supported_extensions.ext_uuid {
        log::warn!("ext_uuid required. But not suppored");
        extensions.ext_uuid = false;
    }
    if extensions.ext_hand_interaction && !supported_extensions.ext_hand_interaction {
        log::warn!("ext_hand_interaction required. But not suppored");
        extensions.ext_hand_interaction = false;
    }
    if extensions.ext_active_action_set_priority
        && !supported_extensions.ext_active_action_set_priority
    {
        log::warn!("ext_active_action_set_priority required. But not suppored");
        extensions.ext_active_action_set_priority = false;
    }
    if extensions.ext_local_floor && !supported_extensions.ext_local_floor {
        log::warn!("ext_local_floor required. But not suppored");
        extensions.ext_local_floor = false;
    }
    if extensions.ext_hand_tracking_data_source
        && !supported_extensions.ext_hand_tracking_data_source
    {
        log::warn!("ext_hand_tracking_data_source required. But not suppored");
        extensions.ext_hand_tracking_data_source = false;
    }
    if extensions.ext_plane_detection && !supported_extensions.ext_plane_detection {
        log::warn!("ext_plane_detection required. But not suppored");
        extensions.ext_plane_detection = false;
    }
    if extensions.ext_future && !supported_extensions.ext_future {
        log::warn!("ext_future required. But not suppored");
        extensions.ext_future = false;
    }
    if extensions.ext_user_presence && !supported_extensions.ext_user_presence {
        log::warn!("ext_user_presence required. But not suppored");
        extensions.ext_user_presence = false;
    }
    if extensions.fb_composition_layer_image_layout
        && !supported_extensions.fb_composition_layer_image_layout
    {
        log::warn!("fb_composition_layer_image_layout required. But not suppored");
        extensions.fb_composition_layer_image_layout = false;
    }
    if extensions.fb_composition_layer_alpha_blend
        && !supported_extensions.fb_composition_layer_alpha_blend
    {
        log::warn!("fb_composition_layer_alpha_blend required. But not suppored");
        extensions.fb_composition_layer_alpha_blend = false;
    }
    #[cfg(target_os = "android")]
    if extensions.fb_android_surface_swapchain_create
        && !supported_extensions.fb_android_surface_swapchain_create
    {
        log::warn!("fb_android_surface_swapchain_create required. But not supported");
        extensions.fb_android_surface_swapchain_create = false;
    }
    if extensions.fb_swapchain_update_state && !supported_extensions.fb_swapchain_update_state {
        log::warn!("fb_swapchain_update_state required. But not supported");
        extensions.fb_swapchain_update_state = false;
    }
    if extensions.fb_composition_layer_secure_content
        && !supported_extensions.fb_composition_layer_secure_content
    {
        log::warn!("fb_composition_layer_secure_content required. But not supported");
        extensions.fb_composition_layer_secure_content = false;
    }
    if extensions.fb_body_tracking && !supported_extensions.fb_body_tracking {
        log::warn!("fb_body_tracking required. But not supported");
        extensions.fb_body_tracking = false;
    }
    if extensions.fb_display_refresh_rate && !supported_extensions.fb_display_refresh_rate {
        log::warn!("fb_display_refresh_rate required. But not supported");
        extensions.fb_display_refresh_rate = false;
    }
    if extensions.fb_color_space && !supported_extensions.fb_color_space {
        log::warn!("fb_color_space required. But not supported");
        extensions.fb_color_space = false;
    }
    if extensions.fb_hand_tracking_mesh && !supported_extensions.fb_hand_tracking_mesh {
        log::warn!("fb_hand_tracking_mesh required. But not supported");
        extensions.fb_hand_tracking_mesh = false;
    }
    if extensions.fb_hand_tracking_aim && !supported_extensions.fb_hand_tracking_aim {
        log::warn!("fb_hand_tracking_aim required. But not supported");
        extensions.fb_hand_tracking_aim = false;
    }
    if extensions.fb_hand_tracking_capsules && !supported_extensions.fb_hand_tracking_capsules {
        log::warn!("fb_hand_tracking_capsules required. But not supported");
        extensions.fb_hand_tracking_capsules = false;
    }
    if extensions.fb_spatial_entity && !supported_extensions.fb_spatial_entity {
        log::warn!("fb_spatial_entity required. But not supported");
        extensions.fb_spatial_entity = false;
    }
    if extensions.fb_foveation && !supported_extensions.fb_foveation {
        log::warn!("fb_foveation required. But not supported");
        extensions.fb_foveation = false;
    }
    if extensions.fb_foveation_configuration && !supported_extensions.fb_foveation_configuration {
        log::warn!("fb_foveation_configuration required. But not supported");
        extensions.fb_foveation_configuration = false;
    }
    if extensions.fb_keyboard_tracking && !supported_extensions.fb_keyboard_tracking {
        log::warn!("fb_keyboard_tracking required. But not supported");
        extensions.fb_keyboard_tracking = false;
    }
    if extensions.fb_triangle_mesh && !supported_extensions.fb_triangle_mesh {
        log::warn!("fb_triangle_mesh required. But not supported");
        extensions.fb_triangle_mesh = false;
    }
    if extensions.fb_passthrough && !supported_extensions.fb_passthrough {
        log::warn!("fb_passthrough required. But not supported");
        extensions.fb_passthrough = false;
    }
    if extensions.fb_render_model && !supported_extensions.fb_render_model {
        log::warn!("fb_render_model required. But not supported");
        extensions.fb_render_model = false;
    }
    if extensions.fb_spatial_entity_query && !supported_extensions.fb_spatial_entity_query {
        log::warn!("fb_spatial_entity_query required. But not supported");
        extensions.fb_spatial_entity_query = false;
    }
    if extensions.fb_spatial_entity_storage && !supported_extensions.fb_spatial_entity_storage {
        log::warn!("fb_spatial_entity_storage required. But not supported");
        extensions.fb_spatial_entity_storage = false;
    }
    if extensions.fb_foveation_vulkan && !supported_extensions.fb_foveation_vulkan {
        log::warn!("fb_foveation_vulkan required. But not supported");
        extensions.fb_foveation_vulkan = false;
    }
    #[cfg(target_os = "android")]
    if extensions.fb_swapchain_update_state_android_surface
        && !supported_extensions.fb_swapchain_update_state_android_surface
    {
        log::warn!("fb_swapchain_update_state_android_surface required. But not supported");
        extensions.fb_swapchain_update_state_android_surface = false;
    }
    if extensions.fb_swapchain_update_state_opengl_es
        && !supported_extensions.fb_swapchain_update_state_opengl_es
    {
        log::warn!("fb_swapchain_update_state_opengl_es required. But not supported");
        extensions.fb_swapchain_update_state_opengl_es = false;
    }
    if extensions.fb_swapchain_update_state_vulkan
        && !supported_extensions.fb_swapchain_update_state_vulkan
    {
        log::warn!("fb_swapchain_update_state_vulkan required. But not supported");
        extensions.fb_swapchain_update_state_vulkan = false;
    }
    if extensions.fb_touch_controller_pro && !supported_extensions.fb_touch_controller_pro {
        log::warn!("fb_touch_controller_pro required. But not supported");
        extensions.fb_touch_controller_pro = false;
    }
    if extensions.fb_spatial_entity_sharing && !supported_extensions.fb_spatial_entity_sharing {
        log::warn!("fb_spatial_entity_sharing required. But not supported");
        extensions.fb_spatial_entity_sharing = false;
    }
    if extensions.fb_space_warp && !supported_extensions.fb_space_warp {
        log::warn!("fb_space_warp required. But not supported");
        extensions.fb_space_warp = false;
    }
    if extensions.fb_haptic_amplitude_envelope && !supported_extensions.fb_haptic_amplitude_envelope
    {
        log::warn!("fb_haptic_amplitude_envelope required. But not supported");
        extensions.fb_haptic_amplitude_envelope = false;
    }
    if extensions.fb_scene && !supported_extensions.fb_scene {
        log::warn!("fb_scene required. But not supported");
        extensions.fb_scene = false;
    }
    if extensions.fb_scene_capture && !supported_extensions.fb_scene_capture {
        log::warn!("fb_scene_capture required. But not supported");
        extensions.fb_scene_capture = false;
    }
    if extensions.fb_spatial_entity_container && !supported_extensions.fb_spatial_entity_container {
        log::warn!("fb_spatial_entity_container required. But not supported");
        extensions.fb_spatial_entity_container = false;
    }
    if extensions.fb_face_tracking && !supported_extensions.fb_face_tracking {
        log::warn!("fb_face_tracking required. But not supported");
        extensions.fb_face_tracking = false;
    }
    if extensions.fb_eye_tracking_social && !supported_extensions.fb_eye_tracking_social {
        log::warn!("fb_eye_tracking_social required. But not supported");
        extensions.fb_eye_tracking_social = false;
    }
    if extensions.fb_passthrough_keyboard_hands
        && !supported_extensions.fb_passthrough_keyboard_hands
    {
        log::warn!("fb_passthrough_keyboard_hands required. But not supported");
        extensions.fb_passthrough_keyboard_hands = false;
    }
    if extensions.fb_composition_layer_settings
        && !supported_extensions.fb_composition_layer_settings
    {
        log::warn!("fb_composition_layer_settings required. But not supported");
        extensions.fb_composition_layer_settings = false;
    }
    if extensions.fb_touch_controller_proximity
        && !supported_extensions.fb_touch_controller_proximity
    {
        log::warn!("fb_touch_controller_proximity required. But not supported");
        extensions.fb_touch_controller_proximity = false;
    }
    if extensions.fb_haptic_pcm && !supported_extensions.fb_haptic_pcm {
        log::warn!("fb_haptic_pcm required. But not supported");
        extensions.fb_haptic_pcm = false;
    }
    if extensions.fb_composition_layer_depth_test
        && !supported_extensions.fb_composition_layer_depth_test
    {
        log::warn!("fb_composition_layer_depth_test required. But not supported");
        extensions.fb_composition_layer_depth_test = false;
    }
    if extensions.fb_spatial_entity_storage_batch
        && !supported_extensions.fb_spatial_entity_storage_batch
    {
        log::warn!("fb_spatial_entity_storage_batch required. But not supported");
        extensions.fb_spatial_entity_storage_batch = false;
    }
    if extensions.fb_spatial_entity_user && !supported_extensions.fb_spatial_entity_user {
        log::warn!("fb_spatial_entity_user required. But not supported");
        extensions.fb_spatial_entity_user = false;
    }
    if extensions.fb_face_tracking2 && !supported_extensions.fb_face_tracking2 {
        log::warn!("fb_face_tracking2 required. But not supported");
        extensions.fb_face_tracking2 = false;
    }
    if extensions.htc_vive_cosmos_controller_interaction
        && !supported_extensions.htc_vive_cosmos_controller_interaction
    {
        log::warn!("htc_vive_cosmos_controller_interaction required. But not supported");
        extensions.htc_vive_cosmos_controller_interaction = false;
    }
    if extensions.htc_facial_tracking && !supported_extensions.htc_facial_tracking {
        log::warn!("htc_facial_tracking required. But not supported");
        extensions.htc_facial_tracking = false;
    }
    if extensions.htc_vive_focus3_controller_interaction
        && !supported_extensions.htc_vive_focus3_controller_interaction
    {
        log::warn!("htc_vive_focus3_controller_interaction required. But not supported");
        extensions.htc_vive_focus3_controller_interaction = false;
    }
    if extensions.htc_hand_interaction && !supported_extensions.htc_hand_interaction {
        log::warn!("htc_hand_interaction required. But not supported");
        extensions.htc_hand_interaction = false;
    }
    if extensions.htc_vive_wrist_tracker_interaction
        && !supported_extensions.htc_vive_wrist_tracker_interaction
    {
        log::warn!("htc_vive_wrist_tracker_interaction required. But not supported");
        extensions.htc_vive_wrist_tracker_interaction = false;
    }
    if extensions.htc_passthrough && !supported_extensions.htc_passthrough {
        log::warn!("htc_passthrough required. But not supported");
        extensions.htc_passthrough = false;
    }
    if extensions.htc_foveation && !supported_extensions.htc_foveation {
        log::warn!("htc_foveation required. But not supported");
        extensions.htc_foveation = false;
    }
    if extensions.htc_anchor && !supported_extensions.htc_anchor {
        log::warn!("htc_anchor required. But not supported");
        extensions.htc_anchor = false;
    }
    if extensions.huawei_controller_interaction
        && !supported_extensions.huawei_controller_interaction
    {
        log::warn!("huawei_controller_interaction required. But not supported");
        extensions.huawei_controller_interaction = false;
    }
    #[cfg(target_os = "android")]
    if extensions.khr_android_thread_settings && !supported_extensions.khr_android_thread_settings {
        log::warn!("khr_android_thread_settings required. But not supported");
        extensions.khr_android_thread_settings = false;
    }
    #[cfg(target_os = "android")]
    if extensions.khr_android_surface_swapchain
        && !supported_extensions.khr_android_surface_swapchain
    {
        log::warn!("khr_android_surface_swapchain required. But not supported");
        extensions.khr_android_surface_swapchain = false;
    }
    if extensions.khr_composition_layer_cube && !supported_extensions.khr_composition_layer_cube {
        log::warn!("khr_composition_layer_cube required. But not supported");
        extensions.khr_composition_layer_cube = false;
    }
    #[cfg(target_os = "android")]
    if extensions.khr_android_create_instance && !supported_extensions.khr_android_create_instance {
        log::warn!("khr_android_create_instance required. But not supported");
        extensions.khr_android_create_instance = false;
    }
    if extensions.khr_composition_layer_depth && !supported_extensions.khr_composition_layer_depth {
        log::warn!("khr_composition_layer_depth required. But not supported");
        extensions.khr_composition_layer_depth = false;
    }
    if extensions.khr_vulkan_swapchain_format_list
        && !supported_extensions.khr_vulkan_swapchain_format_list
    {
        log::warn!("khr_vulkan_swapchain_format_list required. But not supported");
        extensions.khr_vulkan_swapchain_format_list = false;
    }
    if extensions.khr_composition_layer_cylinder
        && !supported_extensions.khr_composition_layer_cylinder
    {
        log::warn!("khr_composition_layer_cylinder required. But not supported");
        extensions.khr_composition_layer_cylinder = false;
    }
    if extensions.khr_composition_layer_equirect
        && !supported_extensions.khr_composition_layer_equirect
    {
        log::warn!("khr_composition_layer_equirect required. But not supported");
        extensions.khr_composition_layer_equirect = false;
    }
    if extensions.khr_opengl_enable && !supported_extensions.khr_opengl_enable {
        log::warn!("khr_opengl_enable required. But not supported");
        extensions.khr_opengl_enable = false;
    }
    if extensions.khr_opengl_es_enable && !supported_extensions.khr_opengl_es_enable {
        log::warn!("khr_opengl_es_enable required. But not supported");
        extensions.khr_opengl_es_enable = false;
    }
    if extensions.khr_vulkan_enable && !supported_extensions.khr_vulkan_enable {
        log::warn!("khr_vulkan_enable required. But not supported");
        extensions.khr_vulkan_enable = false;
    }
    #[cfg(windows)]
    if extensions.khr_d3d11_enable && !supported_extensions.khr_d3d11_enable {
        log::warn!("khr_d3d11_enable required. But not supported");
        extensions.khr_d3d11_enable = false;
    }
    #[cfg(windows)]
    if extensions.khr_d3d12_enable && !supported_extensions.khr_d3d12_enable {
        log::warn!("khr_d3d12_enable required. But not supported");
        extensions.khr_d3d12_enable = false;
    }
    if extensions.khr_visibility_mask && !supported_extensions.khr_visibility_mask {
        log::warn!("khr_visibility_mask required. But not supported");
        extensions.khr_visibility_mask = false;
    }
    if extensions.khr_composition_layer_color_scale_bias
        && !supported_extensions.khr_composition_layer_color_scale_bias
    {
        log::warn!("khr_composition_layer_color_scale_bias required. But not supported");
        extensions.khr_composition_layer_color_scale_bias = false;
    }
    #[cfg(windows)]
    if extensions.khr_win32_convert_performance_counter_time
        && !supported_extensions.khr_win32_convert_performance_counter_time
    {
        log::warn!("khr_win32_convert_performance_counter_time required. But not supported");
        extensions.khr_win32_convert_performance_counter_time = false;
    }
    if extensions.khr_convert_timespec_time && !supported_extensions.khr_convert_timespec_time {
        log::warn!("khr_convert_timespec_time required. But not supported");
        extensions.khr_convert_timespec_time = false;
    }
    if extensions.khr_loader_init && !supported_extensions.khr_loader_init {
        log::warn!("khr_loader_init required. But not supported");
        extensions.khr_loader_init = false;
    }
    #[cfg(target_os = "android")]
    if extensions.khr_loader_init_android && !supported_extensions.khr_loader_init_android {
        log::warn!("khr_loader_init_android required. But not supported");
        extensions.khr_loader_init_android = false;
    }
    if extensions.khr_vulkan_enable2 && !supported_extensions.khr_vulkan_enable2 {
        log::warn!("khr_vulkan_enable2 required. But not supported");
        extensions.khr_vulkan_enable2 = false;
    }
    if extensions.khr_composition_layer_equirect2
        && !supported_extensions.khr_composition_layer_equirect2
    {
        log::warn!("khr_composition_layer_equirect2 required. But not supported");
        extensions.khr_composition_layer_equirect2 = false;
    }
    if extensions.khr_binding_modification && !supported_extensions.khr_binding_modification {
        log::warn!("khr_binding_modification required. But not supported");
        extensions.khr_binding_modification = false;
    }
    if extensions.khr_swapchain_usage_input_attachment_bit
        && !supported_extensions.khr_swapchain_usage_input_attachment_bit
    {
        log::warn!("khr_swapchain_usage_input_attachment_bit required. But not supported");
        extensions.khr_swapchain_usage_input_attachment_bit = false;
    }
    if extensions.khr_locate_spaces && !supported_extensions.khr_locate_spaces {
        log::warn!("khr_locate_spaces required. But not supported");
        extensions.khr_locate_spaces = false;
    }
    if extensions.khr_maintenance1 && !supported_extensions.khr_maintenance1 {
        log::warn!("khr_maintenance1 required. But not supported");
        extensions.khr_maintenance1 = false;
    }
    if extensions.meta_foveation_eye_tracked && !supported_extensions.meta_foveation_eye_tracked {
        log::warn!("meta_foveation_eye_tracked required. But not supported");
        extensions.meta_foveation_eye_tracked = false;
    }
    if extensions.meta_local_dimming && !supported_extensions.meta_local_dimming {
        log::warn!("meta_local_dimming required. But not supported");
        extensions.meta_local_dimming = false;
    }
    if extensions.meta_passthrough_preferences && !supported_extensions.meta_passthrough_preferences
    {
        log::warn!("meta_passthrough_preferences required. But not supported");
        extensions.meta_passthrough_preferences = false;
    }
    if extensions.meta_virtual_keyboard && !supported_extensions.meta_virtual_keyboard {
        log::warn!("meta_virtual_keyboard required. But not supported");
        extensions.meta_virtual_keyboard = false;
    }
    if extensions.meta_vulkan_swapchain_create_info
        && !supported_extensions.meta_vulkan_swapchain_create_info
    {
        log::warn!("meta_vulkan_swapchain_create_info required. But not supported");
        extensions.meta_vulkan_swapchain_create_info = false;
    }
    if extensions.meta_performance_metrics && !supported_extensions.meta_performance_metrics {
        log::warn!("meta_performance_metrics required. But not supported");
        extensions.meta_performance_metrics = false;
    }
    if extensions.meta_headset_id && !supported_extensions.meta_headset_id {
        log::warn!("meta_headset_id required. But not supported");
        extensions.meta_headset_id = false;
    }
    if extensions.meta_recommended_layer_resolution
        && !supported_extensions.meta_recommended_layer_resolution
    {
        log::warn!("meta_recommended_layer_resolution required. But not supported");
        extensions.meta_recommended_layer_resolution = false;
    }
    if extensions.meta_passthrough_color_lut && !supported_extensions.meta_passthrough_color_lut {
        log::warn!("meta_passthrough_color_lut required. But not supported");
        extensions.meta_passthrough_color_lut = false;
    }
    if extensions.meta_spatial_entity_mesh && !supported_extensions.meta_spatial_entity_mesh {
        log::warn!("meta_spatial_entity_mesh required. But not supported");
        extensions.meta_spatial_entity_mesh = false;
    }
    if extensions.meta_automatic_layer_filter && !supported_extensions.meta_automatic_layer_filter {
        log::warn!("meta_automatic_layer_filter required. But not supported");
        extensions.meta_automatic_layer_filter = false;
    }
    if extensions.meta_touch_controller_plus && !supported_extensions.meta_touch_controller_plus {
        log::warn!("meta_touch_controller_plus required. But not supported");
        extensions.meta_touch_controller_plus = false;
    }
    if extensions.meta_environment_depth && !supported_extensions.meta_environment_depth {
        log::warn!("meta_environment_depth required. But not supported");
        extensions.meta_environment_depth = false;
    }
    if extensions.ml_ml2_controller_interaction
        && !supported_extensions.ml_ml2_controller_interaction
    {
        log::warn!("ml_ml2_controller_interaction required. But not supported");
        extensions.ml_ml2_controller_interaction = false;
    }
    if extensions.ml_frame_end_info && !supported_extensions.ml_frame_end_info {
        log::warn!("ml_frame_end_info required. But not supported");
        extensions.ml_frame_end_info = false;
    }
    if extensions.ml_global_dimmer && !supported_extensions.ml_global_dimmer {
        log::warn!("ml_global_dimmer required. But not supported");
        extensions.ml_global_dimmer = false;
    }
    if extensions.ml_compat && !supported_extensions.ml_compat {
        log::warn!("ml_compat required. But not supported");
        extensions.ml_compat = false;
    }
    if extensions.ml_marker_understanding && !supported_extensions.ml_marker_understanding {
        log::warn!("ml_marker_understanding required. But not supported");
        extensions.ml_marker_understanding = false;
    }
    if extensions.ml_localization_map && !supported_extensions.ml_localization_map {
        log::warn!("ml_localization_map required. But not supported");
        extensions.ml_localization_map = false;
    }
    if extensions.ml_user_calibration && !supported_extensions.ml_user_calibration {
        log::warn!("ml_user_calibration required. But not supported");
        extensions.ml_user_calibration = false;
    }
    if extensions.mnd_headless && !supported_extensions.mnd_headless {
        log::warn!("mnd_headless required. But not supported");
        extensions.mnd_headless = false;
    }
    if extensions.mnd_swapchain_usage_input_attachment_bit
        && !supported_extensions.mnd_swapchain_usage_input_attachment_bit
    {
        log::warn!("mnd_swapchain_usage_input_attachment_bit required. But not supported");
        extensions.mnd_swapchain_usage_input_attachment_bit = false;
    }
    if extensions.msft_unbounded_reference_space
        && !supported_extensions.msft_unbounded_reference_space
    {
        log::warn!("msft_unbounded_reference_space required. But not supported");
        extensions.msft_unbounded_reference_space = false;
    }
    if extensions.msft_spatial_anchor && !supported_extensions.msft_spatial_anchor {
        log::warn!("msft_spatial_anchor required. But not supported");
        extensions.msft_spatial_anchor = false;
    }
    if extensions.msft_spatial_graph_bridge && !supported_extensions.msft_spatial_graph_bridge {
        log::warn!("msft_spatial_graph_bridge required. But not supported");
        extensions.msft_spatial_graph_bridge = false;
    }
    if extensions.msft_hand_interaction && !supported_extensions.msft_hand_interaction {
        log::warn!("msft_hand_interaction required. But not supported");
        extensions.msft_hand_interaction = false;
    }
    if extensions.msft_hand_tracking_mesh && !supported_extensions.msft_hand_tracking_mesh {
        log::warn!("msft_hand_tracking_mesh required. But not supported");
        extensions.msft_hand_tracking_mesh = false;
    }
    if extensions.msft_secondary_view_configuration
        && !supported_extensions.msft_secondary_view_configuration
    {
        log::warn!("msft_secondary_view_configuration required. But not supported");
        extensions.msft_secondary_view_configuration = false;
    }
    if extensions.msft_first_person_observer && !supported_extensions.msft_first_person_observer {
        log::warn!("msft_first_person_observer required. But not supported");
        extensions.msft_first_person_observer = false;
    }
    if extensions.msft_controller_model && !supported_extensions.msft_controller_model {
        log::warn!("msft_controller_model required. But not supported");
        extensions.msft_controller_model = false;
    }
    #[cfg(windows)]
    if extensions.msft_perception_anchor_interop
        && !supported_extensions.msft_perception_anchor_interop
    {
        log::warn!("msft_perception_anchor_interop required. But not supported");
        extensions.msft_perception_anchor_interop = false;
    }
    #[cfg(windows)]
    if extensions.msft_holographic_window_attachment
        && !supported_extensions.msft_holographic_window_attachment
    {
        log::warn!("msft_holographic_window_attachment required. But not supported");
        extensions.msft_holographic_window_attachment = false;
    }
    if extensions.msft_composition_layer_reprojection
        && !supported_extensions.msft_composition_layer_reprojection
    {
        log::warn!("msft_composition_layer_reprojection required. But not supported");
        extensions.msft_composition_layer_reprojection = false;
    }
    if extensions.msft_spatial_anchor_persistence
        && !supported_extensions.msft_spatial_anchor_persistence
    {
        log::warn!("msft_spatial_anchor_persistence required. But not supported");
        extensions.msft_spatial_anchor_persistence = false;
    }
    #[cfg(target_os = "android")]
    if extensions.oculus_android_session_state_enable
        && !supported_extensions.oculus_android_session_state_enable
    {
        log::warn!("oculus_android_session_state_enable required. But not supported");
        extensions.oculus_android_session_state_enable = false;
    }
    if extensions.oculus_audio_device_guid && !supported_extensions.oculus_audio_device_guid {
        log::warn!("oculus_audio_device_guid required. But not supported");
        extensions.oculus_audio_device_guid = false;
    }
    if extensions.oculus_external_camera && !supported_extensions.oculus_external_camera {
        log::warn!("oculus_external_camera required. But not supported");
        extensions.oculus_external_camera = false;
    }
    if extensions.oppo_controller_interaction && !supported_extensions.oppo_controller_interaction {
        log::warn!("oppo_controller_interaction required. But not supported");
        extensions.oppo_controller_interaction = false;
    }
    if extensions.qcom_tracking_optimization_settings
        && !supported_extensions.qcom_tracking_optimization_settings
    {
        log::warn!("qcom_tracking_optimization_settings required. But not supported");
        extensions.qcom_tracking_optimization_settings = false;
    }
    if extensions.ultraleap_hand_tracking_forearm
        && !supported_extensions.ultraleap_hand_tracking_forearm
    {
        log::warn!("ultraleap_hand_tracking_forearm required. But not supported");
        extensions.ultraleap_hand_tracking_forearm = false;
    }
    if extensions.valve_analog_threshold && !supported_extensions.valve_analog_threshold {
        log::warn!("valve_analog_threshold required. But not supported");
        extensions.valve_analog_threshold = false;
    }
    if extensions.varjo_quad_views && !supported_extensions.varjo_quad_views {
        log::warn!("varjo_quad_views required. But not supported");
        extensions.varjo_quad_views = false;
    }
    if extensions.varjo_foveated_rendering && !supported_extensions.varjo_foveated_rendering {
        log::warn!("varjo_foveated_rendering required. But not supported");
        extensions.varjo_foveated_rendering = false;
    }
    if extensions.varjo_composition_layer_depth_test
        && !supported_extensions.varjo_composition_layer_depth_test
    {
        log::warn!("varjo_composition_layer_depth_test required. But not supported");
        extensions.varjo_composition_layer_depth_test = false;
    }
    if extensions.varjo_environment_depth_estimation
        && !supported_extensions.varjo_environment_depth_estimation
    {
        log::warn!("varjo_environment_depth_estimation required. But not supported");
        extensions.varjo_environment_depth_estimation = false;
    }
    if extensions.varjo_marker_tracking && !supported_extensions.varjo_marker_tracking {
        log::warn!("varjo_marker_tracking required. But not supported");
        extensions.varjo_marker_tracking = false;
    }
    if extensions.varjo_view_offset && !supported_extensions.varjo_view_offset {
        log::warn!("varjo_view_offset required. But not supported");
        extensions.varjo_view_offset = false;
    }
    if extensions.varjo_xr4_controller_interaction
        && !supported_extensions.varjo_xr4_controller_interaction
    {
        log::warn!("varjo_xr4_controller_interaction required. But not supported");
        extensions.varjo_xr4_controller_interaction = false;
    }
    if extensions.yvr_controller_interaction && !supported_extensions.yvr_controller_interaction {
        log::warn!("yvr_controller_interaction required. But not supported");
        extensions.yvr_controller_interaction = false;
    }
    if extensions.extx_overlay && !supported_extensions.extx_overlay {
        log::warn!("extx_overlay required. But not supported");
        extensions.extx_overlay = false;
    }
    if extensions.mndx_egl_enable && !supported_extensions.mndx_egl_enable {
        log::warn!("mndx_egl_enable required. But not supported");
        extensions.mndx_egl_enable = false;
    }
    if extensions.mndx_force_feedback_curl && !supported_extensions.mndx_force_feedback_curl {
        log::warn!("mndx_force_feedback_curl required. But not supported");
        extensions.mndx_force_feedback_curl = false;
    }
    if extensions.htcx_vive_tracker_interaction
        && !supported_extensions.htcx_vive_tracker_interaction
    {
        log::warn!("htcx_vive_tracker_interaction required. But not supported");
        extensions.htcx_vive_tracker_interaction = false;
    }
    let supported_extensions_other_set: HashSet<String> =
        supported_extensions.other.iter().cloned().collect();
    extensions.other = extensions
        .other
        .iter()
        .filter_map(|e| match supported_extensions_other_set.contains(e) {
            true => Some(e.clone()),
            false => {
                log::warn!("{} required. But not supported", e);
                None
            }
        })
        .collect();

    Ok(extensions)
}
