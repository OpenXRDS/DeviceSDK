#include postproc::types

@group(0) @binding(0)
var input_sampler: sampler;
@group(0) @binding(1)
var input_texture: texture_2d_array<f32>;

@fragment
fn main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    var color = textureSample(input_texture, input_sampler, in.uv, in.view_index);
    // Color may contains alpha information for next frame. So force output alpha as 1.0
    return vec4<f32>(color.rgb, 1.0);
    // return vec4<f32>(color.a, color.a, color.a, 1.0);
}