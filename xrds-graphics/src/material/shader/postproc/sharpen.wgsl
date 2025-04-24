#include postproc::types

@group(0) @binding(0)
var input_sampler: sampler;
@group(0) @binding(1)
var input_texture: texture_2d_array<f32>;

// TODO: parameterized
const SHARPEN_STRENGTH: f32 = 0.1;

@fragment
fn fs_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    let tex_dims = vec2<f32>(textureDimensions(input_texture).xy);
    
    let texel_size = 1.0 / tex_dims;
    let center_color = textureSample(input_texture, input_sampler, in.uv, in.view_index);
    let left_color = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-texel_size.x, 0.0), in.view_index);
    let right_color = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(texel_size.x, 0.0), in.view_index);
    let top_color = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, -texel_size.y), in.view_index);
    let bottom_color = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, texel_size.y), in.view_index);

    let sharpened_rgb = center_color.rgb + SHARPEN_STRENGTH * (
        4.0 * center_color.rgb - (top_color.rgb + bottom_color.rgb + left_color.rgb + right_color.rgb)
    );

    let final_color_rgb = clamp(sharpened_rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    // Return center_color.a for dynamic_alpha value for next frame
    return vec4<f32>(final_color_rgb, center_color.a);
}