#ifndef POSTPROC_TYPES_WGSL
#define POSTPROC_TYPES_WGSL

struct SimpleQuadInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
}

struct SimpleQuadOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) view_index: i32,
}

const CONST_PI = 3.14159265359;

#endif // POSTPROC_TYPES_WGSL