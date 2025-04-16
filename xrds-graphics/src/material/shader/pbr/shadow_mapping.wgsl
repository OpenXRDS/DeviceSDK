#include pbr::vertex_params
#include postproc::types
#include common::light_params

struct ShadowmapOutput {
    @location(0) depth_and_square: vec2<f32>
}

@fragment
fn main(in: VertexOutput) -> ShadowmapOutput {
    var depth: f32 = in.position.z / in.position.w;
    
    var out: ShadowmapOutput;
    out.depth_and_square = vec2<f32>(depth, depth * depth);

    return out;
}