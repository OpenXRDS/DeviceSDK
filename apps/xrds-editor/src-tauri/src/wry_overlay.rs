//! Phase 2 wry editor overlay.
//!
//! Single full-window WebView serving the React editor SPA.
//! On Windows, SetWindowRgn carves a viewport hole so Bevy renders there natively.
//! React sends exact bounds via IPC after each layout change; until then the
//! overlay uses approximate constants so the camera starts correctly.
//!
//!  ┌──────────────────────────────────────────────┐
//!  │   React SPA: menubar + toolbar (full width)  │
//!  ├──────────┬───────────────────────┬───────────┤
//!  │  left    │  Bevy viewport (hole) │  right    │
//!  │  sidebar │  SetWindowRgn cuts    │  inspector│
//!  ├──────────┴───────────────────────┴───────────┤
//!  │   React SPA: palette (full width)            │
//!  └──────────────────────────────────────────────┘

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested, WindowResized};
use crate::bevy_bridge::BevyBridgeResource;
use crate::bridge::EditorCommand;

// ---------------------------------------------------------------------------
// Layout constants (logical pixels) — kept for viewport_camera.rs
// ---------------------------------------------------------------------------

pub const LEFT_W:  u32 = 240;
pub const RIGHT_W: u32 = 280;
pub const TOP_H:   u32 = 62;
pub const BOT_H:   u32 = 170;   // palette max-height 160px + small buffer; React corrects exact bounds

// Custom protocol name and origin used when serving the pre-built dist/ bundle.
const PROTO: &str      = "xrds";
const PROTO_URL: &str  = "xrds://localhost/";
const DEV_URL: &str    = "http://localhost:5173";

fn dist_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist")
}

fn has_dist() -> bool { dist_dir().join("index.html").exists() }

/// Serve a file from the pre-built dist/ directory for the custom protocol.
/// Any path with no extension (SPA routes) falls back to index.html.
fn serve_dist(dist: &std::path::Path, req: wry::http::Request<Vec<u8>>)
    -> wry::http::Response<std::borrow::Cow<'static, [u8]>>
{
    use std::borrow::Cow;
    let rel = req.uri().path().trim_start_matches('/');
    let candidate = if rel.is_empty() || !rel.contains('.') {
        dist.join("index.html")
    } else {
        dist.join(rel)
    };
    let (path, mime) = if candidate.exists() {
        (candidate.clone(), mime_for(&candidate))
    } else {
        let idx = dist.join("index.html");
        let m   = mime_for(&idx);
        (idx, m)
    };
    let body: Cow<'static, [u8]> = Cow::Owned(std::fs::read(&path).unwrap_or_default());
    wry::http::Response::builder()
        .header("Content-Type", mime)
        .body(body)
        .unwrap()
}

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html")           => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript",
        Some("css")            => "text/css",
        Some("svg")            => "image/svg+xml",
        Some("png")            => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico")            => "image/x-icon",
        Some("wasm")           => "application/wasm",
        Some("json")           => "application/json",
        Some("woff2")          => "font/woff2",
        Some("woff")           => "font/woff",
        Some("ttf")            => "font/ttf",
        _                      => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// ViewportRect resource
// ---------------------------------------------------------------------------

/// Centre viewport bounds in logical pixels.  Updated by React's `viewport_bounds`
/// IPC message and by `handle_editor_resize` (approximate) on window resize.
#[derive(Resource)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for ViewportRect {
    fn default() -> Self {
        Self { x: LEFT_W as f32, y: TOP_H as f32, w: 0.0, h: 0.0 }
    }
}

impl ViewportRect {
    pub fn contains(&self, pos: Vec2) -> bool {
        self.w > 0.0
            && pos.x >= self.x && pos.x < self.x + self.w
            && pos.y >= self.y && pos.y < self.y + self.h
    }
}

// ---------------------------------------------------------------------------
// NonSend marker — forces any system that takes it as a param onto the main thread
// ---------------------------------------------------------------------------

/// Zero-size NonSend resource.  Any Bevy system that takes `NonSend<EditorWvMarker>`
/// is scheduled on the main thread, which is required for thread_local WebView access.
pub struct EditorWvMarker;

// ---------------------------------------------------------------------------
// Marker resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct WryEditorReady;

// ---------------------------------------------------------------------------
// Thread-local WebView storage
// ---------------------------------------------------------------------------

thread_local! {
    static EDITOR_WV: RefCell<Option<wry::WebView>> = RefCell::new(None);
}

// ---------------------------------------------------------------------------
// Shared cross-thread state
// ---------------------------------------------------------------------------

/// JavaScript snippets to evaluate on the next Bevy frame (file-dialog responses, etc.)
fn pending_responses() -> &'static Mutex<Vec<String>> {
    static S: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}

/// Exact logical-px viewport bounds reported by React's ResizeObserver.
fn pending_vp() -> &'static Mutex<Option<(f32, f32, f32, f32)>> {
    static S: OnceLock<Mutex<Option<(f32, f32, f32, f32)>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Pending stereo preview state change from the webview IPC.
/// Tuple: (enabled, ipd_m, fov_deg)
fn pending_stereo() -> &'static Mutex<Option<(bool, f32, f32)>> {
    static S: OnceLock<Mutex<Option<(bool, f32, f32)>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Container HWND raw value for SetWindowRgn.
/// Stored as isize (windows-sys HWND = isize) so the type is always valid.
#[cfg(windows)]
fn container_hwnd_val() -> &'static Mutex<isize> {
    static S: OnceLock<Mutex<isize>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(0))
}

