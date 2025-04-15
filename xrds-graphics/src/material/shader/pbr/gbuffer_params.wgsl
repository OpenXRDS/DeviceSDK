#ifndef GBUFFER_PARAMS
#define GBUFFER_PARAMS

#ifndef GBUFFER_PARAMS_GROUP_INDEX
#define GBUFFER_PARAMS_GROUP_INDEX 1
#endif

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(0)
var position_metallic_sampler: sampler;

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(1)
var position_metallic_texture: texture_2d_array<f32>;

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(2)
var normal_roughness_sampler: sampler;

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(3)
var normal_roughness_texture: texture_2d_array<f32>;

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(4)
var albedo_occlusion_sampler: sampler;

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(5)
var albedo_occlusion_texture: texture_2d_array<f32>;

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(6)
var emissive_sampler: sampler;

@group(${GBUFFER_PARAMS_GROUP_INDEX}) @binding(7)
var emissive_texture: texture_2d_array<f32>;

fn get_position_metallic(uv: vec2<f32>, view_index: i32) -> vec4<f32> {
    return textureSample(position_metallic_texture, position_metallic_sampler, uv, view_index);
}

fn get_normal_roughness(uv: vec2<f32>, view_index: i32) -> vec4<f32> {
    return textureSample(normal_roughness_texture, normal_roughness_sampler, uv, view_index);
}

fn get_albedo_occlusion(uv: vec2<f32>, view_index: i32) -> vec4<f32> {
    return textureSample(albedo_occlusion_texture, albedo_occlusion_sampler, uv, view_index);
}

fn get_emissive(uv: vec2<f32>, view_index: i32) -> vec4<f32> {
    return textureSample(emissive_texture, emissive_sampler, uv, view_index);
}

#endif  // GBUFFER_PARAMS