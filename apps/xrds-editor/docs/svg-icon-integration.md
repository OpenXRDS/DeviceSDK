# SVG Icon Integration in xrds-editor

## Overview

This document describes how to integrate SVG icons from the `assets/icons` directory into the xrds-editor's palette and menubar components.

## Available Icons

SVG icons are organized by category in the `assets/icons` directory:

- **lights/** - Light-related icons (ambient.svg, area.svg, directional.svg, point.svg, shadow.svg, spot.svg)
- **materials/** - Material-related icons (material.svg, normal.map.svg, pbr.svg, texture.svg, uv.map.svg)  
- **primitives/** - Primitive geometry icons (capsule.svg, cone.svg, cube.svg, cylinder.svg, mesh.svg, plane.svg, sphere.svg, torus.svg)
- **scene&hierarchy/** - Scene and hierarchy icons (empty.svg, group.svg, hidden.svg, layers.svg, lock.svg, prefab.svg, scene.svg, visible.svg)
- **shaders/** - Shader icons (shader.graph.svg)
- **viewport&camera/** - Viewport and camera icons (camera.svg, frame.sel.svg, grid.svg, orthographic.svg, perspective.svg, shaded.svg, wireframe.svg, zoom.svg)
- **workflow&assets/** - Workflow and asset icons (asset.svg, build.svg, code.svg, export.svg, import.svg, settings.svg)

## Implementation Approach

### Palette Integration

The palette currently uses emoji characters as placeholders. To integrate SVG icons:

1. Add egui_extras or similar dependency for SVG support
2. Implement asset loading for SVG files in the editor context
3. Replace emoji icons with actual SVG icon references based on primitive type

### Menubar Integration

Menubar items are text-based. For SVG integration:

1. Add SVG loading capability to the menubar context
2. Update menu button labels to include SVG icons where appropriate
3. Consider using dedicated icon-only buttons for certain menu items

## Example Implementation Pattern

```rust
// In palette.rs, replace emoji with:
// "Empty" -> assets/icons/primitives/empty.svg
// "Cube" -> assets/icons/primitives/cube.svg  
// "Sphere" -> assets/icons/primitives/sphere.svg
// etc.
```

## Technical Considerations

- SVG loading may require additional dependencies (egui_extras, image loading crates)
- Ensure proper asset paths are used for cross-platform compatibility
- Consider performance implications of loading many icons at once
- Maintain backward compatibility with current UI elements

## Implementation Steps

1. Add required dependencies to Cargo.toml
2. Implement asset loading system for SVGs in editor context
3. Create icon mapping functions that return appropriate SVG paths
4. Update UI components to use loaded SVG assets instead of emojis
5. Test across different platforms and resolutions