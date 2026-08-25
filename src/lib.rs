//! # DeviceSDK — build XR applications without learning Bevy
//!
//! `xrds` is the entry point to the OpenXRDS DeviceSDK. It targets non-experts
//! first: spawning objects, moving them, handling input and loading assets are
//! expressed as XRDS concepts, and Bevy stays underneath as the implementation
//! engine rather than as the interface you have to learn.
//!
//! ```no_run
//! use xrds::sdk::{primitives::XrdsCube, world::XrdsCamera};
//! use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};
//!
//! #[derive(Default)]
//! struct MyApp;
//!
//! impl XrdsApp for MyApp {
//!     fn setup(&mut self, api: &mut XrdsAPI<'_>) {
//!         api.spawn(&XrdsCamera::new().looking_at([0.0, 0.0, 0.0]));
//!         api.spawn(&XrdsCube::new().with_name("Hello"));
//!     }
//!
//!     fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
//! }
//!
//! Runtime::new(RuntimeParameters::default()).run_xrds(MyApp::default());
//! ```
//!
//! A complete version of this is `examples/xrds_first/simple_api.rs`:
//! `cargo run --example simple_api`.
//!
//! # Two layers, and how to tell which one you are in
//!
//! **The default layer** is [`XrdsApp`], [`XrdsAPI`] and [`XrdsUpdateContext`].
//! `setup` runs once; `update` runs every frame. [`XrdsAPI::spawn`] returns a typed
//! [`Handle`] that later calls take, so a cube handle cannot be passed where a light
//! is meant.
//!
//! **The expert layer** is `RuntimeHandler` and direct Bevy systems, for
//! engine-level control. It stays available and deliberately does not shape the
//! default path.
//!
//! The two are not re-exported into one namespace: **this crate does not re-export
//! Bevy**. If you drop to the expert layer, add `bevy` to your own `Cargo.toml` and
//! import it explicitly. That is a deliberate boundary — reaching Bevy through XRDS
//! would make it ambiguous which layer a given line of code belongs to.
//!
//! # Runtime types and scene-document types
//!
//! Most descriptors exist as a pair, distinguished by an `XrdsScene` prefix, and
//! choosing between them is the question newcomers hit first:
//!
//! - [`XrdsCamera`](sdk::world::XrdsCamera), `XrdsCube`, `XrdsPointLight` — a **live
//!   runtime object** you spawn or edit through [`XrdsAPI`].
//! - `XrdsSceneNode`, `XrdsSceneCube`, `XrdsScenePointLight` — **authored scene
//!   data** that must survive save/load and import/export.
//!
//! The rule holds for every pair. Reach for [`scene_graph`] only when you need a
//! durable document model with stable ids, hierarchy, editor metadata and
//! round-trip persistence; ordinary app code should stay with the runtime-facing
//! types and keep the handles `spawn` returns.
//!
//! Scene environment policy — image-based lighting, skybox, exposure, fog,
//! atmosphere — follows the same rule. Author it in the document when it is part of
//! the saved scene's meaning, or set it at runtime through
//! [`XrdsAPI::set_scene_environment`] when live app logic owns it.
//!
//! # What is in the box
//!
//! Primitives, glTF import, PBR materials with texture slots, the four light types,
//! physics (rigid bodies, colliders, raycasting, grab and throw), spatial audio,
//! particle effects, in-world UI panels, interaction zones, Tracks for
//! trigger-driven choreography, and scene save/load.
//!
//! Scene *export* to glTF is retired: glTF cannot represent panels, triggers,
//! Tracks or anchors, so it silently dropped them. glTF **import** is unaffected.
//!
//! # Platforms
//!
//! Windows and Linux for the editor, SDK and exported apps; macOS planned. Meta
//! Quest 3 and Pro are supported for the SDK and exported apps — Quest 2 is not
//! (the baseline is API 32). Android XR has placeholder build configuration.
//!
//! # Where to look next
//!
//! - [`XrdsAPI`] — the main surface, and the one to browse first.
//! - [`XrdsUpdateContext`] — the same vocabulary, available per frame.
//! - [`scene_graph`] — the authored-document layer.
//! - `docs/getting-started.md` in the repository, for build requirements and
//!   choosing between the editor, the SDK and the expert layer.

pub use xrds_openxr::*;
pub use xrds_runtime::*;

/// Authored scene-document layer.
/// Use this for save/load, import/export, and stable scene data; normal runtime-first SDK code
/// should usually work through `XrdsAPI` and runtime-facing XRDS types instead.
pub use xrds_scene_graph as scene_graph;
