#include common::skinning
#include common::view_params
#include pbr::vertex_params

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    var transform_mat = 
        get_instance_model(in) * // world model
        get_local_model() * // local model
        get_skinning(in); // skinned mesh
       
    var pos = transform_mat * vec4<f32>(in.position, 1.0);
    var view_params = get_view_params(in.view_index);

    out.position = view_params.view_projection * pos;
    out.view_index = in.view_index;
    out.world_position = pos.xyz; // / pos.w;
#ifdef VERTEX_INPUT_TEXCOORD_0
    out.texcoord_0 = in.texcoord_0;
#ifdef VERTEX_INPUT_TEXCOORD_1
    out.texcoord_1 = in.texcoord_1;
#endif
#endif
#ifdef VERTEX_INPUT_COLOR
#ifdef VERTEX_INPUT_COLOR_3CH
    out.color = vec4<f32>(in.color, 1.0);
#else
    out.color = in.color;
#endif
#endif
#ifdef VERTEX_INPUT_NORMAL
    out.normal = in.normal;
#endif
#ifdef VERTEX_INPUT_TANGENT
    out.tangent = in.tangent;
#endif
    out.model_0 = transform_mat[0];
    out.model_1 = transform_mat[1];
    out.model_2 = transform_mat[2];
    out.model_3 = transform_mat[3];
    
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
