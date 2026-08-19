use bevy::{
    app::PluginGroupBuilder,
    prelude::*,
    render::{pipelined_rendering::PipelinedRenderingPlugin, RenderPlugin},
    state::app::StatesPlugin,
};

pub(crate) mod backends;
pub(crate) mod openxr;
#[cfg(target_os = "windows")]
mod windows;

/// Initialize the OpenXR loader on Android.
///
/// MUST be called before any other OpenXR function (including `Entry::load()`).
/// On Android the loader needs the JavaVM and Activity context to locate the
/// runtime. Without this call, `xrEnumerateInstanceExtensionProperties` fails
/// with "LoaderInitData not initialized" and XR falls back to desktop mode.
///
/// Uses `dlopen`/`dlsym` to avoid a strong undefined reference to
/// `xrInitializeLoaderKHR` in the `.so`, which would prevent the library from
/// loading on devices where the symbol is not yet visible at load time.
///
/// # Safety
/// `vm` must be a valid `JavaVM*` and `context` a valid `Activity` jobject for
/// the lifetime of the process.
#[cfg(target_os = "android")]
pub unsafe fn initialize_openxr_loader_android(
    vm:      *mut std::ffi::c_void,
    context: *mut std::ffi::c_void,
) {
    use std::ffi::c_void;

    // dlopen/dlsym are standard POSIX functions always available on Android.
    extern "C" {
        fn dlopen(filename: *const i8, flag: i32) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
    }
    const RTLD_NOW:    i32 = 2;
    const RTLD_GLOBAL: i32 = 256; // make symbols visible to subsequently loaded libs

    extern "C" { fn dlerror() -> *const i8; }

    let handle = dlopen(
        b"libopenxr_loader.so\0".as_ptr() as *const i8,
        RTLD_NOW | RTLD_GLOBAL,
    );
    if handle.is_null() {
        let err = dlerror();
        let msg = if err.is_null() { std::borrow::Cow::Borrowed("unknown") }
                  else { std::ffi::CStr::from_ptr(err as *const u8).to_string_lossy() };
        log::warn!("[xrds] dlopen libopenxr_loader.so failed: {}", msg);
        return;
    }

    // The Khronos loader does not export xrInitializeLoaderKHR as a direct dlsym symbol;
    // it exposes it through xrGetInstanceProcAddr(XR_NULL_HANDLE, "xrInitializeLoaderKHR").
    let get_proc_ptr = dlsym(handle, b"xrGetInstanceProcAddr\0".as_ptr() as *const i8);
    if get_proc_ptr.is_null() {
        log::warn!("[xrds] xrGetInstanceProcAddr not found in loader — XR will be unavailable");
        return;
    }

    // xrGetInstanceProcAddr(XrInstance, const char*, PFN_xrVoidFunction*) -> XrResult
    // XrInstance is an opaque 64-bit handle; XR_NULL_HANDLE = 0.
    type PfnGetProcAddr = unsafe extern "C" fn(u64, *const i8, *mut *mut c_void) -> i32;
    let xr_get_proc: PfnGetProcAddr = std::mem::transmute(get_proc_ptr);

    let mut init_loader_fn: *mut c_void = std::ptr::null_mut();
    let res = xr_get_proc(
        0, // XR_NULL_HANDLE
        b"xrInitializeLoaderKHR\0".as_ptr() as *const i8,
        &mut init_loader_fn,
    );
    if res != 0 || init_loader_fn.is_null() {
        // Loader is pre-initialized (system loader) or the extension is absent.
        // Library is already loaded via RTLD_GLOBAL; Entry::load() will use it.
        log::info!("[xrds] xrInitializeLoaderKHR not available (res={:08x}) — assuming pre-initialized loader", res);
        return;
    }

    // XR_TYPE_LOADER_INIT_INFO_ANDROID_KHR = 1000089000 (OpenXR spec).
    const XR_TYPE_LOADER_INIT_INFO_ANDROID_KHR: i32 = 1000089000;
    #[repr(C)]
    struct XrLoaderInitInfoAndroidKHR {
        ty:                  i32,
        next:                *const c_void,
        application_vm:      *mut c_void,
        application_context: *mut c_void,
    }
    type FnInitLoader = unsafe extern "C" fn(*const XrLoaderInitInfoAndroidKHR) -> i32;
    let xr_init: FnInitLoader = std::mem::transmute(init_loader_fn);

    let info = XrLoaderInitInfoAndroidKHR {
        ty:                  XR_TYPE_LOADER_INIT_INFO_ANDROID_KHR,
        next:                std::ptr::null(),
        application_vm:      vm,
        application_context: context,
    };
    let result = xr_init(&info);
    if result == 0 {
        log::info!("[xrds] xrInitializeLoaderKHR succeeded");
    } else {
        log::warn!("[xrds] xrInitializeLoaderKHR returned {:08x} — XR may be unavailable", result);
    }
    // Do NOT dlclose: keep the library loaded so Entry::load() reuses the same instance.
}

