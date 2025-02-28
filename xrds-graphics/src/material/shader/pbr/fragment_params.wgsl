#define_import_path shader::pbr::fragment_params

struct GBuffer {
    @location(0) position_metallic: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
    @location(2) albedo_occlusion: vec4<f32>,
    @location(3) emissive: vec4<f32>,
}