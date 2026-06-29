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
| Windows       | `SetWindowRgn` (Win32 GDI)                | done                    |
| Linux/X11     | `XShapeCombineRectangles` (X11 Shape ext) | todo                    |
| Linux/Wayland | `wl_surface.set_input_region` (wl_region) | todo (see Wayland note) |
| macOS         | `CAShapeLayer` mask on `NSView`         | todo                    |

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

- [ ] **Handle extraction** — in `try_attach_wry_editor`, match `RawWindowHandle::Xcb { window, connection }` or `RawWindowHandle::Xlib { window, display }` from Bevy's winit window. Store display pointer + parent X window ID in an `OnceLock<Mutex<...>>`.
- [ ] **Child window detection** — after `WebViewBuilder::new().build()`, call `XQueryTree(dpy, parent_xwin)` to get child list. Diff before/after (mirror of the Windows code that diffs `direct_children(hwnd_parent)`) to find the newly created WebView container X window. Store in `container_xwin_val()` OnceLock.
- [ ] **`apply_region(xwin, win_w, win_h, vp_x, vp_y, vp_w, vp_h)`** — build four `XRectangle`s covering the frame area (top bar, left panel, right panel, bottom panel), excluding the viewport hole. Call:

  ```text
  XShapeCombineRectangles(dpy, xwin, ShapeBounding, rects, ShapeSet, YSorted)
  XShapeCombineRectangles(dpy, xwin, ShapeInput,    rects, ShapeSet, YSorted)
  ```

  `ShapeBounding` clips rendering; `ShapeInput` clips mouse event delivery (clicks in the hole go to Bevy instead of the WebView).
- [ ] **`clear_region(xwin)`** — pass a single full-window rectangle to both `ShapeBounding` and `ShapeInput` (equivalent to `SetWindowRgn(hwnd, NULL, TRUE)`).
- [ ] **Wire into existing hooks** — add `#[cfg(target_os = "linux")]` blocks alongside the existing `#[cfg(windows)]` blocks in:

  - `drain_responses_and_viewport` (viewport bounds IPC)
  - `handle_editor_resize` (window resize)
  - `set_viewport_hole` IPC handler (modal open/close)
- [ ] **X11 display handle** — `XShapeCombineRectangles` needs the `Display*`. Extract it from `RawDisplayHandle::Xlib { display }` (available via `winit::window::Window::raw_display_handle()`).

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

- [ ] **Handle extraction** — match `RawWindowHandle::AppKit { ns_view }` from Bevy's winit window. This is the Bevy window's root `NSView`.
- [ ] **Child view detection** — after `WebViewBuilder::new().build()`, enumerate `ns_view.subviews()` to find the newly added `WKWebView` container view. Store its pointer in `container_nsview_val()` OnceLock.
- [ ] **Enable layer backing** on the container view: `container_view.setWantsLayer(YES)`.
- [ ] **`apply_region(ns_view, win_w, win_h, vp_x, vp_y, vp_w, vp_h)`**:

  - Build a `CGMutablePath` that covers the four frame rectangles (the area to KEEP visible).
  - **Y-flip**: macOS NSView uses bottom-left origin; convert: `mac_vp_y = win_h - vp_y - vp_h`.
  - Create a `CAShapeLayer` with that path as `shapeLayer.path`.
  - Set `container_view.layer().setMask(shapeLayer)`.
  - Store the `CAShapeLayer` in an `OnceLock<Mutex<Option<*mut CAShapeLayer>>>` so subsequent calls update the existing layer's path instead of recreating.
- [ ] **`clear_region(ns_view)`** — set `container_view.layer().setMask(nil)`.
- [ ] **Mouse passthrough (hit-testing)**:

  - `CALayer.mask` clips rendering but mouse events are still delivered based on the NSView frame, not the mask shape. Clicks in the viewport hole still go to the WebView.
  - Fix: override `hitTest:` on the container view to return `nil` when the click point is inside the viewport rectangle. With `objc2` this uses the `declare_class!` macro (~30 lines).
  - Alternative: place a transparent `NSView` overlay on top of the viewport hole in the Bevy window and forward events — more complex and can interfere with Bevy's input.
- [ ] **Thread safety** — all `CALayer` / `NSView` calls must run on the main thread. Bevy exclusive systems and `try_attach_wry_editor` already run on the main thread. Resize handlers must dispatch via a main-thread channel if called from a background thread.
- [ ] **Wire into existing hooks** — same three call sites as Linux: `drain_responses_and_viewport`, `handle_editor_resize`, `set_viewport_hole` IPC.

### Open questions

- `objc2-quartz-core` version compatibility with the `objc2-app-kit` version wry uses — verify in Cargo.lock after adding.
- Whether `declare_class!` for the `hitTest:` override works without a separate `.m` file with current objc2 0.5 API (it should, but needs a test build).

---

## Shared refactor needed in `wry_overlay.rs`

The current code has Windows-only `OnceLock` globals and all region logic inside
`#[cfg(windows)]` blocks. Before adding Linux/macOS support, extract the platform-neutral
skeleton:

- [ ] Define a `platform` module gated per OS (`mod win32`, `mod x11`, `mod appkit`).
- [ ] Each module exposes the same interface:

  ```rust
  pub fn init_region_state();
  pub fn detect_and_store_container(world: &mut World, webview: &WebView);
  pub fn apply_region(win_w: u32, win_h: u32, vp_x: u32, vp_y: u32, vp_w: u32, vp_h: u32);
  pub fn clear_region();
  ```
- [ ] The three call sites in `wry_overlay.rs` call `platform::apply_region(...)` with no cfg guards — the dispatch is inside the module.

---

## Testing checklist

- [ ] Linux/X11: UI panels render, hole is transparent, Bevy scene visible, mouse clicks in hole reach Bevy.
- [ ] Linux/X11: modal dialogs (APK export, keyboard shortcuts) clear the hole correctly.
- [ ] Linux/X11: window resize updates region.
- [ ] Wayland: decision made and documented (force X11 or Option A fallback).
- [ ] macOS: UI panels render, viewport hole visible, visual mask correct.
- [ ] macOS: Y-flip coordinates verified (hole aligns with actual Bevy viewport).
- [ ] macOS: mouse clicks in hole reach Bevy (`hitTest:` override works).
- [ ] macOS: modal overlay clears mask.
- [ ] macOS: window resize updates `CAShapeLayer` path.
- [ ] All three platforms: no regression on Windows build.
