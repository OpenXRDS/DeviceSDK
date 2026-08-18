# Export Application Feature Plan

Export the current scene as a standalone, distributable XR application.

---

## Goal

The user presses **File → Export Application…**, picks an output folder, and the editor
produces a self-contained directory they can run on any machine (or deploy to a headset):

```
output_dir/
├── xrds-app(.exe)        ← compiled XR binary
├── scene.json            ← the authored scene document
└── assets/               ← fonts, models, textures, audio referenced in scene.json
```

Running `xrds-app` loads `scene.json` from the same directory and plays it back using
`xrds-runtime` with XR enabled.

---

## Architecture

### Editor side (Tauri)

1. User clicks **File → Export Application…**
2. Native folder-picker dialog opens (`rfd::AsyncFileDialog::pick_folder`)
3. Editor sends `ExportApplication { output_dir: String }` command to Bevy
4. Bevy handler:
   a. Saves current document to `output_dir/scene.json`
   b. Copies `assets/` folder to `output_dir/assets/`
   c. Spawns a background thread running `cargo build --release -p xrds-app`
   d. Returns a `build_job` handle; UI polls it for status
5. On build success: copies the compiled binary to `output_dir/`
6. Status toast: "Exported to output_dir/ (build took N s)"

### Exported app binary (`apps/xrds-app`)

A minimal crate in the workspace that:

```rust
fn main() {
    Runtime::new(RuntimeParameters {
        app_name: env!("CARGO_PKG_NAME").to_owned(),
        enable_xr: true,
        asset_path: Some(
            std::env::current_exe()
                .unwrap()
                .parent().unwrap()
                .join("assets")
                .to_string_lossy()
                .into_owned()
        ),
        allow_unapproved_paths: false,
        ..Default::default()
    })
    .run_xrds(SceneFileApp::new("scene.json"))
    .unwrap();
}
```

`SceneFileApp` is a generic `XrdsApp` implementation that:
- Reads `scene.json` from the executable's directory at startup
- Calls `api.import_scene_document_json("scene.json")`
- Passes GLB animation autoplay the same way as the editor's play mode

---

## New files / changes required

### 1. `apps/xrds-app/` (NEW crate)

```
apps/xrds-app/
├── Cargo.toml      ← depends on xrds-runtime; name = "xrds-app"
└── src/
    └── main.rs     ← SceneFileApp + main()
```

Add to workspace `Cargo.toml`:
```toml
members = ["crates/*", "apps/xrds-editor-tauri/src-tauri", "apps/xrds-app"]
```

### 2. `apps/xrds-editor-tauri/src-tauri/src/io.rs`

Add `ExportApplication { output_dir }` handler:

```rust
EditorCommand::ExportApplication { output_dir } => {
    // Step 1: save scene document
    let scene_path = Path::new(output_dir).join("scene.json");
    let _ = session.0.document().save_json(&scene_path);

    // Step 2: copy assets/
    copy_assets_dir(asset_root, output_dir);

    // Step 3: background cargo build
    let out_dir = output_dir.clone();
    let handle = std::thread::spawn(move || {
        std::process::Command::new("cargo")
            .args(["build", "--release", "-p", "xrds-app"])
            .status()
            .map(|s| if s.success() { Ok(()) } else { Err("build failed".to_string()) })
            .unwrap_or_else(|e| Err(e.to_string()))
    });
    state.build_job = Some(BuildJob { handle, out_dir });
    false
}
```

### 3. `apps/xrds-editor-tauri/src-tauri/src/editor_state.rs`

Add build job state:

```rust
pub struct BuildJob {
    pub handle: std::thread::JoinHandle<Result<(), String>>,
    pub out_dir: String,
}

// In EditorState:
pub build_job: Option<BuildJob>,
pub build_progress: Option<String>,  // "Building…", "Done", "Error: …"
```

