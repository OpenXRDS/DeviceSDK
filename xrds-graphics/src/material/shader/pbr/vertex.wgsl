#import shader::pbr::vertex as Vertex

@vertex
fn vs_main(in: Vertex::Input) -> Vertex::Output {
    var out: Vertex::Output;

#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_0
    var skinning_model = get_skinning_model(in.joints_0, in.weights_0);
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_1
    skinning_model += get_skinning_model(in.joints_1, in.weights_1);
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
        in.curr_model_0,
        in.curr_model_1,
        in.curr_model_2,
        in.curr_model_3
    );
    var transform_mat = model * skinning_model;
    var pos: vec4<f32> = transform_mat * vec4<f32>(in.position, 1.0);
    out.clip_position = u_curr_view_proj * pos;
    out.world_position = pos.xyz / pos.w;
#ifdef VERTEX_INPUT_TEXCOORD_0
    out.texcoord_0 = in.texcoord_0;
#endif
#ifdef VERTEX_INPUT_TEXCOORD_1
    out.texcoord_1 = in.texcoord_1;
#endif
#ifdef VERTEX_INPUT_TANGENT
    out.world_tangent = inv_model * in.tangent;
#endif
#ifdef VERTEX_INPUT_COLOR
#ifdef VERTEX_INPUT_COLOR_3CH
    out.color = vec4(in.color, 1.0);
#else
    out.color = in.color;
#endif
}