pub use openxr::camera::OpenXrCamera;
pub use openxr::camera::OpenXrCameraIndex;
pub use openxr::camera::OpenXrPlayerRoot;
pub use openxr::input::{XrHand, XrHapticRequest, XrInput, XrInputSource, XrPointerState};
pub use openxr::render_model::XrControllerModelAssets;
/// Switches the `XR_FB_passthrough` composition layer on and off.
///
/// Re-exported because `xrds-runtime` projects `XrdsSceneMetadata::xr_blend_mode`
/// onto it; see `xrds-runtime`'s `xrds_api::passthrough`.
pub use openxr::resources::OpenXrPassthroughEnabled;

use crate::openxr::{
    blit::OpenXrBlitPlugin,
    camera::OpenXrCameraPlugin, init::OpenXrInitPlugin,
    input::XrInputPlugin,
    reference_space::OpenXrReferenceSpacePlugin, render::OpenXrRenderPlugin,
    render_model::ControllerModelPlugin,
    session::OpenXrSessionPlugin, swapchain::OpenXrSwapchainPlugin,
};

/// Returns `true` if an OpenXR runtime is installed and detectable on this system.
///
/// On Windows this reads the registry key written by runtime installers (SteamVR, Oculus, WMR, etc.).
/// On other platforms it attempts to load the shared OpenXR library from the system linker path.
/// Call this before adding XR plugins so the app can fall back to desktop rendering gracefully.
pub fn is_openxr_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::is_runtime_registered()
    }
    #[cfg(not(target_os = "windows"))]
    {
        unsafe { ::openxr::Entry::load() }.is_ok()
    }
}

pub fn add_plugins<PG: PluginGroup>(base_plugins: PG, app_name: String) -> PluginGroupBuilder {
    let plugin_builder = base_plugins
        .build()
        .disable::<RenderPlugin>()
        .disable::<StatesPlugin>()
        .disable::<PipelinedRenderingPlugin>()
        .add_before::<RenderPlugin>(StatesPlugin)
        .add_before::<RenderPlugin>(OpenXrInitPlugin {
            app_name,
            ..Default::default()
        })
        .add(OpenXrSessionPlugin)
        .add(OpenXrReferenceSpacePlugin)
        .add(OpenXrSwapchainPlugin)
        .add(OpenXrCameraPlugin)
        .add(OpenXrRenderPlugin)
        .add(OpenXrBlitPlugin)
        .add(XrInputPlugin)
        .add(ControllerModelPlugin);

    #[cfg(feature = "preview_window")]
    let plugin_builder = {
        plugin_builder.set(WindowPlugin {
            primary_window: Some(Window {
                transparent: true,
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                // Keep the preview window focused so keyboard input reaches Bevy
                // even while the user is looking through the HMD.
                focused: true,
                ..Default::default()
            }),
            ..Default::default()
        })
    };

    plugin_builder
}
