# XRDS Editor (Tauri)

A GUI editor for the XRDS SDK built with **Tauri 2** + **React + TypeScript** (panels) and **Bevy 0.17** (3D viewport).

The Tauri webview provides the editor panels (hierarchy, inspector, palette, menubar).
The Bevy window provides the 3D viewport with orbit camera, transform gizmo, and play mode.

---

## Architecture

```text
Main thread — Tauri (tao/wry)
  └── Webview window — React + TypeScript panels

Background thread — Bevy (run_on_any_thread = true)
  └── 3D viewport — Bevy renderer + XRDS runtime
```

Commands flow from the webview → Tauri command handler → `Arc<Mutex<VecDeque>>` → Bevy drain system.
State snapshots flow from Bevy → async emitter task → `app.emit("editor_state", …)` → React.

---

## Prerequisites

| Tool                 | Minimum version | Install                               |
| -------------------- | --------------- | ------------------------------------- |
| Rust + Cargo         | 1.77            | [https://rustup.rs](https://rustup.rs)   |
| Node.js              | 20              | [https://nodejs.org](https://nodejs.org) |
| npm                  | 10              | bundled with Node.js                  |
| Tauri CLI (optional) | 2               | `cargo install tauri-cli`           |

**Windows** — requires the [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (installed as part of Visual Studio or the standalone build tools).

**Linux** — requires `libwebkit2gtk-4.1-dev`, `libssl-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

---

## First-time setup

```bash
# 1. Navigate to the editor frontend directory
cd apps/xrds-editor-tauri

# 2. Install npm dependencies
npm install

# 3. Build the React frontend (outputs to dist/)
npm run build
```

After this, `cargo run -p xrds-editor-tauri` will load the compiled frontend from `dist/`.

---

## Running

### Standard (production frontend)

Build the React app once, then run with Cargo.
Use this for day-to-day Rust development or general use.

```bash
# From workspace root
cargo run -p xrds-editor-tauri
```

If you change any React source files (`src/`), rebuild before running:

```bash
cd apps/xrds-editor-tauri && npm run build
```

### Hot-reload (active UI development)

Run the Vite dev server alongside Cargo for instant React hot-reload.

```bash
# Terminal 1 — Vite dev server
cd apps/xrds-editor-tauri
npm run dev
```

Then temporarily add `devUrl` to `src-tauri/tauri.conf.json`:

```json
"build": {
  "frontendDist": "../dist",
  "devUrl": "http://localhost:5173"
}
```

```bash
# Terminal 2 — Tauri app
cargo run -p xrds-editor-tauri
```

> Remove `devUrl` when done — otherwise `cargo run` without the Vite server shows a blank window.

---

## Project structure

```text
apps/xrds-editor-tauri/
├── src/                        # React + TypeScript frontend
│   ├── main.tsx                # React entry point
│   ├── App.tsx                 # Root layout + global keyboard shortcuts
│   ├── components/
│   │   ├── Menubar.tsx         # File / Edit dropdown menus
│   │   ├── Toolbar.tsx         # Gizmo mode, camera, grid, play buttons
│   │   ├── Hierarchy.tsx       # Scene node tree panel
│   │   ├── Inspector.tsx       # Node properties panel
│   │   └── Palette.tsx         # Primitive palette + project assets
│   ├── hooks/
│   │   ├── useEditorState.ts   # Subscribes to Bevy state snapshots
│   │   └── useSendCommand.ts   # Sends commands to Bevy
│   ├── types/
│   │   └── bridge.ts           # TypeScript types for all commands + snapshots
│   └── styles/
│       └── editor.css          # Catppuccin Mocha dark theme
├── src-tauri/                  # Rust / Tauri backend
│   ├── src/
│   │   ├── lib.rs              # Tauri app entry, bridge setup
│   │   ├── bridge.rs           # EditorCommand / EditorSnapshot types
│   │   ├── bevy_bridge.rs      # Bevy resource, drain system, broadcaster
│   │   ├── bevy_scene.rs       # XrdsApp impl, system registration
│   │   ├── commands.rs         # Tauri commands (file dialogs)
│   │   ├── editor_state.rs     # EditorState + EditorSession resources
│   │   ├── hierarchy.rs        # Hierarchy command handlers
│   │   ├── inspector.rs        # Inspector command handlers
│   │   ├── palette.rs          # Palette (spawn) command handlers
│   │   ├── toolbar.rs          # Toolbar + play mode command handlers
│   │   ├── io.rs               # File I/O command handlers
│   │   ├── viewport_camera.rs  # Orbit / fly camera system
│   │   ├── viewport_gizmo.rs   # Transform gizmo + floor grid rendering
│   │   ├── viewport_gizmo_interaction.rs  # Gizmo drag interaction
│   │   ├── viewport_selection.rs          # Ray-cast click selection
│   │   └── viewport_player.rs             # Play mode pawn locomotion
│   ├── tauri.conf.json         # Tauri app configuration
│   └── Cargo.toml
├── index.html                  # HTML entry (Vite root)
├── vite.config.ts
├── tsconfig.json
└── package.json
```

---

## Editor keyboard shortcuts

### Tauri panel window

| Key                        | Action                    |
| -------------------------- | ------------------------- |
| `T`                      | Translate gizmo mode      |
| `R`                      | Rotate gizmo mode         |
| `S` / `Y`              | Scale gizmo mode          |
| `V`                      | Toggle Orbit / Fly camera |
| `G`                      | Toggle floor grid         |
| `F`                      | Frame selected node       |
| `Space`                  | Play / Stop               |
| `Escape`                 | Deselect / Stop play      |
| `Delete` / `Backspace` | Delete selected nodes     |
| `Ctrl+Z`                 | Undo                      |
| `Ctrl+Y`                 | Redo                      |
| `Ctrl+N`                 | New scene                 |
| `Ctrl+O`                 | Open scene                |
| `Ctrl+S`                 | Save scene                |
| `Ctrl+Shift+S`           | Save scene as             |
| `Ctrl+I`                 | Import asset (GLB / GLTF) |

### Bevy viewport window

| Input                      | Action                     |
| -------------------------- | -------------------------- |
| Middle drag                | Orbit camera               |
| Shift + Middle drag        | Pan camera                 |
| Scroll wheel               | Zoom                       |
| WASD / Q / E               | Move camera pivot          |
| RMB + drag (Fly mode)      | Free-look                  |
| Left click                 | Select object              |
| Ctrl + Left click          | Add to selection           |
| Drag gizmo axis            | Translate / Rotate / Scale |
| `Delete` / `Backspace` | Delete selected nodes      |
| `F`                      | Frame selected             |

---

## Gitignore

The following directories are generated and should not be committed:

```text
apps/xrds-editor-tauri/node_modules/      # npm packages   — restore: npm install
apps/xrds-editor-tauri/dist/              # Vite output    — restore: npm run build
apps/xrds-editor-tauri/src-tauri/target/  # Rust build     — covered by root .gitignore
```

---

## Asset import

1. **File → Import Asset…** (or `Ctrl+I`) — opens a file dialog for `.glb` / `.gltf` files.
2. The asset is registered in the project catalog and appears in the **Project Assets** tab of the palette.
3. Click an asset in the palette to spawn it into the scene.
4. Click **✕** on a palette asset to remove it from the project (also removes all scene instances).

---

## Scene save format

Scenes are saved as JSON (`.json`) in the XRDS scene document format.
Assets are referenced by absolute path — if you move asset files, re-import them.

---

## Exported App — PCVR window rendering

The exported runtime (`xrds-app`) has a dual rendering path for the desktop window and the HMD.
`AppCamera` is both the locomotion root and the desktop window camera; the XR blit takes over the
window when the HMD session is actively rendering.

| State                           | XR cameras active?             | Window shows                                                 |
| ------------------------------- | ------------------------------ | ------------------------------------------------------------ |
| No HMD connected                | No (entities don't exist)      | AppCamera renders — desktop view                            |
| HMD connected, session starting | No (`should_render = false`) | AppCamera renders — desktop view                            |
| HMD on, session running         | Yes                            | AppCamera disabled; blit copies left eye — exact HMD pixels |
| HMD covered (proximity sensor)  | No (`should_render = false`) | AppCamera reactivates — desktop view                        |
| HMD taken off mid-session       | No                             | AppCamera reactivates immediately                            |

The toggle is purely reactive — `manage_window_camera` checks whether any XR eye camera is
`is_active` each frame; no session-state polling required.

---

## Dependencies

### Frontend (`package.json`)

- `react` / `react-dom` 18
- `@tauri-apps/api` 2
- `vite` 5, `@vitejs/plugin-react`, `typescript` 5

### Backend (`src-tauri/Cargo.toml`)

- `tauri` 2
- `xrds-runtime` (workspace)
- `xrds-scene-graph` (workspace)
- `bevy` 0.17 (workspace)
- `bevy_mod_outline` 0.11
- `rfd` 0.15 (native file dialogs)
- `tokio` (workspace)

## Build Command

- npm run build
- cargo run -p xrds-editor-tauri
