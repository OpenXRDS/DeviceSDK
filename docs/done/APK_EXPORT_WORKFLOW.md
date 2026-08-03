# APK Export Workflow — TODO

Goal: add an "Export for Quest" action to xrds-editor that produces a ready-to-install
APK bundle in a user-chosen directory, complete with a one-command install script.

---

## Current state

| What exists | Location |
| --- | --- |
| Scene export (scene.json + assets) | `io.rs::ExportApplication` |
| APK build script (Windows) | `android/quest/build.ps1 -SceneDir <path>` |
| APK build script (Linux) | `android/quest/build.sh --scene-dir <path>` |
| OpenXR loader fetch | `android/quest/fetch_loader.ps1 / fetch_loader.sh` |
| Export progress IPC | `EditorSnapshot.is_exporting` |

Missing: everything that wires the editor UI to the APK build scripts and produces
the final output directory with an install script.

## Constraints

- **Scene must be saved before export.** The editor enforces this: if `is_dirty` is
  true, export is blocked and the user is prompted to save first.
- **Scene file is always normalized to `scene.json` inside the APK.** The runtime
  hardcodes this name when loading from assets. Whatever filename the user saved
  the scene as, `prepare_export_document()` always copies it into the staging
  directory as `scene.json`. No user-facing rename step is needed.

---

## TODO

### 1. Prerequisites and environment check (backend, `io.rs` or new `android.rs`)  ✅

- [x] On editor startup (or lazily on first APK export attempt), validate:
  - `ANDROID_HOME` is set and `build-tools/` exists
  - Android NDK found (`ANDROID_NDK_HOME` or auto-detected under `$ANDROID_HOME/ndk/`)
  - `cargo-ndk` is installed (`cargo ndk --version`)
  - `libs/arm64-v8a/libopenxr_loader.so` exists under `android/quest/` (fetched by user)
- [x] Expose `CheckApkPrerequisites` IPC command → returns list of
  `ApkPrerequisite { name, ok, hint }` via `EditorSnapshot.apk_prerequisites` (one-shot field).

### 2. New Tauri command: `ExportApk` (backend)  ✅

- [x] Add `ExportApk { output_dir: String }` / `CheckApkPrerequisites` to `EditorCommand` in `bridge.rs`
- [x] Add `is_exporting_apk: bool`, `apk_build_log: Vec<String>`, `apk_prerequisites` to `EditorSnapshot`
- [x] Implement in `io.rs`:
  0. Reject if concurrent job is already running or scene is unsaved
  1. Stage scene assets to temp dir; scene file always written as `scene.json`
  2. Run `build.ps1 -SceneDir` (Windows) / `build.sh --scene-dir` (Linux), streaming stdout+stderr
  3. Collect exit code via `child.wait()` (Phase 7 fix); surface non-zero as error
  4. Copy `xrds-app.apk` and write install scripts into `output_dir/`

### 3. Install script generation (backend)  ✅

- [x] `output_dir/install.ps1` (Windows, CRLF line endings)
- [x] `output_dir/install.sh` (Unix, chmod 755)

### 4. IPC types (`bridge.ts`)  ✅

- [x] `CheckApkPrerequisites` / `ExportApk` added to `EditorCommand` union
- [x] `apk_prerequisites`, `is_exporting_apk`, `apk_build_log` added to `EditorSnapshot`

### 5. Build log streaming (IPC)  ✅

Chose Option B (poll into snapshot): `Arc<Mutex<Vec<String>>>` appended by two reader threads;
last 200 lines included in each snapshot frame. No Tauri event system needed.

### 6. UI: Export dialog / menu item  ✅

- [x] "Export for Quest…" menu item — disabled while `is_exporting` or `is_exporting_apk`
- [x] `ApkExportDialog` component: prereq checklist, dir picker, live build log, done message
- [x] Dirty-scene warning blocks export; modal disables viewport hole during display

### 7. Error handling  ✅

- [x] Build script exit code captured via `child.wait()` after reader threads drain;
  non-zero exits report "Build script failed (exit code N) — see log for details"
- [x] `libopenxr_loader.so` missing detected in prerequisites check with targeted hint
- [x] Concurrent export guard: backend rejects second `ExportApk`; menu item disabled

### 8. Testing / validation

- [x] Manual end-to-end: export from editor → `adb install` → launch on Quest →
  verify scene loads correctly. *(Windows confirmed 2026-06-25)*
- [x] Test with `-SceneDir` containing GLB assets (not just primitives). *(confirmed 2026-06-25)*
- [x] Test on Linux (build.sh path). *(confirmed 2026-06-29)*
- [x] Verify install scripts run on Linux/macOS (bash) without modification. *(confirmed 2026-06-29)*

---

## Output directory layout (target state)

```text
<output_dir>/
  xrds-app.apk          ← signed APK
  install.ps1           ← Windows one-click install
  install.sh            ← Linux/macOS one-click install
  README.txt            ← brief instructions
```