### 4. `apps/xrds-editor-tauri/src-tauri/src/bevy_scene.rs` — `update()`

Poll the build job handle each frame:

```rust
if let Some(job) = state.build_job.take_if(|j| j.handle.is_finished()) {
    match job.handle.join() {
        Ok(Ok(())) => {
            // Copy binary to output_dir
            copy_binary(&job.out_dir)?;
            state.pending_status = Some(format!("Exported to {}", job.out_dir));
        }
        Ok(Err(e)) | Err(_) => {
            state.pending_status = Some(format!("Build failed: {e}"));
        }
    }
}
```

### 5. `apps/xrds-editor-tauri/src/components/Menubar.tsx`

Add "Export Application…" entry under File:

```tsx
<div className="mb-action" onClick={action(onExportApp)}>
  Export Application… <span className="mb-shortcut">Ctrl+Shift+A</span>
</div>
```

### 6. `apps/xrds-editor-tauri/src/App.tsx`

```tsx
async function handleExportApp() {
  const dir = await invoke<string | null>("show_export_app_dialog");
  if (dir) send({ type: "ExportApplication", payload: { output_dir: dir } });
}
```

---

## Asset copy logic

```rust
fn copy_assets_dir(src: &Path, output_dir: &str) {
    let dst = Path::new(output_dir).join("assets");
    std::fs::create_dir_all(&dst).ok();
    copy_dir_recursive(src, &dst);
}
```

Only copy assets actually referenced in the document (GLB, textures, audio) to keep
the output small.  For the first version, copying the entire `assets/` folder is acceptable.

---

## Build configuration

The `xrds-app` crate needs to find Cargo at runtime.  Use `CARGO` env var if set (CI),
fall back to `cargo` on PATH:

```rust
let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
std::process::Command::new(cargo).args([...])
```

For Windows, the compiled binary is at
`target/release/xrds-app.exe`.  Copy to `output_dir/`.

---

## UI / progress feedback

Since `cargo build --release` can take 30–120 s on first run:

- Status bar shows "Building… (Ctrl+. to cancel)" while `build_job` is active
- Progress updates are limited to `pending_status` one-shots until the handle resolves
- On completion: toast "Exported to output_dir/ (42 s)"
- On failure: toast "Build failed: <stderr excerpt>"

---

## Implementation checklist ✅ All complete

- [x] Create `apps/xrds-app/` crate with `SceneFileApp` + `main()`
- [x] Add to workspace members
- [x] `show_export_app_dialog` Tauri command (folder picker via `rfd`)
- [x] `ExportApplication { output_dir }` command in `bridge.rs`
- [x] Handler in `io.rs` — save scene, copy assets, spawn build thread
- [x] `ExportJob` in `EditorState` (`Arc<Mutex<Option<Result>>>` result channel, not JoinHandle)
- [x] Build job poll in `bevy_scene.rs::update()` — copy binary on success
- [x] File → Export Application… menu entry + `Ctrl+Shift+A` keyboard shortcut
- [x] Build progress bar (`is_exporting` snapshot field → persistent spinner + disabled menu item)
- [x] Test: export confirmed working, scene loads in exported binary
- [x] Test: assets (fonts, models) load correctly — fixed `asset_path = exe_dir/assets`, URIs = plain filenames

## Post-plan additions

- [x] `PlayerSpawn` node detection — `read_spawn_config()` parses `scene.json` to find spawn position, rotation, FOV, and locomotion mode
- [x] Fly + grounded locomotion systems (WASD + RMB look, gravity, Space jump)
- [x] `deactivate_scene_cameras` — prevents scene Camera nodes from conflicting with `AppCamera`
- [x] FOV fix — default 60° vertical (was 90° causing edge distortion); `XrdsScenePlayerSpawn::fov_deg` default updated to match
- [x] Per-dialog path caching — each dialog type remembers its own last directory, persisted to `<AppData>/dialog_paths.json`
