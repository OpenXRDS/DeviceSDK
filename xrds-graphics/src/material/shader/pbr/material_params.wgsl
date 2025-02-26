#define_import_path shader::pbr::material_params

struct PbrParams {
    base_color_factor: vec4<f32>,
    emissive_factor: vec4<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
    normal_scale: f32,
    occlusion_strength: f32,
}

@group(1) @binding(0)
var<uniform> pbr_params: PbrParams;
#ifdef MATERIAL_INPUT_BASE_COLOR_TEXTURE
@group(1) @binding(1)
var base_color_texcure: texture_2d<f32>;
@group(1) @binding(2)
var base_color_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_DIFFUSE_TEXTURE
@group(1) @binding(3)
var diffuse_texture: texture_2d<f32>;
@group(1) @binding(4)
var diffuse_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_EMISSIVE_TEXTURE
@group(1) @binding(5)
var emissive_texture: texture_2d<f32>;
@group(1) @binding(6)
var emissive_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE
@group(1) @binding(7)
var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(8)
var metallic_roughness_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_NORMAL_TEXTURE
@group(1) @binding(9)
var normal_texture: texture_2d<f32>;
@group(1) @binding(10)
var normal_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_OCCLUSION_TEXTURE
@group(1) @binding(11)
var occlusion_texture: texture_2d<f32>;
@group(1) @binding(12)
var occlusion_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE
@group(1) @binding(13)
var specular_glossiness_texture: texture_2d<f32>;
@group(1) @binding(14)
var specular_glossiness_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_IBL
#ifdef MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE
@group(1) @binding(15)
var ibl_diffuse_texture: texture_cube<f32>;
@group(1) @binding(16)
var ibl_diffuse_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_IBL_SPECULAR_TEXTURE
@group(1) @binding(17)
var ibl_specular_texture: texture_cube<f32>;
@group(1) @binding(18)
var ibl_specular_sampler: sampler;
#endif
#endif
#ifdef MATERIAL_INPUT_BRDF_TEXTURE
@group(1) @binding(19)
var brdf_texture: texture_2d<f32>;
@group(1) @binding(20)
var brdf_sampler: sampler;
#endif

fn get_base_color_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_BASE_COLOR_TEXTURE
    return textureSample(base_color_texcure, base_color_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_diffuse_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_DIFFUSE_TEXTURE
    return textureSample(diffuse_texture, diffuse_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_emissive_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_EMISSIVE_TEXTURE
    return textureSample(emissive_texture, emissive_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_metallic_roughness_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE
    return textureSample(metallic_roughness_texture, metallic_roughness_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_normal_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_NORMAL_TEXTURE
    return textureSample(normal_texture, normal_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_occlusion_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_OCCLUSION_TEXTURE
    return textureSample(occlusion_texture, occlusion_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_specular_glossiness_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_SPECULAR_GLOSSINESS_TEXTURE
    return textureSample(specular_glossiness_texture, specular_glossiness_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_ibl_diffuse_texture(coord: vec3<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_IBL_DIFFUSE_TEXTURE
    return textureSample(ibl_diffuse_sampler, ibl_diffuse_sampler, coord);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_ibl_specular_texture(coord: vec3<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_IBL_SPECULAR_TEXTURE
    return textureSample(ibl_specular_sampler, ibl_specular_sampler, coord);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}

fn get_brdf_texture(uv: vec2<f32>) -> vec4<f32> {
#ifdef MATERIAL_INPUT_BRDF_TEXTURE
    return textureSample(brdf_texture, brdf_sampler, uv);
#else
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
}