/// Parent (Bevy window) HWND — used to steal keyboard focus back from the WebView.
#[cfg(windows)]
fn parent_hwnd_val() -> &'static Mutex<isize> {
    static S: OnceLock<Mutex<isize>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(0))
}

/// Last applied region parameters (win_w, win_h, vp_x, vp_y, vp_w, vp_h) in physical pixels.
/// Stored so the hole can be restored after a modal clears it.
#[cfg(windows)]
fn last_region_params() -> &'static Mutex<Option<(u32, u32, u32, u32, u32, u32)>> {
    static S: OnceLock<Mutex<Option<(u32, u32, u32, u32, u32, u32)>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Container X window ID for XShapeCombineRectangles.
#[cfg(target_os = "linux")]
fn container_xwin_val() -> &'static Mutex<u32> {
    static S: OnceLock<Mutex<u32>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(0))
}

/// Last applied region parameters on Linux — mirrors the Windows version.
#[cfg(target_os = "linux")]
fn last_region_params() -> &'static Mutex<Option<(u32, u32, u32, u32, u32, u32)>> {
    static S: OnceLock<Mutex<Option<(u32, u32, u32, u32, u32, u32)>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

// ---------------------------------------------------------------------------
// Approximate viewport geometry helpers
// ---------------------------------------------------------------------------

fn approx_vp_phys(pw: u32, ph: u32, sf: f32) -> (u32, u32, u32, u32) {
    let lx = (LEFT_W  as f32 * sf) as u32;
    let ly = (TOP_H   as f32 * sf) as u32;
    let rx = (RIGHT_W as f32 * sf) as u32;
    let by = (BOT_H   as f32 * sf) as u32;
    (lx, ly, pw.saturating_sub(lx + rx), ph.saturating_sub(ly + by))
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Exclusive-world system — retries every frame until WINIT_WINDOWS has the
/// window, then creates the single full-window editor WebView.
pub fn try_attach_wry_editor(world: &mut World) {
    if world.get_resource::<WryEditorReady>().is_some() { return; }

    let entity = {
        let mut q = world.query_filtered::<Entity, With<PrimaryWindow>>();
        match q.single(world) { Ok(e) => e, Err(_) => return }
    };

    let (win_w, win_h) = match world.get::<Window>(entity) {
        Some(w) => (w.width() as u32, w.height() as u32),
        None    => return,
    };

    // Clone the inbound queue handle for the IPC closure.
    let inbound = match world.get_resource::<BevyBridgeResource>() {
        Some(r) => Arc::clone(&r.0.inbound),
        None    => return,
    };

    let mut attached = false;

    bevy::winit::WINIT_WINDOWS.with_borrow(|ww| {
        let Some(win) = ww.get_window(entity) else {
            info!("[wry] window not yet registered — retrying");
            return;
        };
        let parent = &**win;

        // ── On Windows: snapshot direct children before WebView creation ──────
        #[cfg(windows)]
        let before = win32::direct_children(win32::parent_hwnd(parent));

        // ── On Linux/X11: init connection and snapshot child X windows before ─
        #[cfg(target_os = "linux")]
        let linux_state = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            x11::init_conn();
            let parent_xwin: Option<u32> = match parent.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Xcb(h))  => Some(h.window.get()),
                Ok(RawWindowHandle::Xlib(h)) => Some(h.window as u32),
                _ => None,
            };
            let before = parent_xwin.map(|pw| x11::query_children(pw)).unwrap_or_default();
            (parent_xwin, before)
        };

        // ── On macOS: snapshot subview count before WebView creation ──────────
        #[cfg(target_os = "macos")]
        let (mac_ns_view, mac_before_count) = {
            let v = appkit::parent_ns_view(parent);
            (v, appkit::snapshot_subview_count(v))
        };

        // ── Build the WebView ─────────────────────────────────────────────────
        let full_bounds = wry::Rect {
            position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
            size:     wry::dpi::LogicalSize::new(win_w as f64, win_h as f64).into(),
        };

        let single_binary = has_dist();
        let ui_url = if single_binary { PROTO_URL } else { DEV_URL };
        info!("[wry] loading UI from {ui_url}");

        let mut builder = wry::WebViewBuilder::new()
            .with_url(ui_url)
            .with_bounds(full_bounds)
            .with_ipc_handler({
                let inbound = Arc::clone(&inbound);
                move |req| ipc_handler(req, &inbound)
            });

        if single_binary {
            let dist = dist_dir();
            builder = builder.with_custom_protocol(
                PROTO.to_string(),
                move |_id, req| serve_dist(&dist, req),
            );
        }

        let webview = match builder.build_as_child(parent) {
            Ok(wv) => wv,
            Err(e) => { error!("[wry] WebView create failed: {e}"); return; }
        };

        // ── On Windows: find the new container HWND and apply viewport region ─
        #[cfg(windows)]
        {
            let hwnd_parent = win32::parent_hwnd(parent);
            *parent_hwnd_val().lock().unwrap() = hwnd_parent;
            let after       = win32::direct_children(hwnd_parent);
            let new_hwnd    = after.into_iter().find(|h| !before.contains(h));
            if let Some(hwnd) = new_hwnd {
                *container_hwnd_val().lock().unwrap() = hwnd; // HWND = isize in windows-sys
                let sf = 1.0f32; // scale_factor not available here; approximate
                let (vx, vy, vw, vh) = approx_vp_phys(win_w, win_h, sf);
                win32::apply_region(hwnd, win_w, win_h, vx, vy, vw, vh);
                info!("[wry] container HWND {hwnd} — initial SetWindowRgn applied");
            } else {
                warn!("[wry] could not find container HWND — SetWindowRgn skipped");
            }
        }

        // ── On Linux/X11: find new child X window and apply XShape region ────
        #[cfg(target_os = "linux")]
        {
            let (parent_xwin, before) = linux_state;
            if let Some(pxwin) = parent_xwin {
                let after = x11::query_children(pxwin);
                if let Some(xwin) = after.into_iter().find(|w| !before.contains(w)) {
                    *container_xwin_val().lock().unwrap() = xwin;
                    let (vx, vy, vw, vh) = approx_vp_phys(win_w, win_h, 1.0);
                    x11::apply_region(xwin, win_w, win_h, vx, vy, vw, vh);
                    info!("[wry] container X window {xwin:#010x} — initial XShape applied");
                } else {
                    warn!("[wry] could not find container X window — XShape skipped");
                }
            }
        }

        // ── On macOS: find the new NSView container and configure it ─────────
        #[cfg(target_os = "macos")]
        {
            appkit::detect_container(mac_ns_view, mac_before_count);
            let sf = 1.0f32; // approximate; refined once React sends exact bounds
            let (vx, vy, vw, vh) = approx_vp_phys(win_w, win_h, sf);
            appkit::apply_region(win_w, win_h, vx, vy, vw, vh, sf);
            info!("[wry] macOS container NSView — initial CAShapeLayer mask applied");
        }

        EDITOR_WV.with_borrow_mut(|slot| *slot = Some(webview));
        info!("[wry] editor WebView attached ({}×{} logical)", win_w, win_h);
        attached = true;
    });

    if attached {
        world.insert_non_send_resource(EditorWvMarker);
        world.insert_resource(WryEditorReady);
    }
}

