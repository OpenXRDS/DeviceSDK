#ifndef PBR_VERTEX_PARAMS_WGSL
#define PBR_VERTEX_PARAMS_WGSL

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
#ifdef VERTEX_INPUT_POSITION
    @location(0) position: vec3<f32>,
#endif
#ifdef VERTEX_INPUT_COLOR
#ifdef VERTEX_INPUT_COLOR_3CH
    @location(1) color: vec3<f32>,
#else
    @location(1) color: vec4<f32>,
#endif
#endif
#ifdef VERTEX_INPUT_TEXCOORD_0
    @location(2) texcoord_0: vec2<f32>,
#ifdef VERTEX_INPUT_TEXCOORD_1
    @location(2) texcoord_1: vec2<f32>,
#endif
#endif
#ifdef VERTEX_INPUT_NORMAL
    @location(4) normal: vec3<f32>,
#endif
#ifdef VERTEX_INPUT_TANGENT
    @location(5) tangent: vec4<f32>,
#endif
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_0
    @location(6) weights_0: vec4<f32>,
    @location(7) joints_0: vec4<u32>,
#endif
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_1
    @location(8) weights_1: vec4<f32>,
    @location(9) joints_1: vec4<u32>,
#endif
    @location(10) model_0: vec4<f32>,
    @location(11) model_1: vec4<f32>,
    @location(12) model_2: vec4<f32>,
    @location(13) model_3: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) view_index: i32,
    @location(1) world_position: vec3<f32>,
#ifdef VERTEX_INPUT_TEXCOORD_0
    @location(2) texcoord_0: vec2<f32>,
#ifdef VERTEX_INPUT_TEXCOORD_1
    @location(3) texcoord_1: vec2<f32>,
#endif
#endif
#ifdef VERTEX_INPUT_COLOR
    @location(4) color: vec4<f32>,
#endif
#ifdef VERTEX_INPUT_NORMAL
    @location(5) normal: vec3<f32>,
#endif
#ifdef VERTEX_INPUT_TANGENT
    @location(6) tangent: vec4<f32>,
#endif
}

var<push_constant> p_local_model: mat4x4<f32>;

fn get_instance_model(in: VertexInput) -> mat4x4<f32> {
    var model = mat4x4<f32>(
        in.model_0,
        in.model_1,
        in.model_2,
        in.model_3
    );

    return model;
}


fn get_local_model() -> mat4x4<f32> {
    return p_local_model;
}

#endif  // PBR_VERTEX_PARAMS_WGSL