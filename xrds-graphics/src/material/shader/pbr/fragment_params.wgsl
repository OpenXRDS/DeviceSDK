#ifndef PBR_FRAGMENT_PARAMS_WGSL
#define PBR_FRAGMENT_PARAMS_WGSL

struct GBuffer {
    @location(0) position_metallic: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
    @location(2) albedo_occlusion: vec4<f32>,
    @location(3) emissive: vec4<f32>,
}

#endif  // PBR_FRAGMENT_PARAMS_WGSL