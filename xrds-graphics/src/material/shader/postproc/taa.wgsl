#include postproc::types


@group(0) @binding(0)
var final_color_sampler: sampler;
@group(0) @binding(1)
var final_color: texture_2d_array<f32>;
@group(1) @binding(0)
var histroy_color_sampler: sampler;
@group(1) @binding(1)
var history_color: texture_2d_array<f32>;
@group(2) @binding(0)
var motion_vector_sampler: sampler;
@group(2) @binding(1)
var motion_vector: texture_2d_array<f32>;

const ALPHA: f32 = 0.1;
const NEIGHBOORS: array<vec2<f32>, 8> = array<vec2<f32>, 8>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(-1.0, 0.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(0.0, -1.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
);

fn clamp_aabb(value: vec4<f32>, aabb_min: vec4<f32>, aabb_max: vec4<f32>) -> vec4<f32> {
    return max(aabb_min, min(aabb_max, value));
}

@fragment
fn fs_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    // Sampling data
    var curr = textureSample(final_color, final_color_sampler, in.uv, in.view_index);
    var mv = textureSample(motion_vector, motion_vector_sampler, in.uv, in.view_index);

    // Reproject History
    let prev_uv = in.uv - mv.rg;
    let hist = textureSample(history_color, histroy_color_sampler, prev_uv, in.view_index);

    // Neighborhood Clamping
    let tex_dims = vec2<f32>(textureDimensions(final_color, 0).xy);
    let texel_size = vec2<f32>(1.0 / tex_dims.x, 1.0 / tex_dims.y);

    var min_color = curr;
    var max_color = curr;

    // Sample 8 colors from neighbors
    for (var i = 0; i < 8; i = i + 1) {
        let offset = NEIGHBOORS[i] * texel_size;
        let neighbor_uv = in.uv + offset;
        let neighbor_color = textureSample(final_color, final_color_sampler, neighbor_uv, in.view_index);

        min_color = min(min_color, neighbor_color);
        max_color = max(max_color, neighbor_color);
    }

    // Clamp history color to the AABB
    let clamped_hist = clamp_aabb(hist, min_color, max_color);

    let color_diff = length(curr.rgb - clamped_hist.rgb);
    let change_factor = saturate(color_diff * 5.0);
    let dynamic_alpha = mix(0.05, 1.0, change_factor);

    // Blend curr and hist
    let final_color = mix(clamped_hist, curr, 0.3);

    return vec4<f32>(final_color.rgb, 1.0);
}