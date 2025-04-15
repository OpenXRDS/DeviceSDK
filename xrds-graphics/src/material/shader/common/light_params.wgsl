#ifndef LIGHT_PARAMS_WGSL
#define LIGHT_PARAMS_WGSL

#ifndef LIGHT_PARAMS_GROUP_INDEX
#define LIGHT_PARAMS_GROUP_INDEX 0
#endif

const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
const LIGHT_TYPE_POINT: u32 = 1;
const LIGHT_TYPE_SPOT: u32 = 2;

// 256-bytes padding
struct Light {
    view: mat4x4<f32>,      // 64
    view_proj: mat4x4<f32>, // 128
    direction: vec3<f32>,   // 140
    range: f32,             // 144
    color: vec3<f32>,       // 156
    intensity: f32,         // 160
    position: vec3<f32>,    // 172
    ty: u32,                // 176
    inner_cons_cos: f32,    // 180
    outer_cons_cos: f32,    // 184
    cast_shadow: u32,       // 188
    shadow_map_index: u32,  // 192
    _pad: mat4x4<f32>       // 256
}

#ifdef SHADOW_MAPPING

@group(${LIGHT_PARAMS_GROUP_INDEX}) @binding(0)
var<storage, read> s_light_data: array<Light>;

fn get_light() -> Light {
    return s_light_data[0];
}

#else

struct LightSystemParams {
    light_count: u32,
}

@group(${LIGHT_PARAMS_GROUP_INDEX}) @binding(0)
var<storage, read> s_light_data: array<Light>;

@group(${LIGHT_PARAMS_GROUP_INDEX}) @binding(1)
var<uniform> u_light_params: LightSystemParams;

@group(${LIGHT_PARAMS_GROUP_INDEX}) @binding(2)
var shadowmap_sampler: sampler;

@group(${LIGHT_PARAMS_GROUP_INDEX}) @binding(3)
var shadowmaps: binding_array<texture_2d<f32>, 64>;

fn get_light_count() -> u32 {
    return u_light_params.light_count;
}

fn get_light_ith(i: i32) -> Light {
    return s_light_data[i];
}

fn get_shadowmap(i: i32, uv: vec2<f32>) -> vec2<f32> {
    return textureSample(shadowmaps[i], shadowmap_sampler, uv).rg;
}

#endif

#endif  // LIGHT_PARAMS_WGSL