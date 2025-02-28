#define_import_path shader::view_params

struct ViewParams {
    view_projection: mat4x4<f32>,
    inv_view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    projection: mat4x4<f32>,
    inv_projection: mat4x4<f32>,
    world_position: vec3<f32>,
    width: u32,
    height: u32,
}

#if VIEW_COUNT > 1
@group(0) @binding(0)
var<uniform> u_view_params: array<ViewParams, #{VIEW_COUNT}>;
#else
@group(0) @binding(0)
var<uniform> u_view_params: ViewParams;
#endif