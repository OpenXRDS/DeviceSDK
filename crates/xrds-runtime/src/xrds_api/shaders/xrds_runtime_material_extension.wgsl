#import bevy_pbr::{
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing, SampleBias},
    pbr_types,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
#endif

const XRDS_TEXTURE_FLAG_BASE_COLOR: u32 = 1u << 0u;
const XRDS_TEXTURE_FLAG_METALLIC_ROUGHNESS: u32 = 1u << 1u;
const XRDS_TEXTURE_FLAG_NORMAL: u32 = 1u << 2u;
const XRDS_TEXTURE_FLAG_OCCLUSION: u32 = 1u << 3u;
const XRDS_TEXTURE_FLAG_EMISSIVE: u32 = 1u << 4u;

struct XrdsRuntimeTextureSlotUniform {
    uv_transform: mat3x3<f32>,
    uv_set: u32,
    _padding0: vec3<u32>,
}

struct XrdsRuntimeMaterialExtensionUniform {
    flags: u32,
    _padding: vec3<u32>,
    base_color: XrdsRuntimeTextureSlotUniform,
    metallic_roughness: XrdsRuntimeTextureSlotUniform,
    normal: XrdsRuntimeTextureSlotUniform,
    occlusion: XrdsRuntimeTextureSlotUniform,
    emissive: XrdsRuntimeTextureSlotUniform,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> xrds_runtime_material: XrdsRuntimeMaterialExtensionUniform;
@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var base_color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102)
var textures_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103)
var metallic_roughness_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104)
var normal_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105)
var occlusion_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106)
var emissive_texture: texture_2d<f32>;

fn xrds_has_texture(flag: u32) -> bool {
    return (xrds_runtime_material.flags & flag) != 0u;
}

fn xrds_sample_bias(in: VertexOutput) -> SampleBias {
    var bias: SampleBias;
    bias.mip_bias = view.mip_bias;
    return bias;
}

#ifdef VERTEX_UVS
fn xrds_slot_uv(slot: XrdsRuntimeTextureSlotUniform, in: VertexOutput) -> vec2<f32> {
    var uv = in.uv;
#ifdef VERTEX_UVS_B
    if slot.uv_set == 1u {
        uv = in.uv_b;
    }
#endif
    return (slot.uv_transform * vec3(uv, 1.0)).xy;
}
#endif

fn xrds_apply_extension(
    in: VertexOutput,
    is_front: bool,
    source: pbr_types::PbrInput,
) -> pbr_types::PbrInput {
    var pbr_input = source;

#ifdef VERTEX_UVS
    let bias = xrds_sample_bias(in);

    if xrds_has_texture(XRDS_TEXTURE_FLAG_BASE_COLOR) {
        pbr_input.material.base_color *= textureSampleBias(
            base_color_texture,
            textures_sampler,
            xrds_slot_uv(xrds_runtime_material.base_color, in),
            bias.mip_bias,
        );
    }

    if xrds_has_texture(XRDS_TEXTURE_FLAG_EMISSIVE) {
        pbr_input.material.emissive = vec4<f32>(
            pbr_input.material.emissive.rgb
                * textureSampleBias(
                    emissive_texture,
                    textures_sampler,
                    xrds_slot_uv(xrds_runtime_material.emissive, in),
                    bias.mip_bias,
                ).rgb,
            pbr_input.material.emissive.a,
        );
    }

    if xrds_has_texture(XRDS_TEXTURE_FLAG_METALLIC_ROUGHNESS) {
        let metallic_roughness = textureSampleBias(
            metallic_roughness_texture,
            textures_sampler,
            xrds_slot_uv(xrds_runtime_material.metallic_roughness, in),
            bias.mip_bias,
        );
        pbr_input.material.metallic *= metallic_roughness.b;
        pbr_input.material.perceptual_roughness *= metallic_roughness.g;
    }

    if xrds_has_texture(XRDS_TEXTURE_FLAG_OCCLUSION) {
        pbr_input.diffuse_occlusion *= textureSampleBias(
            occlusion_texture,
            textures_sampler,
            xrds_slot_uv(xrds_runtime_material.occlusion, in),
            bias.mip_bias,
        ).r;
    }

#ifdef VERTEX_TANGENTS
    if xrds_has_texture(XRDS_TEXTURE_FLAG_NORMAL) {
        let double_sided =
            (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let tbn = pbr_functions::calculate_tbn_mikktspace(
            pbr_input.world_normal,
            in.world_tangent,
        );
        let sampled_normal = textureSampleBias(
            normal_texture,
            textures_sampler,
            xrds_slot_uv(xrds_runtime_material.normal, in),
            bias.mip_bias,
        ).rgb;
        pbr_input.N = pbr_functions::apply_normal_mapping(
            pbr_input.material.flags,
            tbn,
            double_sided,
            is_front,
            sampled_normal,
        );
        pbr_input.clearcoat_N = pbr_input.N;
    }
#endif
#endif

    return pbr_input;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input = xrds_apply_extension(in, is_front, pbr_input);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    return deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
#endif
}