/// PostUpdate system: drain outbound snapshots and push to the WebView.
/// NonSend param forces execution on the main thread (thread_local access).
pub fn push_snapshot_to_webview(
    _main:  NonSend<EditorWvMarker>,
    bridge: Res<BevyBridgeResource>,
) {
    // Keep only the latest snapshot.
    let latest = {
        let mut q = bridge.0.outbound.lock().unwrap();
        let mut snap = None;
        while let Some(s) = q.pop_front() { snap = Some(s); }
        snap
    };
    if let Some(snapshot) = latest {
        if let Ok(json) = serde_json::to_string(&snapshot) {
            // Use JSON.parse so we avoid double-escaping.
            let safe = json.replace('\\', "\\\\").replace('`', "\\`");
            let js   = format!("window.__xrds__?.onEditorState?.(JSON.parse(`{safe}`))");
            EDITOR_WV.with_borrow(|opt| {
                if let Some(wv) = opt.as_ref() { let _ = wv.evaluate_script(&js); }
            });
        }
    }
}

/// Update system: when Bevy detects any mouse button press (only possible in the viewport
/// hole where SetWindowRgn excludes the WebView), steal keyboard focus back from the WebView.
pub fn focus_viewport_on_click(
    _main:  NonSend<EditorWvMarker>,
    mouse:  Res<ButtonInput<MouseButton>>,
) {
    if mouse.just_pressed(MouseButton::Left)
        || mouse.just_pressed(MouseButton::Middle)
        || mouse.just_pressed(MouseButton::Right)
    {
        #[cfg(windows)]
        {
            let hwnd = *parent_hwnd_val().lock().unwrap();
            if hwnd != 0 { win32::set_focus(hwnd); }
        }
    }
}

