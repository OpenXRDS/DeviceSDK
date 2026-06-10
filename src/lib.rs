pub use xrds_internal::*;

/// Authored scene-document layer.
/// Use this for save/load, import/export, and stable scene data; normal runtime-first SDK code
/// should usually work through `XrdsAPI` and runtime-facing XRDS types instead.
pub use xrds_scene_graph as scene_graph;
