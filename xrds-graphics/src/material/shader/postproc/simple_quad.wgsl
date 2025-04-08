#include postproc::types

struct SimpleQuadInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
}

@vertex
fn main(in: SimpleQuadInput) -> SimpleQuadOutput {
    var output: SimpleQuadOutput;

    var uv: vec2<f32> = vec2<f32>(f32((in.vertex_index << 1) & 2), f32(in.vertex_index & 2));
    output.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);

    // Flip uv for wgpu texture coordinates correction
    uv.y = 1.0 - uv.y;
    output.uv = uv;
    output.view_index = in.view_index;

    return output;
}