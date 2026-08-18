# Cross-Platform Window Region — Implementation Plan

Goal: replicate the Windows `SetWindowRgn` "viewport hole" on Linux and macOS so the
editor UI works on all three platforms.

Current state: all region code is `#[cfg(windows)]`; Linux and macOS builds compile
but show no UI because the WebView paints over the entire Bevy window with no hole.

---

## Approach chosen: Option B — platform-native region APIs

Each platform has a native API equivalent to Win32's `SetWindowRgn`:

| Platform      | API                                         | Status                  |
| ------------- | ------------------------------------------- | ----------------------- |
| Windows       | `SetWindowRgn` (Win32 GDI)                  | done                    |
| Linux/X11     | `XShapeCombineRectangles` (X11 Shape ext)   | done                    |
| Linux/Wayland | `wl_surface.set_input_region` (wl_region)   | n/a — X11 forced        |
| macOS         | `CAShapeLayer` mask on `NSView`             | done                    |

---

## Linux — X11 path

### Feasibility

**Viable.** `XShapeCombineRectangles` (X11 SHAPE extension) is the direct analogue of
`SetWindowRgn`. The wry WebView on X11 creates a child X window inside Bevy's X window,
so we can apply a shape mask to that child window the same way Windows code finds and
masks the container HWND.

Crates already in the dep tree via wry 0.55: `x11-dl`.
New crate needed: `x11rb` with `shape` feature (safer than raw `x11-dl` bindings).

### New crate

```toml
[target.'cfg(target_os = "linux")'.dependencies]
x11rb = { version = "0.13", features = ["shape"] }
```

### Implementation steps

- [x] **Handle extraction** — `try_attach_wry_editor` matches `RawWindowHandle::Xcb` / `RawWindowHandle::Xlib` to get the parent X window ID.
- [x] **Child window detection** — `x11::query_children` snapshots children before and after `build_as_child`; the new child is the container X window stored in `container_xwin_val()`.
- [x] **`apply_region`** — builds four `x11rb::Rectangle`s (top bar, left sidebar, right inspector, bottom palette) and calls `shape_rectangles` with both `ShapeBounding` and `ShapeInput`.
- [x] **`clear_region`** — calls `shape_mask` with source bitmap `0` (NONE) to reset both shape kinds to the full bounding box.
- [x] **Wire into existing hooks** — `#[cfg(target_os = "linux")]` blocks added alongside `#[cfg(windows)]` in `drain_responses_and_viewport`, `handle_editor_resize`, and the `set_viewport_hole` IPC handler.
- [x] **X11 connection** — managed by `x11rb::connect(None)` in a `OnceLock`; no raw `Display*` needed.
- [x] **GTK event pump** — `pump_gtk_events` system drains `gtk::main_iteration_do(false)` every Bevy frame so webkit2gtk can paint and process input.

### Runtime fixes required on Linux

Three issues were discovered and fixed during bring-up:

**1 — libpthread symbol lookup error (VS Code Snap)**
VS Code (Snap, base: core20) sets `GDK_PIXBUF_MODULEDIR`, `GIO_MODULE_DIR`, `GTK_PATH`, etc. to snap-packaged loaders compiled for glibc 2.31. Those loaders embed `DT_RPATH=/snap/core20/current/lib/x86_64-linux-gnu`, pulling in a stub `libpthread.so.0` that requires `__libc_pthread_init` — removed in glibc 2.34+.
Fix: `.cargo/config.toml` runner unsets all six snap GTK env vars before launching.

**2 — GLXBadWindow panic + wgpu swap chain stutter (NVIDIA + X11)**
webkit2gtk's DMABUF renderer calls `glXDestroyWindow` during GL init on NVIDIA hardware, leaving a `GLXBadWindow` error in the shared Xlib error queue. winit's `XSetICFocus` handler picks it up and panics. The same renderer's software-paint fallback floods the parent window with `XPutImage` calls, causing NVIDIA's Vulkan WSI to mark the wgpu swap chain out-of-date every frame.
Fix: `WEBKIT_DISABLE_DMABUF_RENDERER=1` — webkit falls back to EGL compositing on its own child surface, avoiding both GLX operations and direct X11 painting on the parent.

