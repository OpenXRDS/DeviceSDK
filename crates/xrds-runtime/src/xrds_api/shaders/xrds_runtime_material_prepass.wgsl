#import bevy_pbr::{
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions,
    pbr_functions::SampleBias,
    pbr_prepass_functions,
    pbr_types,
    prepass_io::{VertexOutput, FragmentOutput},
}

const XRDS_TEXTURE_FLAG_BASE_COLOR: u32 = 1u << 0u;
const XRDS_TEXTURE_FLAG_NORMAL: u32 = 1u << 2u;
const PREMULTIPLIED_ALPHA_CUTOFF: f32 = 0.05;

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
var base_color_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105)
var normal_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106)
var normal_sampler: sampler;

fn xrds_has_texture(flag: u32) -> bool {
    return (xrds_runtime_material.flags & flag) != 0u;
}

fn xrds_sample_bias() -> SampleBias {
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
    let bias = xrds_sample_bias();

    if xrds_has_texture(XRDS_TEXTURE_FLAG_BASE_COLOR) {
        pbr_input.material.base_color *= textureSampleBias(
            base_color_texture,
            base_color_sampler,
            xrds_slot_uv(xrds_runtime_material.base_color, in),
            bias.mip_bias,
        );
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
            normal_sampler,
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

fn xrds_prepass_alpha_discard(pbr_input: pbr_types::PbrInput) {
    let alpha_mode =
        pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK {
        if pbr_input.material.base_color.a < pbr_input.material.alpha_cutoff {
            discard;
        }
    } else if (
        alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_BLEND ||
        alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ADD ||
        alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ALPHA_TO_COVERAGE
    ) {
        if pbr_input.material.base_color.a < PREMULTIPLIED_ALPHA_CUTOFF {
            discard;
        }
    } else if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_PREMULTIPLIED {
        if all(pbr_input.material.base_color < vec4(PREMULTIPLIED_ALPHA_CUTOFF)) {
            discard;
        }
    }
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input = xrds_apply_extension(in, is_front, pbr_input);
    xrds_prepass_alpha_discard(pbr_input);

    var out: FragmentOutput;

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif

#ifdef NORMAL_PREPASS
    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.normal = vec4(pbr_input.N * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }
#endif

#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(
        in.world_position,
        in.previous_world_position,
    );
#endif

    return out;
}
