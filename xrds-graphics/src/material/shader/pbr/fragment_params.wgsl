#define_import_path shader::pbr::fragment_params

struct Output {
#ifdef FRAGMENT_OUTPUT_FINAL_COLOR
    @location(1) final_color: vec4<f32>,
#endif
#ifdef FRAGMENT_OUTPUT_SPECULAR_ROUGHNESS
    @location(2) specular_roughness: vec4<f32>,
#endif
#ifdef FRAGMENT_OUTPUT_DIFFUSE
    @location(2) diffuse: vec4<f32>,
#endif
#ifdef FRAGMENT_OUTPUT_NORMAL
    @location(3) normal: vec4<f32>,
#endif
#ifdef FRAGMENT_OUTPUT_UPSCALE_REACTIVE
    @location(4) upscale_reactive: vec4<f32>,
#endif
#ifdef FRAGMENT_OUTPUT_UPSCALE_TRANSPARENCY_AND_COMPOSITION
    @location(5) upscale_transparency_and_composition: vec4<f32>,
#endif
#ifdef FRAGMENT_OUTPUT_MOTION_VECTOR
    @location(6) motion_vector: vec2<f32>
#endif
}