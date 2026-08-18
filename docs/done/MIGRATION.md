# Migration Plan: xrds-editor-tauri → wry + Bevy native rendering

## Background

The current `xrds-editor-tauri` runs Bevy in a **background thread** with Tauri owning the OS window.
Bevy renders to an offscreen texture, which is JPEG-encoded every 16 ms and piped to a `<canvas>`
element via Tauri events. This imposes encode latency, prevents native Bevy input, and makes
gizmo/selection interaction indirect (all mouse events are forwarded through IPC).

The migration replaces Tauri with a **wry child WebView** embedded into Bevy's native window —
the same pattern proven in the wry prototype. After migration:

- Bevy owns the OS window and renders natively (no JPEG capture)
- The React UI lives in a single wry child WebView, full-window
- A `SetWindowRgn` cut-out exposes the Bevy DXGI surface in the centre viewport
- All JPEG encode/decode infrastructure is removed
- Viewport mouse events go directly to Bevy's input system (no forwarding)
- IPC shrinks to: `evaluate_script` for state updates, `ipc.postMessage` for commands + file dialogs

---

## What stays unchanged

| Path | Notes |
| --- | --- |
| `src-tauri/src/bevy_scene.rs` | Remove `hide_bevy_window`, `ViewportCapturePlugin`, `setup_render_target` |
| `src-tauri/src/viewport_camera.rs` | Rewrite orbit to direct Bevy events; remove `ViewportInputState` |
| `src-tauri/src/viewport_gizmo*.rs` | Keep all gizmo systems; update input source |
| `src-tauri/src/viewport_player.rs` | Keep; update click source to Bevy events |
| `src-tauri/src/viewport_selection.rs` | Keep; update click source to Bevy events |
| `src-tauri/src/hierarchy.rs` … `toolbar.rs` | All `apply_*` and `build_*` functions — **zero changes** |
| `src-tauri/src/editor_state.rs` | Keep entirely |
| `src-tauri/src/bridge.rs` | Keep `EditorBridge`, `EditorSnapshot`, most `EditorCommand` variants |
| All React components | `Hierarchy`, `Inspector`, `Palette`, `Toolbar`, `Menubar`, `PlayerPanel` — zero changes |
| `src/types/bridge.ts` | Keep; remove viewport mouse command variants |

---

## What is deleted

| Path | Reason |
| --- | --- |
| `src-tauri/src/viewport_capture.rs` | JPEG pipeline replaced by native render |
| `bevy_bridge.rs::spawn_snapshot_emitter` | Tauri async emitter replaced by `evaluate_script` |
| `bevy_bridge.rs::encode_frame_jpeg` | JPEG encoding not needed |
| `src/components/ViewportCanvas.tsx` | Canvas replaced by a native Bevy viewport |
| `EditorCommand::ViewportMouseButton/Move/Scroll` variants | Bevy reads input directly |
| `viewport_camera.rs::ViewportInputState` | Replaced by direct Bevy event reading |
| `bevy_scene.rs::hide_bevy_window()` call | Bevy window is now the main window |

---

## What changes

### Rust seams

**`src-tauri/Cargo.toml`**

- Remove `tauri`, `tauri-build`
- Add `wry = "0.55"`, `windows = { version = "...", features = ["Win32_Graphics_Gdi", "Win32_UI_WindowsAndMessaging"] }`
- Keep `rfd`, `image`, `base64`, all xrds-* workspace crates

**`src-tauri/src/lib.rs`** (rewrite)

- Remove `tauri::Builder`
- `run()` creates a Bevy `App`, adds all Bevy systems, then attaches a wry child WebView
- Spawns the wry child WebView (full-window) serving the React bundle
- Calls `SetWindowRgn` to carve the viewport hole immediately after WebView creation
- Registers IPC handler: `{ type: "command", command }` → push to `EditorBridge::inbound`
- Registers IPC handler: `{ type: "file_dialog", id, kind }` → `rfd` on blocking thread → `evaluate_script` response

**`src-tauri/src/bevy_bridge.rs`**

- Remove `spawn_snapshot_emitter` and `encode_frame_jpeg`
- `broadcast_editor_snapshot_system` already pushes to `outbound` queue — keep as-is
- Add new Bevy system `push_snapshot_to_webview`: drains `outbound`, calls
  `webview.evaluate_script("window.__xrds__.onEditorState(JSON.stringify(snap))")` via a
  `NonSendMut<WryWebViewHandle>` resource

