#define_import_path shader::pbr::vertex_params

struct Input {
#ifdef VIEW_COUNT > 1
    @builtin(view_index) view_index: i32,
#endif
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
    @location(2) texcoord_0n: vec2<f32>,
#ifdef VERTEX_INPUT_TEXCOORD_1
    @location(3) texcoord_1n: vec2<f32>,
#endif
#endif
#ifdef VERTEX_INPUT_NORMAL
    @location(4) normal: vec3<f32>,
#endif
#ifdef VERTEX_INPUT_TANGENT
    @location(5) tangent: vec4<f32>,
#endif
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_0
    @location(6) weights_0n: vec4<f32>,
    @location(7) joints_0n: vec4<u32>,
#endif
#ifdef VERTEX_INPUT_WEIGHTS_JOINTS_1
    @location(8) weights_1n: vec4<f32>,
    @location(9) joints_1n: vec4<u32>,
#endif
    // Instance buffer value. It is changable model matrix
    @location(10) model_0n: vec4<f32>,
    @location(11) model_1n: vec4<f32>,
    @location(12) model_2n: vec4<f32>,
    @location(13) model_3n: vec4<f32>,
}

struct Output {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
#ifdef VERTEX_INPUT_TEXCOORD_0
    @location(2) texcoord_0n: vec2<f32>,
#ifdef VERTEX_INPUT_TEXCOORD_1
    @location(3) texcoord_1n: vec2<f32>,
#endif
#endif
#ifdef VERTEX_INPUT_TANGENT
    @location(4) world_tangent: vec4<f32>,
#endif
#ifdef VERTEX_INPUT_COLOR
    @location(5) color: vec4<f32>,
#endif
    @location(6) view_index: i32,
}