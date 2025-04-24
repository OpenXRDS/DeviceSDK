#include postproc::types

@group(0) @binding(0)
var final_color_sampler: sampler;
@group(0) @binding(1)
var final_color: texture_2d_array<f32>;
@group(1) @binding(0)
var history_color_sampler: sampler;
@group(1) @binding(1)
var history_color: texture_2d_array<f32>;
@group(2) @binding(0)
var motion_vector_sampler: sampler;
@group(2) @binding(1)
var motion_vector: texture_2d_array<f32>;

const RADIUS: i32 = 1;
const RADIUS_FLOAT: f32 = 1.0;

@fragment
fn fs_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    // Sampling data
    var curr = textureSample(final_color, final_color_sampler, in.uv, in.view_index);
    var mv = textureSample(motion_vector, motion_vector_sampler, in.uv, in.view_index);
    var texel_size = 1.0 / vec2<f32>(textureDimensions(final_color).rg);

    var vsum = vec3<f32>(0.0);
    var vsum2 = vec3<f32>(0.0);
    var wsum = 0.0;

    let radius_plus_1_sq = (RADIUS_FLOAT + 1.0) * (RADIUS_FLOAT + 1.0);
    let gaussian_falloff = -3.0 / radius_plus_1_sq;

    for (var y: i32 = -RADIUS; y <= RADIUS; y = y + 1) {
        for (var x: i32 = -RADIUS; x <= RADIUS; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let neighbor_uv = in.uv + offset * texel_size;
            let neighbor_color = textureSample(final_color, final_color_sampler, neighbor_uv, in.view_index).rgb;

            let dist_sq = dot(offset, offset);
            let w = exp(gaussian_falloff * dist_sq);

            vsum = vsum + neighbor_color * w;
            vsum2 = vsum2 + neighbor_color * neighbor_color * w;
            wsum = wsum + w;
        }
    }

    if (wsum < 0.00001) {
        wsum = 1.0;
    }

    let mean = vsum / wsum;
    let variance = max(vsum2 / wsum - mean * mean, vec3<f32>(0.0));
    let std_dev = sqrt(variance);
    

    let velocity_length = length(mv.rg / texel_size);
    let box_factor = smoothstep(2.0, 0.5, velocity_length);
    let box_size = mix(0.5, 2.5, box_factor);

    let nmin = mean - std_dev * box_size;
    let nmax = mean + std_dev * box_size;
    let prev_uv = in.uv - mv.rg;
    let hist = textureSample(history_color, history_color_sampler, prev_uv, in.view_index).rgb;
    let clamped_hist = clamp(hist, nmin, nmax);

    let color_diff = length(curr.rgb - clamped_hist.rgb);
    let change_factor = saturate(color_diff * 5.0);
    let dynamic_alpha = mix(0.05, 1.0, change_factor);

    let final_color = mix(clamped_hist, curr.rgb, dynamic_alpha);

    return vec4<f32>(final_color.rgb, 1.0);
}