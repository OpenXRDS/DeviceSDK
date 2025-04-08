#include pbr::fragment_params
#include postproc::types

#define LIGHT_PARAMS_OUTPUT
#include light_params

// From g-buffer
@group(0) @binding(0)
var position_metallic_sampler: sampler;
@group(0) @binding(1)
var position_metallic_texture: texture_2d_array<f32>;

struct ShadowmapOutput {
    @location(0) depth_and_square: vec2<f32>
}

@fragment
fn main(in: SimpleQuadOutput) -> ShadowmapOutput {
    var world_position: vec3<f32> = get_position_metallic(in).rgb;
    var light_space_position: vec4<f32> = u_light.view_proj * vec4<f32>(world_position, 1.0);
    var depth: f32 = light_space_position.z / light_space_position.w;

    var out: ShadowmapOutput;
    out.depth_and_square = vec2<f32>(depth, depth * depth);

    return out;
}

fn get_position_metallic(in: SimpleQuadOutput) -> vec4<f32> {
    return textureSample(position_metallic_texture, position_metallic_sampler, in.uv, in.view_index);
}