**`src-tauri/src/viewport_camera.rs`**

- Remove `ViewportInputState` resource and all its update logic
- Rewrite `orbit_camera_system` to read `ButtonInput<MouseButton>`, `EventReader<MouseMotion>`,
  `EventReader<MouseWheel>` directly (same pattern proven in the wry prototype)
- Add `ViewportRect` resource for hit testing (mouse must be inside the viewport hole)
- Update on resize: `ViewportRect` tracks current physical/logical viewport bounds

**`src-tauri/src/viewport_gizmo_interaction.rs`**

- Remove reads from `ViewportInputState::delta`
- Replace with `EventReader<MouseMotion>` and `ButtonInput<MouseButton>` directly

**`src-tauri/src/viewport_selection.rs`**

- Remove reads from `ViewportInputState` for click detection
- Replace with Bevy `ButtonInput<MouseButton>` + cursor position

### TypeScript seams (3 files, minimal changes)

**`src/hooks/useSendCommand.ts`**

```typescript
// Before:
import { invoke } from "@tauri-apps/api/core";
return useCallback((command: EditorCommand) => {
  invoke("send_editor_command", { command }).catch(console.error);
}, []);

// After:
return useCallback((command: EditorCommand) => {
  window.ipc.postMessage(JSON.stringify({ type: "command", command }));
}, []);
```

**`src/hooks/useEditorState.ts`**

```typescript
// Before: listen<EditorSnapshot>("editor_state", ...)
// After: Rust calls evaluate_script("window.__xrds__.onEditorState(JSON.stringify(snap))")
useEffect(() => {
  (window as any).__xrds__ ??= {};
  (window as any).__xrds__.onEditorState = (json: string) =>
    setSnapshot(JSON.parse(json));
}, []);
```

**`src/App.tsx`** — file dialog `invoke()` calls → request-response IPC

```typescript
// Shared helper replacing all invoke("show_*_dialog") calls
function ipcDialog(kind: string, opts: object): Promise<string | null> {
  return new Promise(resolve => {
    const id = Math.random().toString(36).slice(2);
    (window as any).__xrds__ ??= {};
    (window as any).__xrds__.dialogs ??= {};
    (window as any).__xrds__.dialogs[id] = resolve;
    window.ipc.postMessage(JSON.stringify({ type: "file_dialog", id, kind, opts }));
  });
}
// Rust responds: evaluate_script(`window.__xrds__.dialogs['${id}']?.(${result_json})`)
```

**`src/components/ViewportCanvas.tsx`** — delete. Replace usage in `App.tsx` with:

```tsx
<div className="viewport-canvas" />
```

CSS placeholder only; Bevy renders natively in the hole cut by `SetWindowRgn`.

---

## SetWindowRgn viewport hole

`SetWindowRgn` physically removes the viewport rectangle from the WebView HWND's clipping region.
No transparency tricks needed — the DXGI swap chain is visible there because no child HWND covers it.
Mouse events in that area bypass the WebView and reach Bevy directly.

```rust
// After WebViewBuilder::build_as_child(parent) returns:
use wry::WebViewExtWindows;
let container_hwnd = unsafe { webview.controller().ParentWindow().unwrap() };

fn apply_viewport_region(hwnd: HWND, win_w: i32, win_h: i32, vp: &ViewportRect) {
    let full  = unsafe { CreateRectRgn(0, 0, win_w, win_h) };
    let hole  = unsafe { CreateRectRgn(vp.x, vp.y, vp.x + vp.w, vp.y + vp.h) };
    let frame = unsafe { CreateRectRgn(0, 0, win_w, win_h) };
    unsafe { CombineRgn(frame, full, hole, RGN_DIFF) };
    unsafe { SetWindowRgn(hwnd, frame, TRUE) };
}
// Call on creation and again on every WindowResized event.
```

DPI note: `CreateRectRgn` takes physical pixels. Multiply all logical coords by `scale_factor`.

---

## Serving React in wry

**Development:** `with_url("http://localhost:5173")` — hits Vite dev server. No build step needed.

