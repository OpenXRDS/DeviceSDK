#define_import_path shader::view_params

struct ViewParams {
    view_projection: mat4x4<f32>,
    inv_view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    projection: mat4x4<f32>,
    inv_projection: mat4x4<f32>,
    world_position: vec3<f32>,
    _pad: u32,
}

@group(0) @binding(0)
var<uniform> u_view_params: array<ViewParams, 2>;
var<push_constant> u_local_model: mat4x4<f32>;

fn get_view_params(view_index: i32) -> ViewParams {
    return u_view_params[view_index];
}

fn get_local_model() -> mat4x4<f32> {
    return u_local_model;
}