**3 — Segfault on close**
`std::process::exit(0)` runs Rust thread-local destructors, which drops `EDITOR_WV` → `wry::WebView` → webkit2gtk/GDK teardown outside the GTK event loop → segfault.
Fix: `libc::_exit(0)` — direct `exit_group` syscall, bypasses all destructors; the OS reclaims resources.

Both fixes are encoded in `.cargo/config.toml` (runner) and `run-editor.sh` (release launcher).

### Known limitations

- **Window resize stutter**: dragging the window border causes repeated Vulkan `SurfaceError::Lost` — one per `ConfigureNotify` event. Bevy recovers by dropping the affected frames. This is a Vulkan/X11 limitation; prefer keyboard-based window snapping during recording or demos.

### Wayland note

wry on Wayland produces `RawWindowHandle::Wayland { surface: wl_surface }` — a Wayland
surface handle, not an X window ID. `XShape` does not apply to Wayland surfaces. Options:

1. **Force X11 for editor sessions**: set `WINIT_UNIX_BACKEND=x11` before launching the editor binary. Wayland compositors run XWayland, so the editor runs as an X client. No code changes needed, but the user loses native Wayland.
2. **`wl_region` input masking** (visual compositing still requires Option A): `wl_compositor.create_region()` → add rectangles → `wl_surface.set_input_region(region)`. Makes clicks pass through to Bevy in the hole but **does not clip the WebView's visual rendering** — the WebView still paints over Bevy. Full visual fix on Wayland requires CSS transparency.
3. **Runtime detect + fallback**: check handle type; use XShape on X11 and fall back to CSS transparency on Wayland.

**Decision needed: require X11 for Linux editor (simplest) or support Wayland natively?**

Decision: X11

---

## macOS — CAShapeLayer mask

### macOS feasibility

**Viable but more complex.** macOS does not have an API named "window region", but
`CALayer.mask` achieves the same effect: assign a `CAShapeLayer` with the frame path as
a mask on the wry WebView's `NSView`. The masked layer clips rendering to the shape,
making the hole transparent. Mouse click passthrough requires an additional `NSView`
`hitTest:` override.

Crates already in tree via wry 0.55: `objc2`, `objc2-app-kit`, `objc2-foundation`.
New crate needed: `objc2-quartz-core` for `CAShapeLayer` / `CALayer`.