/// Update system: drain pending GTK/GLib events so webkit2gtk can paint and process input.
/// webkit2gtk renders through the GLib main loop; without this pump the WebView shows nothing.
/// Runs every frame after WryEditorReady — registered only on Linux.
#[cfg(target_os = "linux")]
pub fn pump_gtk_events(_main: NonSend<EditorWvMarker>) {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

/// Update system: exit immediately when the user closes the window.
///
/// On Windows: exits before Bevy destroys the parent HWND, which would trigger synchronous
/// WebView2 COM teardown that tries to re-enter the winit message loop (deadlock).
///
/// On Linux: uses `libc::_exit` (direct exit_group syscall) instead of
/// `std::process::exit`. The latter runs thread-local destructors, which drops
/// `EDITOR_WV` → wry::WebView → webkit2gtk/GDK teardown outside the GTK event
/// loop → segfault. `_exit` bypasses all destructors; the OS reclaims every
/// resource cleanly.
pub fn force_exit_on_close(mut close: MessageReader<WindowCloseRequested>) {
    if close.read().next().is_some() {
        #[cfg(target_os = "linux")]
        // SAFETY: intentional immediate process termination to avoid webkit2gtk
        // segfault in its destructor when called outside the GTK event loop.
        unsafe { libc::_exit(0); }
        #[cfg(not(target_os = "linux"))]
        std::process::exit(0);
    }
}

/// Update system: drain JS responses (file dialogs) and apply pending viewport bounds.
/// NonSend param forces execution on the main thread.
pub fn drain_responses_and_viewport(
    _main:      NonSend<EditorWvMarker>,
    mut cam_q:  Query<&mut Camera, With<crate::viewport_camera::EditorCameraMarker>>,
    windows:    Query<&Window, With<PrimaryWindow>>,
    mut vp:     ResMut<ViewportRect>,
    mut stereo: ResMut<crate::viewport_camera::StereoPreviewState>,
) {
    // ── Drain JS responses ────────────────────────────────────────────────────
    let scripts: Vec<String> = pending_responses().lock().unwrap().drain(..).collect();
    EDITOR_WV.with_borrow(|opt| {
        if let Some(wv) = opt.as_ref() {
            for js in &scripts { let _ = wv.evaluate_script(js); }
        }
    });

    // ── Apply stereo preview state — must run every frame, before the viewport early-return.
    // pending_vp is only set when React's ResizeObserver fires (layout changes only);
    // most frames pending_vp is None and we would early-return without ever consuming
    // pending_stereo, making the L|R toggle appear broken.
    if let Some((enabled, ipd_m, fov_deg)) = pending_stereo().lock().unwrap().take() {
        stereo.enabled = enabled;
        stereo.ipd_m   = ipd_m;
        stereo.fov_deg = fov_deg;
    }

    // ── Apply exact viewport bounds from React ────────────────────────────────
    let Some((bx, by, bw, bh)) = pending_vp().lock().unwrap().take() else { return };

    let Ok(win) = windows.single() else { return };
    let sf = win.scale_factor();
    let pw = win.physical_width();
    let ph = win.physical_height();

    let phys_x = (bx * sf) as u32;
    let phys_y = (by * sf) as u32;
    let phys_w = (bw * sf) as u32;
    let phys_h = (bh * sf) as u32;

    for mut cam in &mut cam_q {
        if let Some(v) = &mut cam.viewport {
            v.physical_position = UVec2::new(phys_x, phys_y);
            v.physical_size     = UVec2::new(phys_w, phys_h);
        }
    }
    vp.x = bx; vp.y = by; vp.w = bw; vp.h = bh;

    #[cfg(windows)]
    {
        let hwnd_val = *container_hwnd_val().lock().unwrap();
        if hwnd_val != 0 {
            *last_region_params().lock().unwrap() = Some((pw, ph, phys_x, phys_y, phys_w, phys_h));
            win32::apply_region(hwnd_val, pw, ph, phys_x, phys_y, phys_w, phys_h);
        }
    }
    #[cfg(target_os = "linux")]
    {
        let xwin_val = *container_xwin_val().lock().unwrap();
        if xwin_val != 0 {
            *last_region_params().lock().unwrap() = Some((pw, ph, phys_x, phys_y, phys_w, phys_h));
            x11::apply_region(xwin_val, pw, ph, phys_x, phys_y, phys_w, phys_h);
        }
    }
    #[cfg(target_os = "macos")]
    {
        *appkit::last_region().lock().unwrap() =
            Some((pw, ph, phys_x, phys_y, phys_w, phys_h, sf));
        appkit::apply_region(pw, ph, phys_x, phys_y, phys_w, phys_h, sf);
    }
    let _ = (pw, ph);
}

/// Update system: resize the WebView and update camera viewport + SetWindowRgn.
/// NonSend param forces main thread.
pub fn handle_editor_resize(
    mut resized:   MessageReader<WindowResized>,
    mut camera_q:  Query<&mut Camera, With<crate::viewport_camera::EditorCameraMarker>>,
    win_query:     Query<&Window, With<PrimaryWindow>>,
    _main:         NonSend<EditorWvMarker>,
    mut vp:        ResMut<ViewportRect>,
) {
    let Some(ev) = resized.read().last() else { return };
    let Ok(win)  = win_query.single()    else { return };

    let lw = ev.width  as u32;
    let lh = ev.height as u32;
    let sf = win.scale_factor();
    let pw = win.physical_width();
    let ph = win.physical_height();

    // Resize WebView to cover the full window
    EDITOR_WV.with_borrow(|opt| {
        if let Some(wv) = opt.as_ref() {
            let _ = wv.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
                size:     wry::dpi::LogicalSize::new(lw as f64, lh as f64).into(),
            });
        }
    });

    // Approximate camera viewport — refined once React sends exact bounds
    let (phys_x, phys_y, phys_w, phys_h) = approx_vp_phys(pw, ph, sf);
    for mut cam in &mut camera_q {
        if let Some(v) = &mut cam.viewport {
            v.physical_position = UVec2::new(phys_x, phys_y);
            v.physical_size     = UVec2::new(phys_w, phys_h);
        }
    }
    vp.w = (lw.saturating_sub(LEFT_W + RIGHT_W)) as f32;
    vp.h = (lh.saturating_sub(TOP_H + BOT_H)) as f32;

    #[cfg(windows)]
    {
        let hwnd_val = *container_hwnd_val().lock().unwrap();
        if hwnd_val != 0 {
            *last_region_params().lock().unwrap() = Some((pw, ph, phys_x, phys_y, phys_w, phys_h));
            win32::apply_region(hwnd_val, pw, ph, phys_x, phys_y, phys_w, phys_h);
        }
    }
    #[cfg(target_os = "linux")]
    {
        let xwin_val = *container_xwin_val().lock().unwrap();
        if xwin_val != 0 {
            *last_region_params().lock().unwrap() = Some((pw, ph, phys_x, phys_y, phys_w, phys_h));
            x11::apply_region(xwin_val, pw, ph, phys_x, phys_y, phys_w, phys_h);
        }
    }
    #[cfg(target_os = "macos")]
    {
        *appkit::last_region().lock().unwrap() =
            Some((pw, ph, phys_x, phys_y, phys_w, phys_h, sf));
        appkit::apply_region(pw, ph, phys_x, phys_y, phys_w, phys_h, sf);
    }
}

// ---------------------------------------------------------------------------
// IPC handler (runs on main thread — same thread that created the WebView)
// ---------------------------------------------------------------------------

