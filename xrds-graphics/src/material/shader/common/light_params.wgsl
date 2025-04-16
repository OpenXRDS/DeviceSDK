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

// VSM parameters
// const MIN_VSM_VARIANCE = 0.00002;
const MIN_VSM_VARIANCE = 0.0003;
const LIGHT_BLEED_REDUCTION_FACTOR = 0.3;

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
var shadowmaps: binding_array<texture_2d<f32>, 32>;

fn get_light_count() -> u32 {
    return u_light_params.light_count;
}

fn get_light_ith(i: i32) -> Light {
    return s_light_data[i];
}

fn calculate_vsm_shadow(shadowmap_index: i32, uv: vec2<f32>, fragment_depth_from_light: f32, min_variance: f32, light_bleed_reduction: f32) -> f32 {
    let shadowmap_value = textureSample(shadowmaps[shadowmap_index], shadowmap_sampler, uv).rg;
    let M1 = shadowmap_value.r; // E[depth]
    let M2 = shadowmap_value.g; // E[depth^2]

    // Calculate variance, clamping to a minimum value to avoid issues
    var variance = M2 - M1 * M1;
    variance = max(variance, min_variance);

    // Calculate the difference in depth between the fragment and the average depth in the shadow map
    let delta = fragment_depth_from_light - M1;

    // Chebyshev's inequality: P(x >= t) <= variance / (variance + (t - E[x])^2)
    // This gives an upper bound on the probability that the fragment is occluded.
    // We use a common VSM formulation which estimates visibility directly.
    // The max(0.0, delta) term is crucial for reducing light bleeding, preventing
    // surfaces closer to the light than the average occluder depth from being shadowed.
    // A more advanced light bleed reduction can be used here if needed.
    let delta_no_bleed = max(0.0, delta); // Basic light bleed reduction
    // let visibility = variance / (variance + delta_no_bleed * delta_no_bleed);

    // Alternative light bleed reduction (can be smoother):
    let amount = delta * light_bleed_reduction; // Adjust falloff
    let visibility = smoothstep(0.0, 1.0, variance / (variance + delta_no_bleed * delta_no_bleed)); // Apply smoothing

    // The result might slightly exceed 1.0 due to filtering/precision, clamp it.
    return saturate(visibility);
}

fn calculate_shadow(light: Light, world_position: vec3<f32>) -> f32 {
    var shadow_factor = 1.0;
    if light.cast_shadow == 1u && light.shadow_map_index < 32u {
        let light_clip_pos = light.view_proj * vec4<f32>(world_position, 1.0);

        if light_clip_pos.w > 0.0 {
            let light_ndc = light_clip_pos.xyz / light_clip_pos.w;
            let shadow_uv = light_ndc.xy * vec2(0.5, -0.5) + vec2(0.5, 0.5);

            if shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0 && shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0 &&
               light_ndc.z >= 0.0 && light_ndc.z <= 1.0 {
                let fragment_depth_from_light = light_ndc.z;
                shadow_factor = calculate_vsm_shadow(i32(light.shadow_map_index),
                                                     shadow_uv,
                                                     fragment_depth_from_light,
                                                     MIN_VSM_VARIANCE,
                                                     LIGHT_BLEED_REDUCTION_FACTOR);
            }
        }
    }

    return shadow_factor;
}

#endif

#endif  // LIGHT_PARAMS_WGSL