### macOS new crates

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2-app-kit    = { version = "0.2", features = ["NSView", "NSWindow"] }
objc2-quartz-core = { version = "0.2", features = ["CALayer", "CAShapeLayer"] }
objc2-foundation  = { version = "0.2", features = ["NSArray"] }
```

### macOS implementation steps

- [x] **Handle extraction** — `appkit::parent_ns_view()` matches `RawWindowHandle::AppKit { ns_view }`.
- [x] **Child view detection** — `appkit::snapshot_subview_count()` before / `appkit::detect_container()` after: takes `lastObject` of the subview diff.
- [x] **Enable layer backing** — `setWantsLayer:true` in `detect_container`.
- [x] **`apply_region(win_w, win_h, vp_x, vp_y, vp_w, vp_h, sf)`**:
  - Builds four `CGRect`s in CoreGraphics coords (Y-flipped: `cg_vy = lh - vy - vh`).
  - Updates a persistent `CAShapeLayer` path via `setPath:` (transmuted `objc_msgSend`).
  - Sets `container.layer().setMask(shapeLayer)`.
- [x] **`clear_region()`** — `setMask: nil`, clears `vp_hole` so hitTest passthrough is disabled.
- [x] **Mouse passthrough** — `class_replaceMethod` on the WKWebView container class replaces `hitTest:` with `hit_test_imp`, which returns nil for viewport-area points and calls the saved original IMP otherwise. Approach used: `class_replaceMethod` FFI (saves original IMP) instead of `declare_class!`, avoids objc2 trait-bound issues with `CGPoint` arguments.
- [x] **Thread safety** — all calls are in Bevy main-thread systems (`NonSend<EditorWvMarker>` or exclusive world systems).
- [x] **Wire into existing hooks** — `drain_responses_and_viewport`, `handle_editor_resize`, `set_viewport_hole` IPC handler.
- [x] **New dep** — `objc2 = "0.5"` added to `[target.'cfg(target_os = "macos")'.dependencies]`.

### Open questions (resolved)

- `objc2 0.5` is already in Cargo.lock via wry; no version conflict.
- hitTest: override implemented via `class_replaceMethod` FFI instead of `declare_class!` — avoids `EncodeArgument` constraints on `CGPoint`.
  - **Coordinate system**: winit's `NSView` has `isFlipped = YES` (y=0 at top), so the `hitTest:`
    point is in top-down coordinates — same as `vy_top`. **No Y-flip needed.** An earlier version
    applied `appkit_y = lh - vy_top` (assuming y=0 at bottom), shifting the passthrough zone 108 px
    below the visual hole. Fixed by comparing `point.y` directly against `vy_top..vy_top+vh`.
- **`CAShapeLayer.fillColor` must be set explicitly** — `.new` starts with `fillColor = nil` (transparent),
  making the entire mask invisible. Fixed by calling `CGColorCreateGenericRGB(1,1,1,1)` via raw
  `transmute(objc_msgSend)` (same pattern as `setPath:`/`setMask:`; `msg_send!` panics on `*mut c_void`).
- **Class name logging**: `msg_send![cls, name]` panics in objc2 0.5 (no `name` selector on `AnyClass`).
  Use `class_getName()` C-runtime API instead.
- **CALayer coordinate system**: `WKWebView` is a flipped NSView (`isFlipped = YES`), so its backing
  `CALayer` has `geometryFlipped = YES` — layer y=0 is at the **top**, not the CoreGraphics default
  bottom. Mask path rects must use top-down coordinates. The original Y-flip (`cg_vy = lh - vy - vh`)
  caused the hole to be 108 px too low, bleeding into the palette. **Fixed** by removing the CG flip
  and using `vy` directly as the top-of-hole coordinate.
- **`msg_send![cls, name]` for class name** → replaced with `class_getName()` (stable C runtime API).
- `objc2 0.5` and `0.6` coexist safely because the code only uses raw pointer casts, not typed objects
  across versions.
- **Blank WebView Rendering**: Backing layer changes on macOS (like `setMask:`) do not trigger automatic NSView redraws. The WebView remained invisible until Web Inspector was opened. **Fixed** by explicitly sending `setNeedsDisplay: true` to the container view during both mask allocation (`apply_region`) and deallocation (`clear_region`).
- **Mac Keyboard Drift**: Key release events (`keyUp`) are swallowed by `WKWebView` when input focus is directed to a panel. When returning the cursor to the Bevy viewport, Bevy still registers the key as held down. **Fixed** by resetting movement keys (`WASD`, `E`, `Q`) in `orbit_camera_system` (`viewport_camera.rs`) when a cursor re-entry transition (`prev_in_vp = false` -> `in_vp = true`) is detected.

### Run requirement

> [!IMPORTANT]
> `cargo run -p xrds-editor` **must be launched from the workspace root** (`DeviceSDK/`).
> Running from `apps/xrds-editor/` causes the asset path resolution to fail because
> `CARGO_MANIFEST_DIR` is baked in at compile time but relative traversal depends on CWD for
> some Bevy internal paths. Always use `cargo run -p xrds-editor` from `DeviceSDK/`.
---

## Shared refactor in `wry_overlay.rs`

The clean module-extraction approach (separate `mod win32`, `mod x11`, `mod appkit` with a
shared interface) was considered but not implemented. Instead, inline `#[cfg(target_os = "linux")]`
blocks were added alongside the existing `#[cfg(windows)]` blocks at each call site.
This is adequate for two platforms; revisit if macOS is added and the duplication becomes
unwieldy.

---

## Testing checklist

- [x] Linux/X11: UI panels render, hole is transparent, Bevy scene visible, mouse clicks in hole reach Bevy.
- [x] Linux/X11: modal dialogs (APK export, keyboard shortcuts) clear the hole correctly.
- [x] Linux/X11: window resize updates region.
- [x] Wayland: decision made — X11 forced via `WINIT_UNIX_BACKEND=x11` in launcher.
- [x] macOS: UI panels render, viewport hole visible, visual mask correct.
- [x] macOS: Y-flip coordinates verified (hole aligns with actual Bevy viewport).
- [x] macOS: mouse clicks in hole reach Bevy (`hitTest:` override works).
- [x] macOS: modal overlay clears mask.
- [x] macOS: window resize updates `CAShapeLayer` path.
- [ ] All three platforms: no regression on Windows build.
