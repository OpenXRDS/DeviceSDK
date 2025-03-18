struct DeferredVertexInput {
    @builtin(vertex_index) vertex_index: u32,
// #if VIEW_COUNT > 1
    @builtin(view_index) view_index: i32,
// #endif
}

struct DeferredVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
// #if VIEW_COUNT > 1
    @location(1) view_index: i32,
// #endif
}

struct DeferredFragmentOutput {
    @location(0) final_color: vec4<f32>,
}

// #if VIEW_COUNT > 1
@group(0) @binding(0)
var position_metallic: texture_2d_array<f32>;
@group(0) @binding(2)
var normal_roughness: texture_2d_array<f32>;
@group(0) @binding(4)
var albedo_occlusion: texture_2d_array<f32>;
@group(0) @binding(6)
var emissive: texture_2d_array<f32>;
// #else
// @group(0) @binding(0)
// var position_metallic: texture_2d<f32>;
// @group(0) @binding(2)
// var normal_roughness: texture_2d<f32>;
// @group(0) @binding(4)
// var albedo_occlusion: texture_2d<f32>;
// @group(0) @binding(6)
// var emissive: texture_2d<f32>;
// #endif
@group(0) @binding(1)
var position_metallic_sampler: sampler;
@group(0) @binding(3)
var normal_roughness_sampler: sampler;
@group(0) @binding(5)
var albedo_occlusion_sampler: sampler;
@group(0) @binding(7)
var emissive_sampler: sampler;

@vertex
fn vs_main(in: DeferredVertexInput) -> DeferredVertexOutput {
    var output: DeferredVertexOutput;

    var uv: vec2<f32> = vec2<f32>(f32((in.vertex_index << 1) & 2), f32(in.vertex_index & 2));
    output.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    output.uv = uv;

    return output;
}

@fragment
fn fs_main(in: DeferredVertexOutput) -> DeferredFragmentOutput {
    // Do deferred lighting
    var output: DeferredFragmentOutput;

    output.final_color = get_albedo(in);

    return output;
}

fn get_position(in: DeferredVertexOutput) -> vec4<f32> {
// #if VIEW_COUNT > 1
    return vec4<f32>(textureSample(position_metallic, position_metallic_sampler, in.uv, in.view_index).xyz, 1.0);
// #else
    // return vec4<f32>(textureSample(position_metallic, position_metallic_sampler, in.uv).xyz, 1.0);
// #endif
}

fn get_metallic(in: DeferredVertexOutput) -> f32 {
// #if VIEW_COUNT > 1
    return textureSample(position_metallic, position_metallic_sampler, in.uv, in.view_index).w;
// #else
    // return textureSample(position_metallic, position_metallic_sampler, in.uv).w;
// #endif
}

fn get_normal(in: DeferredVertexOutput) -> vec4<f32> {
// #if VIEW_COUNT > 1
    return vec4<f32>(textureSample(normal_roughness, normal_roughness_sampler, in.uv, in.view_index).xyz, 1.0);
// #else
    // return vec4<f32>(textureSample(normal_roughness, normal_roughness_sampler, in.uv).xyz, 1.0);
// #endif
}

fn get_roughness(in: DeferredVertexOutput) -> f32 {
// #if VIEW_COUNT > 1
    return textureSample(normal_roughness, normal_roughness_sampler, in.uv, in.view_index).w;
// #else
    // return textureSample(normal_roughness, normal_roughness_sampler, in.uv).w;
// #endif
}

fn get_albedo(in: DeferredVertexOutput) -> vec4<f32> {
// #if VIEW_COUNT > 1
    return vec4<f32>(textureSample(albedo_occlusion, albedo_occlusion_sampler, in.uv, in.view_index).xyz, 1.0);
// #else
    // return vec4<f32>(textureSample(albedo_occlusion, albedo_occlusion_sampler, in.uv).xyz, 1.0);
// #endif
}

fn get_occlusion(in: DeferredVertexOutput) -> f32 {
// #if VIEW_COUNT > 1
    return textureSample(albedo_occlusion, albedo_occlusion_sampler, in.uv, in.view_index).w;
// #else
    // return textureSample(albedo_occlusion, albedo_occlusion_sampler, in.uv).w;
// #endif
}

fn get_emissive(in: DeferredVertexOutput) -> vec4<f32> {
// #if VIEW_COUNT > 1
    return textureSample(emissive, emissive_sampler, in.uv, in.view_index);
// #else
    // return textureSample(emissive, emissive_sampler, in.uv);
// #endif
}