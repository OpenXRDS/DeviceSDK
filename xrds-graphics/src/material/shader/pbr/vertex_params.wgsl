#define_import_path shader::pbr::vertex_params

struct Input {
    @location(0) clip_position: vec3<f32>,
#if VERTEX_INPUT_COLOR
#ifdef VERTEX_INPUT_COLOR_3CH
    @location(1) color: vec3<f32>
#else
    @location(1) color: vec4<f32>,
#endif
#endif
#if VERTEX_INPUT_TEXCOORD_0
    @location(2) texcoord_0: vec2<f32>,
#if VERTEX_INPUT_TEXCOORD_1
    @location(3) texcoord_1: vec2<f32>,
#endif
#endif
    @location(4) normal: vec3<f32>,
#if VERTEX_INPUT_TANGENT
    @location(5) tangent: vec4<f32>,
#endif
#if VERTEX_INPUT_WEIGHTS_JOINTS_0
    @location(6) weights_0: vec4<f32>,
    @location(7) joints_0: vec4<u32>,
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_1
    @location(8) weights_1: vec4<f32>,
    @location(9) joints_1: vec4<u32>,
#endif
#endif
#if VERTEX_INPUT_INSTANCE
    @location(10) curr_model_0: vec4<f32>,
    @location(11) curr_model_1: vec4<f32>,
    @location(12) curr_model_2: vec4<f32>,
    @location(13) curr_model_3: vec4<f32>,
    // @location(14) prev_model_0: vec4<f32>,
    // @location(15) prev_model_1: vec4<f32>,
    // @location(16) prev_model_2: vec4<f32>,
    // @location(17) prev_model_3: vec4<f32>,
#endif
}

struct Output {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
#if VERTEX_INPUT_TEXCOORD_0
    @location(2) texcoord_0: vec2<f32>,
#if VERTEX_INPUT_TEXCOORD_1
    @location(3) texcoord_1: vec2<f32>,
#endif
#endif
#if VERTEX_INPUT_TANGENT
    @location(4) world_tangent: vec4<f32>,
#endif
#if VERTEX_INPUT_COLOR
    @location(5) color: vec4<f32>,
#endif
}