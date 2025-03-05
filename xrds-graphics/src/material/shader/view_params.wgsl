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

@group(0) @binding(0)
#if VIEW_COUNT > 1
var<uniform> u_view_params: array<ViewParams, #{VIEW_COUNT}>;
#else
var<uniform> u_view_params: ViewParams;
#endif

#ifdef PUSH_CONSTANT_SUPPORTED
var<push_constant> u_local_model: mat4x4<f32>;
#else
@group(0) @binding(1)
var<uniform> u_local_model: mat4x4<f32>;
#endif

fn get_view_params(view_index: i32) -> ViewParams {
#if VIEW_COUNT > 1
    return u_view_params[view_index];
#else
    return u_view_params;
#endif
}

fn get_local_model() -> mat4x4<f32> {
    return u_local_model;
}