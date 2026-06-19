// XRDS Interface Level: 1 (Application Glue)
// Purpose: Attach XRDS resources/systems to an existing runtime app and expose XRDS-centric scene helpers.
// Target: Application developers who want XRDS semantics without wiring Bevy internals directly.
// When To Use: Construct the app through Runtime, then attach XRDS during runtime setup.
#[path = "xrds_api/api.rs"]
mod api;
#[path = "xrds_api/context.rs"]
mod context;
#[path = "xrds_api/environment.rs"]
mod environment;
#[path = "xrds_api/gltf.rs"]
mod gltf;
#[path = "xrds_api/helper.rs"]
mod helper;
#[path = "xrds_api/hierarchy.rs"]
mod hierarchy;
#[path = "xrds_api/install.rs"]
mod install;
#[path = "xrds_api/material.rs"]
mod material;
#[path = "xrds_api/recipes.rs"]
mod recipes;
#[path = "xrds_api/reimport.rs"]
mod reimport;
#[path = "xrds_api/registry.rs"]
mod registry;
#[path = "xrds_api/spawn.rs"]
mod spawn;
#[path = "xrds_api/state.rs"]
mod state;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[path = "xrds_api/anchor.rs"]
mod anchor;
#[path = "xrds_api/grab.rs"]
mod grab;
#[path = "xrds_api/raycast.rs"]
mod raycast;
#[path = "xrds_api/billboard.rs"]
mod billboard;
#[path = "xrds_api/updaters.rs"]
mod updaters;
#[path = "xrds_api/zone.rs"]
mod zone;

pub use anchor::{
    ActivePlayerAnchorEntity, PlayerAnchorCameraPose,
    XrdsAnchorExposure, XrdsAnchorFov, XrdsBodyLocked, XrdsComfortPinned, XrdsCylindrical,
    XrdsHeadLocked, XrdsInitialAnchor, XrdsPlayerAnchorRoot, XrdsPlayerCamera, XrdsPlayerRoot,
};
pub use billboard::XrdsBillboard;
pub use xrds_components::primitives::{XrdsText, XrdsTextAlignment, XrdsTextAnchor};
pub use xrds_components::TextParams;

use bevy::{ecs::world::CommandQueue, prelude::*};
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;
use xrds_components::primitives::{
    XrdsCube, XrdsCylinder, XrdsExtrudedText, XrdsExtrudedTextAlignment,
    XrdsPlane3D, XrdsSphere, XrdsTetrahedron,
};
use xrds_components::world::lights::{
    XrdsAmbientLight, XrdsDirectionalLight, XrdsPointLight, XrdsSpotLight,
};
use xrds_components::world::{XrdsAudioClip, XrdsCamera, XrdsGltfAsset, XrdsNode};
use xrds_components::{
    AmbientLightParams, CameraKind, CameraLookAtPatch, CameraProjectionParams,
    CameraProjectionPatch, CubeGeometryParams, CylinderGeometryParams, DirectionalLightParams,
    GltfAssetSourcePatch, NamePatch, OrthographicCameraParams, ParentPatch,
    PerspectiveCameraParams, Plane3DGeometryParams, PointLightParams, SphereGeometryParams,
    ExtrudedTextParams, SpotLightParams, TetrahedronGeometryParams, TransformParams,
    VisibilityPatch,
    XrdsAssetComponent, XrdsColor, XrdsComponent, XrdsComponentsPlugin, XrdsId, XrdsLinearRgba,
    XrdsMaterialAlphaMode, XrdsMaterialParams, XrdsMaterialPbrParams, XrdsMaterialTextureRef,
    XrdsMaterialTextureSlotKind, XrdsMaterialTextureSlots,
    XrdsMutableComponent, XrdsRegistry,
};
use xrds_scene_graph::{
    XrdsEditorMetadata, XrdsSceneAnimationRepeatMode, XrdsSceneAsset,
    XrdsSceneAssetKind, XrdsSceneDocument, XrdsSceneGltfAnimationSelector,
    XrdsSceneGltfMorphTargetSelector, XrdsSceneGltfNodeAuthoring, XrdsSceneGltfPlayback,
    XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodePayload,
    XrdsSceneRuntimeComponent, XrdsSceneRuntimeNode,
};