fn ipc_handler(req: wry::http::Request<String>, inbound: &Arc<Mutex<VecDeque<EditorCommand>>>) {
    let body = req.body();
    let Ok(msg) = serde_json::from_str::<serde_json::Value>(body) else {
        warn!("[ipc] bad JSON: {body}");
        return;
    };

    match msg["type"].as_str() {
        Some("command") => {
            match serde_json::from_value::<EditorCommand>(msg["command"].clone()) {
                Ok(cmd) => { inbound.lock().unwrap().push_back(cmd); }
                Err(e)  => { warn!("[ipc] command decode error: {e}\n  raw: {}", msg["command"]); }
            }
        }

        Some("file_dialog") => {
            let id   = msg["id"].as_str().unwrap_or("").to_string();
            let kind = msg["kind"].as_str().unwrap_or("").to_string();
            // Blocking file dialog — OK on main thread (OS modal loop handles messages)
            let result = run_file_dialog(&kind);
            let result_json = match &result {
                Some(p) => serde_json::to_string(p).unwrap_or_else(|_| "null".into()),
                None    => "null".into(),
            };
            let id_json = serde_json::to_string(&id).unwrap_or_else(|_| "\"\"".into());
            let js = format!(
                "if(window.__xrds__?.dialogs?.[{id_json}]) \
                 {{ window.__xrds__.dialogs[{id_json}]({result_json}); \
                    delete window.__xrds__.dialogs[{id_json}]; }}"
            );
            pending_responses().lock().unwrap().push(js);
        }

        Some("set_viewport_hole") => {
            // A modal overlay is opening (enabled=false) or closing (enabled=true).
            // When disabled: remove the WebView clipping region so the WebView paints
            // over the entire window, making the modal visible above the Bevy viewport.
            // When re-enabled: restore the saved region parameters.
            #[cfg(windows)]
            {
                let hwnd_val = *container_hwnd_val().lock().unwrap();
                if hwnd_val != 0 {
                    let enabled = msg["enabled"].as_bool().unwrap_or(true);
                    if enabled {
                        if let Some((pw, ph, vx, vy, vw, vh)) =
                            *last_region_params().lock().unwrap()
                        {
                            win32::apply_region(hwnd_val, pw, ph, vx, vy, vw, vh);
                        }
                    } else {
                        win32::clear_region(hwnd_val);
                    }
                }
            }
            #[cfg(target_os = "linux")]
            {
                let xwin_val = *container_xwin_val().lock().unwrap();
                if xwin_val != 0 {
                    let enabled = msg["enabled"].as_bool().unwrap_or(true);
                    if enabled {
                        if let Some((pw, ph, vx, vy, vw, vh)) =
                            *last_region_params().lock().unwrap()
                        {
                            x11::apply_region(xwin_val, pw, ph, vx, vy, vw, vh);
                        }
                    } else {
                        x11::clear_region(xwin_val);
                    }
                }
            }
            #[cfg(target_os = "macos")]
            {
                let enabled = msg["enabled"].as_bool().unwrap_or(true);
                if enabled {
                    if let Some((pw, ph, vx, vy, vw, vh, sf)) =
                        *appkit::last_region().lock().unwrap()
                    {
                        appkit::apply_region(pw, ph, vx, vy, vw, vh, sf);
                    }
                } else {
                    appkit::clear_region();
                }
            }
        }

        Some("viewport_bounds") => {
            // React reports the exact logical-px bounds of the centre viewport div.
            let x = msg["x"].as_f64().unwrap_or(0.0) as f32;
            let y = msg["y"].as_f64().unwrap_or(0.0) as f32;
            let w = msg["w"].as_f64().unwrap_or(0.0) as f32;
            let h = msg["h"].as_f64().unwrap_or(0.0) as f32;
            if w > 0.0 && h > 0.0 {
                *pending_vp().lock().unwrap() = Some((x, y, w, h));
            }
        }

        Some("stereo_preview") => {
            let enabled = msg["enabled"].as_bool().unwrap_or(false);
            let ipd_mm  = msg["ipd_mm"].as_f64().unwrap_or(63.0) as f32;
            let fov_deg = msg["fov_deg"].as_f64().unwrap_or(90.0) as f32;
            *pending_stereo().lock().unwrap() = Some((enabled, ipd_mm / 1000.0, fov_deg));
        }

        other => { warn!("[ipc] unknown message type: {:?}", other); }
    }
}

