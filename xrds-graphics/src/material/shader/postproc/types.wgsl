#ifndef POSTPROC_TYPES_WGSL
#define POSTPROC_TYPES_WGSL

struct SimpleQuadOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) view_index: i32,
}

#endif // POSTPROC_TYPES_WGSL