pub use context::XrdsUpdateContext;
pub use xrds_components::{XrDropEvent, XrGrabEvent, XrGrabHand, XrGrabbable, XrGrabbed, XrRayhit, XrdsPlayerSpawnZone};
pub use xrds_components::XrdsPhysicsBody;
pub use xrds_components::{XrdsGrabType, XrdsInteractionZone, XrdsInteractionZoneShape, XrZoneEnterEvent, XrZoneExitEvent};
pub use environment::XrdsReceivesEnvironment;
/// Read-only entity→id index exposed for viewport picking systems.
/// Access as `Res<XrdsIdIndex>` in Bevy systems: `id_index.id_of(entity)`
/// returns the `XrdsId` for any entity that was spawned by an XRDS import.
pub use state::XrdsIdIndex;
pub use state::XrdsStoredHudInstance;

/// `SystemSet` label for `run_xrds_app_update` (the exclusive system that drives
/// `XrdsApp::update`).  Runs in `PostUpdate`, before Bevy's `VisibilityPropagate`
/// and before `ensure_visibility_hierarchy_components_system`, so any entities
/// spawned by a reimport are visible to the PostUpdate visibility safety-net.
#[derive(bevy::ecs::schedule::SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct XrdsUpdateSystemSet;
use environment::*;
use gltf::*;
use helper::*;
use hierarchy::*;
use install::*;
use material::*;
use recipes::*;
use registry::*;
use spawn::*;
use state::*;
use updaters::*;

