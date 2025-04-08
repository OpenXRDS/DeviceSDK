#ifndef LIGHT_PARAMS_WGSL
#define LIGHT_PARAMS_WGSL

const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
const LIGHT_TYPE_POINT: u32 = 1;
const LIGHT_TYPE_SPOT: u32 = 2;

struct Light {
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    direction: vec3<f32>,
    range: f32,
    color: vec3<f32>,
    intensity: f32,
    position: vec3<f32>,
    ty: u32,
    inner_cons_cos: f32,
    outer_cons_cos: f32,
    cast_shadow: u32,
    shadow_map_index: u32,
}

// @group(3) @binding(0)
// var<uniform> u_light: array<Light>;

#endif  // LIGHT_PARAMS_WGSL