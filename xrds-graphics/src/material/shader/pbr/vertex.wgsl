#include common::utils
#include common::skinning
#ifdef SHADOW_MAPPING
#include common::light_params
#else
#include common::view_params
#endif
#include pbr::vertex_params

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    var curr_instance_model = get_curr_instance_model(in);
    var local_model = get_local_model();
    var skinning_model = get_skinning(in);

    var curr_transform = curr_instance_model * local_model * skinning_model;
    var curr_pos = curr_transform * vec4<f32>(in.position, 1.0);

#ifdef SHADOW_MAPPING
    var light = get_light();
    out.position = light.view_proj * curr_pos;
#else
    var view_params = get_view_params(in.view_index);
    var clip_position = view_params.curr_view_projection * curr_pos;
    out.position = clip_position;  // position for fragment shader
    out.curr_position = clip_position;  // position for calculation in fragment shader

    // Calculate previous position for motion vector
    var prev_instance_model = get_prev_instance_model(in);
    var prev_transform = prev_instance_model * local_model * skinning_model;
    var prev_pos = prev_transform * vec4<f32>(in.position, 1.0);
    out.prev_position = view_params.prev_view_projection * prev_pos;
    out.curr_jitter = view_params.curr_jitter;
    out.prev_jitter = view_params.prev_jitter;
#endif // SHADOW_MAPPING

    out.view_index = in.view_index;
    out.world_position = curr_pos.xyz;

#ifdef VERTEX_INPUT_TEXCOORD_0
    out.texcoord_0 = in.texcoord_0;
#ifdef VERTEX_INPUT_TEXCOORD_1
    out.texcoord_1 = in.texcoord_1;
#endif  // VERTEX_INPUT_TEXCOORD_1
#endif  // VERTEX_INPUT_TEXCOORD_2

#ifdef VERTEX_INPUT_COLOR
#ifdef VERTEX_INPUT_COLOR_3CH
    out.color = vec4<f32>(in.color, 1.0);
#else  // VERTEX_INPUT_COLOR_3CH
    out.color = in.color;
#endif  // VERTEX_INPUT_COLOR_3CH
#endif  // VERTEX_INPUT_COLOR

    var model_3x3 = mat3x3<f32>(curr_transform[0].xyz, curr_transform[1].xyz, curr_transform[2].xyz);
    var normal_mat = inverse_transpose_3x3(model_3x3);

#ifdef VERTEX_INPUT_NORMAL
    // out.normal = in.normal;
    out.normal = normalize(normal_mat * in.normal);
#endif  // VERTEX_INPUT_NORMAL

#ifdef VERTEX_INPUT_TANGENT
    // out.tangent = in.tangent;
    var tangent_xyz_world = normalize(normal_mat * in.tangent.xyz);
    out.tangent = vec4<f32>(tangent_xyz_world, in.tangent.w);
#endif  // VERTEX_INPUT_TANGENT
    
    return out;
}

fn get_skinning(in: VertexInput) -> mat4x4<f32> {
#ifdef VERTEX_INPUT_SKINNED
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_0
    var skinning_model = get_skinning_model(in.joints_0, in.weights_0);
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_1
    skinning_model += get_skinning_model(in.joints_1, in.weights_1);
#endif
    return skinning_model;
#endif
#else
    return mat4x4<f32>(
        vec4(1.0, 0.0, 0.0, 0.0),
        vec4(0.0, 1.0, 0.0, 0.0),
        vec4(0.0, 0.0, 1.0, 0.0),
        vec4(0.0, 0.0, 0.0, 1.0)
    );
#endif
}
