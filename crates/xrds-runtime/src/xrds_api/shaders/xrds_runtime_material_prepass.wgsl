#import bevy_pbr::{
    mesh_bindings::mesh,
    mesh_view_bindings::view,
    pbr_bindings,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions,
    pbr_functions::SampleBias,
    pbr_prepass_functions,
    pbr_types,
    prepass_io::{VertexOutput, FragmentOutput},
}

// Bindless keeps materials in an array indexed per draw; the non-bindless path
// has a single uniform. Both spellings are needed, and the import itself is
// conditional — copied from `bevy_pbr::pbr_prepass_functions`, which reads the
// material in a prepass for the same reason.
#ifdef BINDLESS
#import bevy_pbr::pbr_bindings::material_indices
#endif  // BINDLESS

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
var textures_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(104)
var normal_texture: texture_2d<f32>;

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
            textures_sampler,
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

/// The minimum needed to decide whether this fragment is discarded.
///
/// A **depth-only** prepass has no `world_normal` on its `VertexOutput` — the
/// field sits behind `NORMAL_PREPASS_OR_DEFERRED_PREPASS` in `prepass_io.wgsl` —
/// while `pbr_input_from_standard_material` reads it unconditionally. Calling that
/// here fails pipeline specialisation with `invalid field accessor world_normal`,
/// and that is a *shader* error raised at draw time: nothing catches it until
/// something actually renders through a depth prepass with this material.
///
/// So this reads only what `xrds_prepass_alpha_discard` needs — the flags, the
/// cutoff and the base colour — and never touches a normal. Bevy's own
/// `prepass.wgsl` follows the same rule, reading `in.world_normal` only under
/// `#ifdef NORMAL_PREPASS`.
///
/// The material read is copied from `bevy_pbr::pbr_prepass_functions`, bindless
/// branch included: this runs in a prepass, where that is the established way to
/// reach the material.
fn xrds_alpha_only_pbr_input(in: VertexOutput) -> pbr_types::PbrInput {
    var pbr_input: pbr_types::PbrInput = pbr_types::pbr_input_new();

#ifdef BINDLESS
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    pbr_input.material.flags = pbr_bindings::material_array[material_indices[slot].material].flags;
    pbr_input.material.alpha_cutoff =
        pbr_bindings::material_array[material_indices[slot].material].alpha_cutoff;
    pbr_input.material.base_color =
        pbr_bindings::material_array[material_indices[slot].material].base_color;
#else   // BINDLESS
    pbr_input.material.flags = pbr_bindings::material.flags;
    pbr_input.material.alpha_cutoff = pbr_bindings::material.alpha_cutoff;
    pbr_input.material.base_color = pbr_bindings::material.base_color;
#endif  // BINDLESS

    // Our own base-colour texture, not the standard material's — the extension is
    // where a video or an authored texture lives, so testing the standard slot
    // would cut out against the wrong image.
#ifdef VERTEX_UVS
    if xrds_has_texture(XRDS_TEXTURE_FLAG_BASE_COLOR) {
        pbr_input.material.base_color *= textureSampleBias(
            base_color_texture,
            textures_sampler,
            xrds_slot_uv(xrds_runtime_material.base_color, in),
            xrds_sample_bias().mip_bias,
        );
    }
#endif

    return pbr_input;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
    // `in.world_normal` exists in this configuration, and the normal output below
    // needs the full input anyway.
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input = xrds_apply_extension(in, is_front, pbr_input);
#else
    // Depth-only: no normal on the vertex output, and nothing here wants one.
    var pbr_input = xrds_alpha_only_pbr_input(in);
#endif
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