fn run_file_dialog(kind: &str) -> Option<String> {
    match kind {
        "open_scene" => rfd::FileDialog::new()
            .set_title("Open Scene")
            .add_filter("XRDS Scene", &["json"])
            .add_filter("All Files",  &["*"])
            .pick_file()
            .map(|p| p.to_string_lossy().into_owned()),
        "save_scene" => rfd::FileDialog::new()
            .set_title("Save Scene As")
            .set_file_name("scene.json")
            .add_filter("XRDS Scene", &["json"])
            .save_file()
            .map(|p| p.to_string_lossy().into_owned()),
        "import_asset" => rfd::FileDialog::new()
            .set_title("Import Asset")
            .add_filter("All Assets",  &["glb","gltf","png","jpg","jpeg","webp","ktx2","mp3","wav","ogg","flac","hdr"])
            .add_filter("3D Models",   &["glb","gltf"])
            .add_filter("Textures",    &["png","jpg","jpeg","webp","ktx2"])
            .add_filter("Audio",       &["mp3","wav","ogg","flac"])
            .add_filter("Environment", &["hdr","ktx2"])
            .add_filter("All Files",   &["*"])
            .pick_file()
            .map(|p| p.to_string_lossy().into_owned()),
        "export_glb" => rfd::FileDialog::new()
            .set_title("Export GLB")
            .set_file_name("scene.glb")
            .add_filter("GLB", &["glb"])
            .save_file()
            .map(|p| p.to_string_lossy().into_owned()),
        "export_app" => rfd::FileDialog::new()
            .set_title("Export Application — choose output folder")
            .pick_folder()
            .map(|p| p.to_string_lossy().into_owned()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// X11/Shape helpers (Linux only)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod x11 {
    use std::sync::OnceLock;
    use x11rb::connection::Connection as _;
    use x11rb::protocol::shape::{self, ConnectionExt as _};
    use x11rb::protocol::xproto::{ClipOrdering, ConnectionExt as _, Rectangle};
    use x11rb::rust_connection::RustConnection;

    fn conn() -> Option<&'static RustConnection> {
        static S: OnceLock<Option<RustConnection>> = OnceLock::new();
        S.get_or_init(|| x11rb::connect(None).ok().map(|(c, _)| c)).as_ref()
    }

    /// Eagerly initialise the X connection (called before WebView creation).
    pub fn init_conn() { let _ = conn(); }

    /// List direct children of `xwin`.  Returns empty vec on any error.
    pub fn query_children(xwin: u32) -> Vec<u32> {
        let Some(c) = conn() else { return vec![] };
        let Ok(cookie) = c.query_tree(xwin) else {
            log::warn!("[x11] query_tree send failed for {xwin:#010x}");
            return vec![];
        };
        match cookie.reply() {
            Ok(r)  => r.children,
            Err(e) => { log::warn!("[x11] query_tree reply failed: {e}"); vec![] }
        }
    }

    /// Apply the viewport-hole shape to `xwin` using XShapeCombineRectangles.
    /// Sets both ShapeBounding (visual clip) and ShapeInput (mouse event clip).
    /// All coordinates are physical pixels relative to the container window.
    pub fn apply_region(xwin: u32, win_w: u32, win_h: u32,
                        vp_x: u32, vp_y: u32, vp_w: u32, vp_h: u32) {
        let Some(c) = conn() else { return };
        let rects = frame_rects(win_w, win_h, vp_x, vp_y, vp_w, vp_h);
        if rects.is_empty() { return; }
        for kind in [shape::SK::BOUNDING, shape::SK::INPUT] {
            let _ = c.shape_rectangles(
                shape::SO::SET, kind, ClipOrdering::UNSORTED, xwin, 0, 0, &rects,
            );
        }
        let _ = c.flush();
    }

    /// Remove the shape mask so the WebView paints the full window (modal open).
    pub fn clear_region(xwin: u32) {
        let Some(c) = conn() else { return };
        // source_bitmap = 0 (NONE) resets the window shape to its full bounding box.
        for kind in [shape::SK::BOUNDING, shape::SK::INPUT] {
            let _ = c.shape_mask(shape::SO::SET, kind, xwin, 0, 0, 0u32);
        }
        let _ = c.flush();
    }

    /// Build the four frame rectangles (top bar, left sidebar, right inspector,
    /// bottom palette) that the WebView should remain visible/interactive in.
    /// The viewport hole is the gap between these four rectangles.
    fn frame_rects(win_w: u32, win_h: u32, vp_x: u32, vp_y: u32, vp_w: u32, vp_h: u32) -> Vec<Rectangle> {
        let vp_x2 = vp_x + vp_w;
        let vp_y2 = vp_y + vp_h;
        let mut v = Vec::with_capacity(4);
        // top bar (full width, above viewport)
        if vp_y > 0 {
            v.push(Rectangle { x: 0, y: 0, width: win_w as u16, height: vp_y as u16 });
        }
        // left sidebar
        if vp_x > 0 {
            v.push(Rectangle { x: 0, y: vp_y as i16, width: vp_x as u16, height: vp_h as u16 });
        }
        // right inspector
        if vp_x2 < win_w {
            v.push(Rectangle { x: vp_x2 as i16, y: vp_y as i16,
                               width: (win_w - vp_x2) as u16, height: vp_h as u16 });
        }
        // bottom palette (full width, below viewport)
        if vp_y2 < win_h {
            v.push(Rectangle { x: 0, y: vp_y2 as i16,
                               width: win_w as u16, height: (win_h - vp_y2) as u16 });
        }
        v
    }
}

// ---------------------------------------------------------------------------
// AppKit helpers (macOS only)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub mod appkit {
    use std::ffi::{c_char, c_void};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    use log::{info, warn};
    use objc2::runtime::{AnyClass, AnyObject, Sel};
    use objc2::{class, msg_send, sel};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // ── CoreGraphics / QuartzCore FFI ─────────────────────────────────────────

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGPoint { x: f64, y: f64 }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGSize  { width: f64, height: f64 }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGRect  { origin: CGPoint, size: CGSize }

    /// NSPoint and CGPoint are identical on all Apple platforms ({x:f64, y:f64}).
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct NSPoint { pub x: f64, pub y: f64 }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPathCreateMutable() -> *mut c_void;
        fn CGPathAddRect(path: *mut c_void, xf: *const c_void, rect: CGRect);
        fn CGPathRelease(path: *mut c_void);
        /// Creates a CGColor with RGBA components in the Generic RGB color space.
        /// The returned color must be released by the caller, but for a mask that
        /// lives for the process lifetime a deliberate "leak" is fine.
        fn CGColorCreateGenericRGB(r: f64, g: f64, b: f64, a: f64) -> *mut c_void;
    }

    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {}

    extern "C" {
        fn class_replaceMethod(
            cls: *const c_void,
            name: Sel,
            imp: *const c_void,
            types: *const c_char,
        ) -> *const c_void;
        /// Returns the name of a class as a C string (stable ObjC runtime API).
        fn class_getName(cls: *const c_void) -> *const c_char;
    }
    extern "C" { fn objc_msgSend(); }

    // ── Shared state ──────────────────────────────────────────────────────────

    fn container_ptr() -> &'static Mutex<usize> {
        static S: OnceLock<Mutex<usize>> = OnceLock::new();
        S.get_or_init(|| Mutex::new(0))
    }

    fn mask_layer_ptr() -> &'static Mutex<usize> {
        static S: OnceLock<Mutex<usize>> = OnceLock::new();
        S.get_or_init(|| Mutex::new(0))
    }

    pub fn last_region() -> &'static Mutex<Option<(u32, u32, u32, u32, u32, u32, f32)>> {
        static S: OnceLock<Mutex<Option<(u32, u32, u32, u32, u32, u32, f32)>>> = OnceLock::new();
        S.get_or_init(|| Mutex::new(None))
    }

    fn vp_hole() -> &'static Mutex<(f64, f64, f64, f64, f64)> {
        static S: OnceLock<Mutex<(f64, f64, f64, f64, f64)>> = OnceLock::new();
        // (vx, vy_topdown, vw, vh, lh) — vy_topdown is y from top (screen coords);
        // lh is logical window height needed to convert to AppKit's bottom-up Y.
        S.get_or_init(|| Mutex::new((0.0, 0.0, 0.0, 0.0, 0.0)))
    }

    static ORIG_HIT_TEST_IMP: AtomicUsize = AtomicUsize::new(0);

    // ── Public interface ──────────────────────────────────────────────────────

    pub fn parent_ns_view(win: &impl HasWindowHandle) -> *mut AnyObject {
        match win.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::AppKit(h)) => h.ns_view.as_ptr() as *mut AnyObject,
            _ => std::ptr::null_mut(),
        }
    }

    pub fn snapshot_subview_count(parent: *mut AnyObject) -> usize {
        unsafe {
            let subs: *mut AnyObject = msg_send![parent, subviews];
            msg_send![subs, count]
        }
    }

    pub fn detect_container(parent: *mut AnyObject, before_count: usize) {
        unsafe {
            let subs: *mut AnyObject = msg_send![parent, subviews];
            let count: usize = msg_send![subs, count];
            if count <= before_count {
                warn!("[appkit] no new subview after WebView build — skipping");
                return;
            }
            let view: *mut AnyObject = msg_send![subs, lastObject];
            if view.is_null() { return; }
            *container_ptr().lock().unwrap() = view as usize;

            let _: () = msg_send![view, setWantsLayer: true];

            // Log the ObjC class name via class_getName (stable runtime API).
            // msg_send![cls, name] does NOT work — AnyClass has no `name` selector.
            let cls_name = class_getName((*view).class() as *const _ as *const c_void);
            let cls_str = std::ffi::CStr::from_ptr(cls_name).to_string_lossy();
            info!("[appkit] container view class: {cls_str}");

            let shape: *mut AnyObject = msg_send![class!(CAShapeLayer), new];
            let shape: *mut AnyObject = msg_send![shape, retain];

            // CAShapeLayer.new starts with fillColor = nil (transparent).
            // A mask layer clips using its alpha channel, so fillColor must be opaque.
            // We use white (1,1,1,1) — the colour does not matter, only the alpha.
            // Must use raw objc_msgSend: msg_send! panics in objc2 0.5 when the
            // argument encoding (*mut c_void) doesn't match CGColorRef.
            let opaque_color = CGColorCreateGenericRGB(1.0, 1.0, 1.0, 1.0);
            {
                type SetFillColorFn = unsafe extern "C" fn(*mut AnyObject, Sel, *mut c_void);
                let f: SetFillColorFn = std::mem::transmute(objc_msgSend as *const ());
                f(shape, sel!(setFillColor:), opaque_color);
            }

            *mask_layer_ptr().lock().unwrap() = shape as usize;

            install_hittest_override(view);
            info!("[appkit] container NSView {:p} configured with opaque CAShapeLayer mask", view);
        }
    }

    /// Apply the viewport hole mask.  All dimensions are physical pixels; `sf`
    /// converts to CoreGraphics logical points.
    pub fn apply_region(win_w: u32, win_h: u32,
                        vp_x: u32, vp_y: u32, vp_w: u32, vp_h: u32,
                        sf: f32) {
        let container = *container_ptr().lock().unwrap() as *mut AnyObject;
        let shape     = *mask_layer_ptr().lock().unwrap() as *mut AnyObject;
        if container.is_null() || shape.is_null() { return; }

        let sf = sf as f64;
        let lw = win_w as f64 / sf;
        let lh = win_h as f64 / sf;
        let vx = vp_x as f64 / sf;
        let vy = vp_y as f64 / sf;
        let vw = vp_w as f64 / sf;
        let vh = vp_h as f64 / sf;

        *vp_hole().lock().unwrap() = (vx, vy, vw, vh, lh);

        // WKWebView is a flipped NSView (isFlipped = YES), which means its backing
        // CALayer has geometryFlipped = YES: layer y=0 is at the TOP, y increases
        // downward.  CoreGraphics default (y=0 at bottom) does NOT apply here.
        // Use top-down coordinates directly — no Y-flip needed.
        let rects = [
            CGRect { origin: CGPoint { x: 0.0,    y: 0.0    }, size: CGSize { width: lw,          height: vy       } }, // top bar
            CGRect { origin: CGPoint { x: 0.0,    y: vy + vh }, size: CGSize { width: lw,          height: lh-vy-vh } }, // bottom panel
            CGRect { origin: CGPoint { x: 0.0,    y: vy     }, size: CGSize { width: vx,          height: vh       } }, // left panel
            CGRect { origin: CGPoint { x: vx + vw, y: vy    }, size: CGSize { width: lw - vx - vw, height: vh      } }, // right panel
        ];

        unsafe {
            let path = CGPathCreateMutable();
            for r in &rects {
                if r.size.width > 0.0 && r.size.height > 0.0 {
                    CGPathAddRect(path, std::ptr::null(), *r);
                }
            }
            type SetPathFn = unsafe extern "C" fn(*mut AnyObject, Sel, *const c_void);
            let set_path: SetPathFn = std::mem::transmute(objc_msgSend as *const ());
            set_path(shape, sel!(setPath:), path as *const c_void);
            CGPathRelease(path);

            let layer: *mut AnyObject = msg_send![container, layer];
            type SetMaskFn = unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject);
            let set_mask: SetMaskFn = std::mem::transmute(objc_msgSend as *const ());
            set_mask(layer, sel!(setMask:), shape);

            let _: () = msg_send![container, setNeedsDisplay: true];
        }
    }

    pub fn clear_region() {
        let container = *container_ptr().lock().unwrap() as *mut AnyObject;
        if container.is_null() { return; }
        unsafe {
            let layer: *mut AnyObject = msg_send![container, layer];
            type SetMaskFn = unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject);
            let set_mask: SetMaskFn = std::mem::transmute(objc_msgSend as *const ());
            set_mask(layer, sel!(setMask:), std::ptr::null_mut());

            let _: () = msg_send![container, setNeedsDisplay: true];
        }
        *vp_hole().lock().unwrap() = (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    // ── hitTest: override ─────────────────────────────────────────────────────

    fn install_hittest_override(view: *mut AnyObject) {
        unsafe {
            let cls = (*view).class() as *const AnyClass as *const c_void;
            let enc = b"@32@0:8{CGPoint=dd}16\0";
            let orig = class_replaceMethod(
                cls, sel!(hitTest:),
                hit_test_imp as *const c_void,
                enc.as_ptr() as *const c_char,
            );
            ORIG_HIT_TEST_IMP.store(orig as usize, Ordering::SeqCst);
        }
    }

    unsafe extern "C" fn hit_test_imp(
        this: *mut AnyObject,
        cmd: Sel,
        point: NSPoint,
    ) -> *mut AnyObject {
        let (vx, vy_top, vw, vh, _lh) = *vp_hole().lock().unwrap();
        if vw > 0.0 {
            // winit's NSView has isFlipped = YES, so the superview's coordinate
            // system is top-down (y=0 at top), matching WKWebView's own layout.
            // WKWebView is placed at origin (0,0) = top-left of the superview,
            // so point.y maps directly to vy_top — no Y-flip needed.
            if point.x >= vx && point.x < vx + vw
                && point.y >= vy_top && point.y < vy_top + vh
            {
                return std::ptr::null_mut(); // hole: let click fall through to Bevy
            }
        }
        let orig = ORIG_HIT_TEST_IMP.load(Ordering::Relaxed);
        if orig == 0 { return this; }
        type HitTestFn = unsafe extern "C" fn(*mut AnyObject, Sel, NSPoint) -> *mut AnyObject;
        let f: HitTestFn = std::mem::transmute(orig);
        f(this, cmd, point)
    }
}