#[derive(Debug, Clone)]
pub enum XrdsGeometrySource {
    /// Spawn geometry and material from an external asset file (`.gltf`, `.glb`, `.obj`, …).
    /// `name`, `transform`, and `visible` are filled automatically from the descriptor.
    /// Prefer [`XrdsAPI::register_asset_interpreter`] when the descriptor already implements
    /// [`XrdsAssetComponent`] — that requires zero closure code.
    GltfScene { path: String, scene_index: usize },
    /// Fallback: PBR unit sphere. `name`, `transform`, and `visible` are filled from the descriptor.
    PbrSphere {
        radius: f32,
        material: XrdsMaterialParams,
    },
    /// Fallback: PBR box primitive. `name`, `transform`, and `visible` are filled from the descriptor.
    PbrCuboid {
        half_extents: [f32; 3],
        material: XrdsMaterialParams,
    },
    /// Fallback: PBR cylinder primitive. `name`, `transform`, and `visible` are filled from the descriptor.
    PbrCylinder {
        radius: f32,
        half_height: f32,
        material: XrdsMaterialParams,
    },
    /// Fallback: PBR plane primitive. `name`, `transform`, and `visible` are filled from the descriptor.
    PbrPlane {
        size: [f32; 2],
        material: XrdsMaterialParams,
    },
    /// Fallback: PBR tetrahedron built from authored vertices.
    PbrTetrahedron {
        vertices: [[f32; 3]; 4],
        material: XrdsMaterialParams,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsGltfLoadStatus {
    NotLoaded,
    Loading,
    Loaded,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsGltfRuntimeError {
    NotAGltfRuntimeEntity,
    AssetNotLoaded(String),
    AnimationNotFound(String),
    MorphTargetMeshNotFound(String),
    MorphTargetNotFound(String),
    InvalidMorphTargetWeight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsGltfAnimationSelector {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsAnimationRepeatMode {
    Once,
    Loop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsGltfAnimationPlaybackOptions {
    pub repeat: XrdsAnimationRepeatMode,
    pub speed: f32,
    pub start_paused: bool,
}

impl Default for XrdsGltfAnimationPlaybackOptions {
    fn default() -> Self {
        Self {
            repeat: XrdsAnimationRepeatMode::Loop,
            speed: 1.0,
            start_paused: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsGltfAnimationInfo {
    pub index: usize,
    pub name: Option<String>,
    pub duration_secs: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsGltfAnimationState {
    pub animation: XrdsGltfAnimationInfo,
    pub playing: bool,
    pub paused: bool,
    pub repeat: XrdsAnimationRepeatMode,
    pub speed: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsGltfNodeLocator {
    pub node_index_path: Vec<usize>,
    pub node_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsGltfMorphTargetInfo {
    pub index: usize,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsGltfMorphTargetSelector {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsGltfMorphTargetWeightValue {
    pub target: XrdsGltfMorphTargetInfo,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsGltfMorphTargetWeights {
    pub node: XrdsGltfNodeLocator,
    pub mesh_name: Option<String>,
    pub weights: Vec<XrdsGltfMorphTargetWeightValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsGltfMorphTargetSet {
    pub node: XrdsGltfNodeLocator,
    pub mesh_name: Option<String>,
    pub targets: Vec<XrdsGltfMorphTargetInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsSceneImportError {
    DuplicateRuntimeId(XrdsId),
    InvalidDocument(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsSceneExportError {
    MissingRuntimeEntity(XrdsId),
    UnsupportedRuntimeDescriptor(XrdsId),
    InvalidRuntimeState(String),
}

impl From<xrds_scene_graph::XrdsSceneValidationError> for XrdsSceneImportError {
    fn from(value: xrds_scene_graph::XrdsSceneValidationError) -> Self {
        Self::InvalidDocument(format!("{value:?}"))
    }
}

impl From<xrds_scene_graph::XrdsSceneValidationError> for XrdsSceneExportError {
    fn from(value: xrds_scene_graph::XrdsSceneValidationError) -> Self {
        Self::InvalidRuntimeState(format!("{value:?}"))
    }
}

impl From<XrdsGltfRuntimeError> for XrdsSceneExportError {
    fn from(value: XrdsGltfRuntimeError) -> Self {
        Self::InvalidRuntimeState(format!("{value:?}"))
    }
}

impl From<XrdsSceneGltfAnimationSelector> for XrdsGltfAnimationSelector {
    fn from(value: XrdsSceneGltfAnimationSelector) -> Self {
        match value {
            XrdsSceneGltfAnimationSelector::Index(index) => Self::Index(index),
            XrdsSceneGltfAnimationSelector::Name(name) => Self::Name(name),
        }
    }
}

impl From<XrdsSceneAnimationRepeatMode> for XrdsAnimationRepeatMode {
    fn from(value: XrdsSceneAnimationRepeatMode) -> Self {
        match value {
            XrdsSceneAnimationRepeatMode::Once => Self::Once,
            XrdsSceneAnimationRepeatMode::Loop => Self::Loop,
        }
    }
}

impl From<XrdsAnimationRepeatMode> for XrdsSceneAnimationRepeatMode {
    fn from(value: XrdsAnimationRepeatMode) -> Self {
        match value {
            XrdsAnimationRepeatMode::Once => Self::Once,
            XrdsAnimationRepeatMode::Loop => Self::Loop,
        }
    }
}

impl From<XrdsSceneGltfPlayback> for XrdsGltfAnimationPlaybackOptions {
    fn from(value: XrdsSceneGltfPlayback) -> Self {
        Self {
            repeat: value.repeat.into(),
            speed: value.speed,
            start_paused: value.start_paused,
        }
    }
}

impl From<XrdsSceneGltfMorphTargetSelector> for XrdsGltfMorphTargetSelector {
    fn from(value: XrdsSceneGltfMorphTargetSelector) -> Self {
        match value {
            XrdsSceneGltfMorphTargetSelector::Index(index) => Self::Index(index),
            XrdsSceneGltfMorphTargetSelector::Name(name) => Self::Name(name),
        }
    }
}

impl From<&XrdsGltfAnimationSelector> for XrdsSceneGltfAnimationSelector {
    fn from(value: &XrdsGltfAnimationSelector) -> Self {
        match value {
            XrdsGltfAnimationSelector::Index(index) => Self::Index(*index),
            XrdsGltfAnimationSelector::Name(name) => Self::Name(name.clone()),
        }
    }
}

/// Mutate the stored XRDS descriptor behind a spawned entity without exposing internal storage.
///
/// This is intended for expert-level custom updater closures registered through
/// [`XrdsAPI::register_updater`]. It returns `None` when the entity does not carry a stored
/// descriptor of type `C`.
pub fn with_descriptor_mut<C, R>(
    world: &mut World,
    entity: Entity,
    apply: impl FnOnce(&mut C) -> R,
) -> Option<R>
where
    C: XrdsComponent + Send + Sync + 'static,
{
    with_stored_descriptor_mut(world, entity, apply)
}

/// SDK-level keyboard key abstraction for [`XrdsUpdateContext`] input helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XrdsKey {
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    KeyC,
    KeyH,
    KeyL,
    KeyR,
}

impl XrdsKey {
    fn into_bevy(self) -> KeyCode {
        match self {
            Self::Digit1 => KeyCode::Digit1,
            Self::Digit2 => KeyCode::Digit2,
            Self::Digit3 => KeyCode::Digit3,
            Self::Digit4 => KeyCode::Digit4,
            Self::KeyC => KeyCode::KeyC,
            Self::KeyH => KeyCode::KeyH,
            Self::KeyL => KeyCode::KeyL,
            Self::KeyR => KeyCode::KeyR,
        }
    }
}

// ── Handle ────────────────────────────────────────────────────────────────────

/// Typed entity handle returned by [`XrdsAPI::spawn`].
///
/// The generic parameter `C` is the XRDS descriptor type that produced this entity,
/// giving callers type-safe access to the spawned entity without exposing Bevy internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle<C> {
    entity: Entity,
    _marker: PhantomData<fn() -> C>,
}

impl<C> Handle<C> {
    pub fn entity(&self) -> Entity {
        self.entity
    }
}

impl<C> From<Entity> for Handle<C> {
    fn from(entity: Entity) -> Self {
        Self {
            entity,
            _marker: PhantomData,
        }
    }
}

/// SDK-level glue attached to an existing Bevy app.
///
/// Use [`XrdsAPI::attach`] during runtime construction to install XRDS resources and systems,
/// then spawn and mutate XRDS descriptors through this wrapper.
pub struct XrdsAPI<'a> {
    app: &'a mut App,
}

#[derive(Debug, Clone)]
pub(super) struct PendingGltfAnimationRequest {
    pub(super) selector: XrdsGltfAnimationSelector,
    pub(super) options: XrdsGltfAnimationPlaybackOptions,
}

#[derive(Resource, Default)]
pub(super) struct PendingGltfAnimationRequests {
    pub(super) requests: HashMap<Entity, PendingGltfAnimationRequest>,
}

#[derive(Resource, Default)]
pub(super) struct ActiveGltfAnimationStates {
    pub(super) states: HashMap<Entity, XrdsGltfAnimationState>,
}

#[derive(Resource, Default)]
pub(super) struct PendingGltfMorphTargetOverrideRequests {
    pub(super) entities: HashSet<Entity>,
}

/// High-level XRDS application interface.
///
/// This is the default, Bevy-hidden layer for SDK users. `Runtime::run_xrds(...)` adapts this
/// trait into the lower-level runtime and Bevy scheduling model internally.
#[allow(unused_variables)]
pub trait XrdsApp {
    /// Called once before [`setup`] with direct access to the Bevy [`App`].
    ///
    /// Use this to add plugins (e.g. `bevy_egui::EguiPlugin`) or Bevy systems that the
    /// SDK does not otherwise expose.  This fires before XRDS resources are initialised,
    /// so it is safe to add any Bevy plugin here.
    ///
    /// The default implementation does nothing.
    fn configure(&mut self, _app: &mut App) {}

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {}
    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {}
}

#[derive(Resource)]
pub(crate) struct XrdsAppState<A> {
    pub(crate) app: A,
}

pub(crate) fn run_xrds_app_update<A>(world: &mut World)
where
    A: XrdsApp + Send + Sync + 'static,
{
    world.resource_scope(|world, mut state: Mut<'_, XrdsAppState<A>>| {
        let mut ctx = XrdsUpdateContext::new(world);
        state.app.update(&mut ctx);
    });
}
