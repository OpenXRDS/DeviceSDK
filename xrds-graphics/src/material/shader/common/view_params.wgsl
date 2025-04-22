#ifndef VIEW_PARAMS_WGSL
#define VIEW_PARAMS_WGSL

#ifndef VIEW_PARAMS_GROUP_INDEX
#define VIEW_PARAMS_GROUP_INDEX 0
#endif

struct ViewParams {
    curr_view_projection: mat4x4<f32>,
    prev_view_projection: mat4x4<f32>,
    curr_jitter: vec2<f32>,
    prev_jitter: vec2<f32>,
    inv_view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    projection: mat4x4<f32>,
    inv_projection: mat4x4<f32>,
    world_position: vec3<f32>,
    _pad: u32,
}

@group(${VIEW_PARAMS_GROUP_INDEX}) @binding(0)
var<uniform> u_view_params: array<ViewParams, 2>;

fn get_view_params(view_index: i32) -> ViewParams {
    return u_view_params[view_index];
}

#endif  // VIEW_PARAMS_WGSL