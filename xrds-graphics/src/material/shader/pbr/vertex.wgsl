#import shader::pbr::vertex_params as Vertex
#import shader::pbr::skinning as Skinning
#import shader::view_params as View

@vertex
fn main(in: Vertex::Input) -> Vertex::Output {
    var out: Vertex::Output;

#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_0
    var skinning_model = Skinning::get_skinning_model(in.joints_0, in.weights_0);
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_1
    skinning_model += Skinning::get_skinning_model(in.joints_1, in.weights_1);
#endif
#else
    var skinning_model = mat4x4<f32>(
        vec4(1.0, 0.0, 0.0, 0.0),
        vec4(0.0, 1.0, 0.0, 0.0),
        vec4(0.0, 0.0, 1.0, 0.0),
        vec4(0.0, 0.0, 0.0, 1.0)
    );
#endif
    var model = mat4x4<f32>(
        in.model_0n,
        in.model_1n,
        in.model_2n,
        in.model_3n
    );

    var transform_mat = model * skinning_model;
    var pos = transform_mat * vec4<f32>(in.position, 1.0);
#if VIEW_COUNT > 1
    var view_params = View::u_view_params[in.view_index];
#else
    var view_params = View::u_view_params;
#endif
    out.clip_position = view_params.view_projection * pos;
    out.world_position = pos.xyz / pos.w;
#ifdef VERTEX_INPUT_NORMAL
    out.world_normal = (transform_mat * vec4<f32>(in.normal, 1.0)).xyz;
#else
    out.world_normal = (transform_mat * vec4(0.0, 0.0, 1.0, 1.0)).xyz;
#endif
#ifdef VERTEX_INPUT_TEXCOORD_0
    out.texcoord_0n = in.texcoord_0n;
#endif
#ifdef VERTEX_INPUT_TEXCOORD_1
    out.texcoord_1n = in.texcoord_1n;
#endif
#ifdef VERTEX_INPUT_TANGENT
    out.world_tangent = transform_mat * in.tangent;
#endif
#ifdef VERTEX_INPUT_COLOR
#ifdef VERTEX_INPUT_COLOR_3CH
    out.color = vec4(in.color, 1.0);
#else
    out.color = in.color;
#endif
#endif
    out.view_index = in.view_index;
    return out;
}