# XRDS Editor

A GUI editor for the XRDS SDK built with **wry** (WebView panels) + **React + TypeScript** and **Bevy 0.17** (3D viewport).

Bevy owns the OS window. A wry child WebView sits on top as the UI layer. `SetWindowRgn` carves a transparent hole in the WebView so the Bevy DXGI surface shows through natively — no JPEG streaming, no roundtrip latency.

---

## Architecture

```text
OS Window (Bevy / winit)
  ├── Bevy renderer — 3D viewport rendered natively into the centre hole
  └── wry child WebView (full-window, clipped to panels only via SetWindowRgn)
        └── React + TypeScript — hierarchy, inspector, palette, menubar, toolbar
```

IPC: `window.ipc.postMessage(JSON.stringify(command))` → Rust IPC handler → `EditorBridge::inbound`  
State: Bevy calls `evaluate_script("window.__xrds__.onEditorState(…)")` each frame → React state

---

## Prerequisites

| Tool         | Minimum version | Install                               |
| ------------ | --------------- | ------------------------------------- |
| Rust + Cargo | 1.77            | [https://rustup.rs](https://rustup.rs)   |
| Node.js      | 20              | [https://nodejs.org](https://nodejs.org) |
| npm          | 10              | bundled with Node.js                  |

**Windows** — requires the [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

**Linux** — requires `libwebkit2gtk-4.1-dev`, `libssl-dev`, `libgtk-3-dev`.

---

## First-time setup

```bash
# 1. Navigate to the editor frontend directory
cd apps/xrds-editor

# 2. Install npm dependencies
npm install

# 3. Build the React frontend (outputs to dist/)
npm run build
```

After this, `cargo run -p xrds-editor` will load the compiled frontend from `dist/`.

---

## Running

### Standard (single binary, production frontend)

Build the React app once, then run with Cargo.

```bash
# From workspace root
cargo run -p xrds-editor
```

If you change any React source files (`src/`), rebuild before running:

```bash
cd apps/xrds-editor && npm run build
```

### Hot-reload (active UI development)

Run the Vite dev server alongside Cargo for instant React hot-reload (HMR).

```bash
# Terminal 1 — Vite dev server
cd apps/xrds-editor
npm run dev
```

```bash
# Terminal 2 — Bevy app (auto-detects no dist/, loads from http://localhost:5173)
cargo run -p xrds-editor
```

The app checks for `dist/index.html` at startup. If absent, it loads from the Vite dev server URL automatically — no config change needed.

---

## Project structure

```text
apps/xrds-editor/
├── src/                        # React + TypeScript frontend
│   ├── main.tsx                # React entry point
│   ├── App.tsx                 # Root layout, global keyboard shortcuts, IPC helpers
│   ├── components/
│   │   ├── Menubar.tsx         # File / Edit dropdown menus
│   │   ├── Toolbar.tsx         # Gizmo mode, camera, grid, play buttons
│   │   ├── Hierarchy.tsx       # Scene node tree panel
│   │   ├── Inspector.tsx       # Node properties panel
│   │   ├── Palette.tsx         # Primitive palette + project assets
│   │   ├── PlayerPanel.tsx     # Player anchor controls
│   │   └── ViewportCanvas.tsx  # Transparent div — Bevy renders through the hole
│   ├── hooks/
│   │   ├── useEditorState.ts   # Subscribes to Bevy state via window.__xrds__
│   │   └── useSendCommand.ts   # Sends commands via window.ipc.postMessage
│   ├── types/
│   │   └── bridge.ts           # TypeScript types for all commands + snapshots
│   └── styles/
│       └── editor.css          # Catppuccin Mocha dark theme
├── src-tauri/                  # Rust backend
│   ├── src/
│   │   ├── lib.rs              # Entry point — creates Bevy app
│   │   ├── wry_overlay.rs      # wry WebView attachment, SetWindowRgn, IPC bridge
│   │   ├── bridge.rs           # EditorCommand / EditorSnapshot types
│   │   ├── bevy_bridge.rs      # Bevy resource, drain system, broadcaster
│   │   ├── bevy_scene.rs       # XrdsApp impl, system registration
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
│   └── Cargo.toml
├── index.html                  # HTML entry (Vite root)
├── vite.config.ts
├── tsconfig.json
└── package.json
```

---

## Keyboard shortcuts

| Key                        | Action                    |
| -------------------------- | ------------------------- |
| `T`                        | Translate gizmo mode      |
| `R`                        | Rotate gizmo mode         |
| `Y`                        | Scale gizmo mode          |
| `G`                        | Toggle floor grid         |
| `F`                        | Frame selected node       |
| `Space`                    | Play / Stop               |
| `Escape`                   | Deselect / Stop play      |
| `Delete` / `Backspace`     | Delete selected nodes     |
| `Ctrl+Z`                   | Undo                      |
| `Ctrl+Y`                   | Redo                      |
| `Ctrl+N`                   | New scene                 |
| `Ctrl+O`                   | Open scene                |
| `Ctrl+S`                   | Save scene                |
| `Ctrl+Shift+S`             | Save scene as             |
| `Ctrl+I`                   | Import asset (GLB / GLTF) |
| `Ctrl+Shift+E`             | Export GLB                |
| `Ctrl+Shift+A`             | Export application        |

### Viewport mouse

| Input                      | Action                     |
| -------------------------- | -------------------------- |
| Middle drag                | Orbit camera               |
| Shift + Middle drag        | Pan camera                 |
| Right drag                 | Orbit camera               |
| Scroll wheel               | Zoom                       |
| WASD / Q / E               | Move camera pivot          |
| RMB + drag (Fly mode)      | Free-look                  |
| Left click                 | Select object              |
| Ctrl + Left click          | Add to selection           |
| Shift + Left click         | Add to selection           |
| Drag gizmo axis            | Translate / Rotate / Scale |
| Ctrl + drag (Translate)    | Snap to grid               |
| Shift + drag (Scale)       | Uniform scale              |

---

## Gitignore

The following directories are generated and should not be committed:

```text
apps/xrds-editor/node_modules/      # npm packages   — restore: npm install
apps/xrds-editor/dist/              # Vite output    — restore: npm run build
apps/xrds-editor/src-tauri/target/  # Rust build     — covered by root .gitignore
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

## Dependencies

### Frontend (`package.json`)

- `react` / `react-dom` 18
- `vite` 5, `@vitejs/plugin-react`, `typescript` 5

### Backend (`src-tauri/Cargo.toml`)

- `wry` 0.55 (WebView)
- `raw-window-handle` 0.6 (HWND extraction)
- `windows-sys` 0.52 (SetWindowRgn, Win32 API)
- `xrds-runtime` (workspace)
- `xrds-scene-graph` (workspace)
- `bevy` 0.17 (workspace)
- `bevy_mod_outline` 0.11
- `rfd` 0.15 (native file dialogs)
- `tokio` (workspace)

## Build Commands

```bash
# Build frontend once (required before first cargo run)
cd apps/xrds-editor && npm run build

# Run
cargo run -p xrds-editor

# Dev mode (hot-reload, no npm build needed)
cd apps/xrds-editor && npm run dev   # Terminal 1
cargo run -p xrds-editor             # Terminal 2
```
