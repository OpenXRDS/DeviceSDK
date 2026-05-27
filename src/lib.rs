pub use xrds_internal::*;

/// Authored scene-document layer.
/// Use this for save/load, import/export, and stable scene data; normal runtime-first SDK code
/// should usually work through `XrdsAPI` and runtime-facing XRDS types instead.
pub use xrds_scene_graph as scene_graph;

/// Re-exports required for building a GUI editor on top of the XRDS SDK.
///
/// Enabled with the `editor` feature:
/// ```toml
/// xrds = { path = "...", features = ["editor"] }
/// ```
///
/// Provides the Bevy ECS primitives (`App`, `Resource`, `Res`, `ResMut`) and
/// the bevy_egui integration types (`EguiContexts`, `EguiPrimaryContextPass`,
/// `egui`) so that editor code does not need direct `bevy` or `bevy_egui`
/// imports.
#[cfg(feature = "editor")]
pub mod editor {
    // ── ECS fundamentals ────────────────────────────────────────────────────────
    pub use bevy::prelude::{
        App, Commands, Component, Entity, EulerRot, GlobalTransform, KeyCode, Local,
        MouseButton, Quat, Query, Res, ResMut, Resource, Single, Startup, Transform, Update,
        Vec2, Vec3, Visibility, With, Without,
    };
    /// Bevy 0.17: `EventReader` is deprecated — use `MessageReader` instead.
    pub use bevy::ecs::message::MessageReader;
    /// Bevy 0.17: `EventWriter` is deprecated — use `MessageWriter` instead.
    pub use bevy::ecs::message::MessageWriter;
    pub use bevy::math::Isometry3d;
    /// Bevy system `Result` type — `Result<(), BevyError>` with default generics.
    pub use bevy::ecs::error::Result;

    // ── Camera, rendering and gizmos ────────────────────────────────────────────
    pub use bevy::prelude::{AssetServer, Camera, Camera3d, Children, Color, Gizmos, Mesh3d, Or, SceneRoot};
    pub use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore};
    /// Mesh edge highlight on selected entities — correct alternative to gizmo circles.
    pub use bevy::pbr::wireframe::{Wireframe, WireframeColor, WireframeConfig, WireframePlugin};
    /// Light component types — query for these to find light entities in the world.
    pub use bevy::prelude::{DirectionalLight, PointLight, SpotLight};

    // ── Mouse and keyboard input ─────────────────────────────────────────────────
    pub use bevy::input::ButtonInput;
    pub use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};

    // ── Viewport picking ─────────────────────────────────────────────────────────
    /// Mesh raycasting events — fired when the cursor hits a 3-D mesh entity.
    pub use bevy::picking::events::{Click, Pointer};
    pub use bevy::picking::pointer::PointerButton;
    /// Entity → XrdsId reverse lookup for mapping click events to scene nodes.
    pub use crate::XrdsIdIndex;
    /// Observer trigger type — use `On<Pointer<Click>>` to receive viewport picks.
    pub use bevy::prelude::On;

    // ── egui ─────────────────────────────────────────────────────────────────────
    /// egui widgets, panels, and layout primitives.
    pub use bevy_egui::egui;
    /// bevy_egui integration: context system param, rendering schedule, and plugin.
    pub use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};

    /// Re-exported so that `#[derive(Resource)]` / `#[derive(Component)]` generated
    /// code can resolve `::bevy_ecs` without the editor crate needing a direct bevy
    /// dependency.  Bring into scope with `use xrds::editor::bevy_ecs;` in any file
    /// that uses those derives.
    #[doc(hidden)]
    pub use bevy::ecs as bevy_ecs;
}
