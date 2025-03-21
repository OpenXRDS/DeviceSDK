#import shader::pbr::vertex_params as Vertex
#import shader::pbr::skinning as Skinning
#import shader::view_params as View

@vertex
fn main(in: Vertex::Input) -> Vertex::Output {
    var out: Vertex::Output;

    var transform_mat = 
        View::get_local_model() * // local model
        get_skinning_model(in) * // skinned mesh
        Vertex::get_instance_model(in); // world model
    var pos = transform_mat * vec4<f32>(in.position, 1.0);
    var view_params = View::get_view_params(in.view_index);

    out.position = view_params.view_projection * pos;
    out.view_index = in.view_index;
    out.world_position = pos.xyz / pos.w;
#ifdef VERTEX_INPUT_TEXCOORD_0
    out.texcoord_0n = in.texcoord_0n;
#endif
#ifdef VERTEX_INPUT_TEXCOORD_1
    out.texcoord_1n = in.texcoord_1n;
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
    out.model_0n = transform_mat[0];
    out.model_1n = transform_mat[1];
    out.model_2n = transform_mat[2];
    out.model_3n = transform_mat[3];
    
    return out;
}

fn get_skinning_model(in: Vertex::Input) -> mat4x4<f32> {
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_0
    var skinning_model = Skinning::get_skinning_model(in.joints_0, in.weights_0);
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_1
    skinning_model += Skinning::get_skinning_model(in.joints_1, in.weights_1);
#endif
    return skinning_model;
#else
    return mat4x4<f32>(
        vec4(1.0, 0.0, 0.0, 0.0),
        vec4(0.0, 1.0, 0.0, 0.0),
        vec4(0.0, 0.0, 1.0, 0.0),
        vec4(0.0, 0.0, 0.0, 1.0)
    );
#endif
}
