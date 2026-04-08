# xrds-scene-graph

`xrds-scene-graph` is the authored scene-document layer for XRDS.

Its job is to hold the durable, serializable, editor-facing meaning of a scene before that scene is projected into the live runtime. This crate is part of the SDK basement for a future editor, not the editor application itself.

## Purpose

This crate exists so XRDS has one canonical document model for:

- stable node ids and authored hierarchy
- editor metadata that should survive save/load
- built-in scene node payloads such as cameras, lights, primitives, and glTF references
- asset catalog references and fallback asset URIs
- save/load, validation, and undo/redo-friendly document workflows

In practice, `xrds-scene-graph` is where scene meaning lives when that meaning should be preserved across serialization, import/export, and future editor sessions.

## What It Owns

The crate owns document-side responsibilities such as:

- `XrdsSceneDocument` as the root authored scene file shape
- `XrdsSceneNode` and built-in payload types
- JSON persistence helpers
- document validation rules
- document-session helpers for save/load and undo/redo
- asset placement, rebinding, removal, rename, and diagnostics at the document layer

## What It Does Not Own

This crate is intentionally not:

- a Bevy runtime scene graph
- a GUI editor framework
- a viewport interaction layer
- a replacement for `xrds-runtime`

The future GUI editor should own selection, hierarchy widgets, inspectors, drag state, viewport tools, and other interaction concerns. `xrds-runtime` should own projection of authored scene data into the live engine world.

## Relationship To `xrds-runtime`

The normal boundary is:

1. Author or load a document in `xrds-scene-graph`.
2. Validate and manipulate it through document/session APIs.
3. Convert it into runtime nodes.
4. Import those nodes into `xrds-runtime`.

That means policy that affects saved scene meaning should generally live here, while policy that affects live engine application should generally live in `xrds-runtime`.

## Current Scope

Today, the crate supports:

- built-in scene document types
- document-to-runtime conversion for the built-in XRDS scene set
- JSON save/load
- document-session save/load and undo/redo
- document-level glTF asset workflows
- asset health and usage diagnostics for future editor UI consumption

## Design Intent

The design goal is to give a future GUI editor a strong basement:

- enough structure that the GUI does not need to invent scene semantics
- enough workflow support that the GUI does not need to duplicate asset policy
- enough persistence support that authored state round-trips cleanly

If a rule changes the meaning of saved scene data, it should usually be implemented in `xrds-scene-graph` first.