**Production:** `with_custom_protocol("assets", handler)` + `with_url("assets://localhost/index.html")`
where `handler` reads from `dist/` embedded at compile time via `include_dir!` or read from disk.

---

## Implementation phases

### Phase 1 — Application host (Rust) ✅

- [x] Remove Tauri from `src-tauri/Cargo.toml`; add `wry = "0.55"`, `windows-sys = "0.52"`, `raw-window-handle = "0.6"`
- [x] Rewrite `lib.rs::run()`: Bevy App owns window, wry child WebView attached in first Update frame (WINIT_WINDOWS pattern from prototype)
- [x] Implement `SetWindowRgn` viewport hole; discover container HWND by diffing child-window list before/after `build_as_child()`
- [x] Handle `WindowResized`: update hole region + camera viewport physical size

### Phase 2 — IPC bridge (Rust + TypeScript) ✅

- [x] Add IPC handler: `command` messages → `EditorBridge::inbound`
- [x] Add `push_snapshot_to_webview` Bevy system (PostUpdate, NonSend); WebView stored in `EDITOR_WV` thread_local + `EditorWvMarker` NonSend resource
- [x] Add file dialog IPC handler: `rfd` on blocking thread, `evaluate_script` response queued via `pending_responses()`
- [x] Update `useSendCommand.ts`, `useEditorState.ts`, `App.tsx` dialog helpers (removed all `@tauri-apps/api` calls)
- [x] `ResizeObserver` on `.editor-center` sends exact `viewport_bounds` IPC message; used to calibrate both camera viewport and `SetWindowRgn`

### Phase 3 — Delete JPEG pipeline ✅

- [x] Delete `viewport_capture.rs` and `commands.rs` (both dead Tauri-era files)
- [x] Remove `spawn_snapshot_emitter`, `encode_frame_jpeg` from `bevy_bridge.rs`
- [x] Remove `ViewportCapturePlugin` + `setup_render_target` from `bevy_scene.rs`
- [x] Replace `ViewportCanvas.tsx` with static transparent div
- [x] Remove `@tauri-apps/api` from `package.json`

### Phase 4 — Viewport input (restore native Bevy) ✅

- [x] Remove `ViewportInputState` from `viewport_camera.rs`
- [x] Rewrite `orbit_camera_system` with direct `ButtonInput<MouseButton>`, `MessageReader<MouseMotion>`, `MessageReader<MouseWheel>`; removed dead `egui_wants_*` guards
- [x] `viewport_gizmo_interaction.rs` already used direct events — no change needed
- [x] `viewport_selection.rs` already used direct Bevy click events — no change needed
- [x] Remove `ViewportMouseButton/Move/Scroll` from `EditorCommand` (Rust enum + TypeScript was already clean)

### Phase 5 — Polish + production ✅

- [x] Implement production `with_custom_protocol("xrds", …)` bundle serving (`xrds://localhost/`)
- [x] Single-binary mode: `has_dist()` check at startup switches between `xrds://localhost/` and `http://localhost:5173`
- [x] Rename crate from `xrds-editor-tauri` to `xrds-editor` (Cargo.toml, workspace, directory, .gitignore, README)
- [x] Full manual test: orbit, gizmo, selection, hierarchy, inspector, file dialogs, resize, export

---

## Risk areas

| Risk | Mitigation |
| --- | --- |
| `SetWindowRgn` DPI scaling | Multiply all logical coords by `scale_factor`; update region on DPI change |
| `ICoreWebView2Controller::ParentWindow()` availability | `WebViewExtWindows` is public API in wry 0.55; confirmed in wry source |
| Gizmo drag without `ViewportInputState` | Direct mouse event pattern proven in the wry prototype; ported to editor |
| React hot-reload with custom protocol | Use `with_url("http://localhost:5173")` in dev; custom protocol only for release build |
| WINIT_WINDOWS timing | Keep the `try_attach_webview` retry pattern from prototype; attach in Update, not Startup |

---

## Files not in scope

The following files are unchanged throughout and require no attention:

`hierarchy.rs`, `inspector.rs`, `io.rs`, `palette.rs`, `toolbar.rs`, `environment.rs`,
`editor_state.rs`, `commands.rs`, `bridge.rs` (except command enum cleanup),
all files under `src/components/`, `src/styles/`, `src/types/`
