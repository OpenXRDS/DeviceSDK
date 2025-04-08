#ifndef PBR_MATERIAL_PARAMS_WGSL
#define PBR_MATERIAL_PARAMS_WGSL

struct PbrParams {
    base_color_factor: vec4<f32>,
    emissive_factor: vec4<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
    normal_scale: f32,
    occlusion_strength: f32,
    alpha_cutoff: f32,
    texcoord_base_color: i32,
    texcoord_emissive: i32,
    texcoord_metallic_roughness: i32,
    texcoord_normal: i32,
    texcoord_occlusion: i32,
    texcoord_diffuse: i32,
    texcoord_specular_glossiness: i32,
}

@group(1) @binding(0)
var<uniform> u_pbr_params: PbrParams;

#ifdef MATERIAL_INPUT_BASE_COLOR_TEXTURE
@group(1) @binding(1)
var base_color_texture: texture_2d<f32>;
@group(1) @binding(2)
var base_color_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE
@group(1) @binding(3)
var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(4)
var metallic_roughness_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_NORMAL_TEXTURE
@group(1) @binding(5)
var normal_texture: texture_2d<f32>;
@group(1) @binding(6)
var normal_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_EMISSIVE_TEXTURE
@group(1) @binding(7)
var emissive_texture: texture_2d<f32>;
@group(1) @binding(8)
var emissive_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_OCCLUSION_TEXTURE
@group(1) @binding(9)
var occlusion_texture: texture_2d<f32>;
@group(1) @binding(10)
var occlusion_sampler: sampler;
#endif
#ifdef MATERIAL_INPUT_DIFFUSE_TEXTURE
@group(1) @binding(11)
var diffuse_texture: texture_2d<f32>;
@group(1) @binding(12)
var diffuse_sampler: sampler;
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

struct GetPbrParamsOutput {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
    occlusion_metallic_roughness: vec4<f32>,
    normal: vec4<f32>,
    normal_scale: f32,
}

fn get_pbr_params(uvs: vec4<f32>) -> GetPbrParamsOutput{
    var out: GetPbrParamsOutput;

    var auvs = array<vec2<f32>, 2>(
        vec2<f32>(uvs.x, uvs.y),
        vec2<f32>(uvs.z, uvs.w)
    );

    out.base_color = u_pbr_params.base_color_factor;
    out.emissive = u_pbr_params.emissive_factor;
    out.occlusion_metallic_roughness = vec4<f32>(u_pbr_params.occlusion_strength, u_pbr_params.metallic_factor, u_pbr_params.roughness_factor, 1.0);
    out.normal = vec4<f32>(u_pbr_params.normal_scale, u_pbr_params.normal_scale, u_pbr_params.normal_scale, 1.0);
    out.normal_scale = u_pbr_params.normal_scale;

#ifdef MATERIAL_INPUT_BASE_COLOR_TEXTURE
    out.base_color *= textureSample(base_color_texture, base_color_sampler, auvs[u_pbr_params.texcoord_base_color]);
#endif
#ifdef MATERIAL_INPUT_EMISSIVE_TEXTURE
    out.emissive *= textureSample(emissive_texture, emissive_sampler, auvs[u_pbr_params.texcoord_emissive]);
#endif
#ifdef MATERIAL_INPUT_METALLIC_ROUGHNESS_TEXTURE
    var metallic_roughness = textureSample(metallic_roughness_texture, metallic_roughness_sampler, auvs[u_pbr_params.texcoord_metallic_roughness]);
    out.occlusion_metallic_roughness *= vec4<f32>(1.0, metallic_roughness.g, metallic_roughness.b, 1.0);
#endif
#ifdef MATERIAL_INPUT_OCCLUSION_TEXTURE
    var occlusion = textureSample(occlusion_texture, occlusion_sampler, auvs[u_pbr_params.texcoord_occlusion]);
    out.occlusion_metallic_roughness *= vec4<f32>(occlusion.r, 1.0, 1.0, 1.0);
#endif
#ifdef MATERIAL_INPUT_NORMAL_TEXTURE
    out.normal = textureSample(normal_texture, normal_sampler, auvs[u_pbr_params.texcoord_normal]);
#endif

    return out;
}

fn get_alpha_cutoff() -> f32 {
    return u_pbr_params.alpha_cutoff;
}

#endif  // PBR_MATERIAL_PARAMS_WGSL