// ---------------------------------------------------------------------------
// Win32 helpers (Windows only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    // windows-sys 0.52: HWND = isize, HRGN = isize (type aliases, not newtypes)
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindow, GW_CHILD, GW_HWNDNEXT};
    use windows_sys::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, HRGN, RGN_DIFF};

    // SetWindowRgn and SetFocus have cross-module feature dependencies in windows-sys 0.52;
    // both live in user32.dll already linked via Win32_UI_WindowsAndMessaging.
    extern "system" {
        fn SetWindowRgn(hwnd: HWND, hrgn: HRGN, bredraw: i32) -> i32;
        fn SetFocus(hwnd: HWND) -> HWND;
    }
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn parent_hwnd(win: &impl HasWindowHandle) -> HWND {
        match win.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            _ => 0,
        }
    }

    pub fn set_focus(hwnd: HWND) {
        unsafe { SetFocus(hwnd); }
    }

    pub fn direct_children(parent: HWND) -> Vec<HWND> {
        let mut result = Vec::new();
        let mut child = unsafe { GetWindow(parent, GW_CHILD) };
        while child != 0 {
            result.push(child);
            child = unsafe { GetWindow(child, GW_HWNDNEXT) };
        }
        result
    }

    /// Removes the WebView container's clipping region so it paints the entire window.
    /// Call this when a modal overlay needs to appear over the Bevy viewport area.
    /// Restore with `apply_region` when the modal closes.
    pub fn clear_region(hwnd: HWND) {
        // Passing null HRGN resets the window to its full rectangular area.
        unsafe { SetWindowRgn(hwnd, 0, 1); }
    }

    /// Sets the WebView container's visible region to the full window minus the
    /// viewport hole so Bevy's DXGI swap-chain shows through.
    /// All coordinates are in physical (device) pixels relative to the container.
    pub fn apply_region(hwnd: HWND, win_w: u32, win_h: u32,
                        vp_x: u32, vp_y: u32, vp_w: u32, vp_h: u32) {
        let (iw, ih) = (win_w as i32, win_h as i32);
        let (ix, iy, ix2, iy2) = (
            vp_x as i32, vp_y as i32,
            (vp_x + vp_w) as i32, (vp_y + vp_h) as i32,
        );
        unsafe {
            let full  = CreateRectRgn(0,  0,  iw,  ih);
            let hole  = CreateRectRgn(ix, iy, ix2, iy2);
            let frame = CreateRectRgn(0,  0,  iw,  ih);
            CombineRgn(frame, full, hole, RGN_DIFF);
            // SetWindowRgn takes ownership of `frame`; do NOT delete it.
            SetWindowRgn(hwnd, frame, 1); // 1 = TRUE (redraw)
            DeleteObject(full  as _);
            DeleteObject(hole  as _);
        }
    }
}
