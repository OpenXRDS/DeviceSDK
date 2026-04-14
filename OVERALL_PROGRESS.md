# DeviceSDK Overall Progress

Last updated: 2026-04-13

## Project Goal

Provide a non-expert-first SDK to build XR applications, with:

- a simple default application surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`)
- a durable scene document model (`xrds-scene-graph`)
- an expert escape hatch for direct engine-level control when needed

## Overall Completion (Estimated)

Estimated overall progress toward a strong SDK basement for XR applications: **78%**.

Notes on the estimate:

- this percentage reflects implementation and validation coverage already present in the repo
- it does not mean feature-complete for a full production editor/platform product

## General 3D Editor Backend Progress (Excluding GUI)

Estimated progress toward a general 3D content editor backend, excluding GUI: **75%**.

Why this estimate:

- the document model already supports durable ids, hierarchy, validation, save/load, and import/export
- the runtime bridge already supports meaningful live-scene editing through XRDS APIs
- editor-basement foundations are largely in place, but broader palette coverage and workflow polish are still needed

What is already strong enough for an editor backend:

- scene graph and hierarchy foundation
- stable identity and document persistence
- runtime import/export bridge
- baseline transform, visibility, material, and light editing
- asset catalog baseline and scene environment policy support

What still keeps this from being editor-backend complete:

- broader built-in content/component coverage for general 3D editing
- wider asset-type support beyond current strongest workflows
- deeper material/render authoring policy for inspector-driven editing
- more integration hardening across crates and real editor workflows
- non-expert naming and workflow polish

## Progress Breakdown

| Area | Status | Percent |
| --- | --- | --- |
| Core SDK surface (`XrdsApp`, `XrdsAPI`, `XrdsUpdateContext`) | Stable baseline and widely used in examples/tests | **82%** |
| Scene document model (`xrds-scene-graph`) | Durable ids, hierarchy, validation, save/load, import/export all in place | **85%** |
| Runtime projection (`xrds-runtime`) | Strong baseline for built-in descriptors and scene policy projection | **80%** |
| Scene environment policy | IBL + skybox + exposure + linear fog implemented and tested in both control paths | **92%** |
| Asset workflow | glTF and texture/image workflows are strong; aliasing policy now improved | **76%** |
| Editor-basement readiness | Most checklist items complete; remaining gaps are feature breadth and UX polish | **74%** |
| Docs/examples/test coverage | Good practical examples and broad test coverage in key crates | **79%** |

## Completed Highlights

- Non-expert-first SDK layering is established and documented.
- Runtime-first and document-first flows both work and are tested.
- Scene document round-trip and runtime import/export are operational.
- Scene environment policy is delivered end-to-end:
	- document-driven authoring
	- runtime-driven policy
	- validated runtime projection
- Asset alias policy now supports intentional same-source variant usage by asset id in runtime and document workflows.

## Missing Parts / Remaining Work

### 1) Feature breadth for a fuller SDK palette

- broaden built-in primitive/component coverage beyond the current baseline
- evaluate capsule/character-oriented defaults if target apps need them

### 2) Asset-type expansion beyond current strong glTF baseline

- prioritize first non-glTF asset expansion plan (narrow scope first)
- define which asset classes are next (for example audio/media/material preset flows)

### 3) Advanced material and rendering authoring policy

- keep non-expert defaults simple while expanding advanced controls safely
- continue isolating expert-only engine-shaped controls from default SDK flows

### 4) Editor UX and naming polish

- improve simple-path naming consistency for non-expert mental models
- keep expert terminology and escape hatches clearly separated

### 5) Cross-crate hardening for release readiness

- extend integration-style tests across runtime/document/network/openxr paths
- add more performance/stability validation scenarios under realistic workloads

## Suggested Next Milestones (Short Horizon)

1. Finalize first non-glTF asset-kind expansion plan and implement the smallest viable slice.
2. Expand advanced material authoring in XRDS terms (without exposing engine detail by default).
3. Add cross-crate integration tests focused on import/edit/export + runtime policy updates.
4. Complete naming/UX pass for non-expert-first API clarity.
