#include common::view_params
#include pbr::vertex_params
#include pbr::fragment_params
#include pbr::material_params

@fragment
fn main(in: VertexOutput) -> GBuffer {
    var output: GBuffer;

    var uvs = vec4<f32>(
#ifdef VERTEX_INPUT_TEXCOORD_0
        in.texcoord_0.x, in.texcoord_0.y,
#else
        0.0, 0.0,
#endif
#ifdef VERTEX_INPUT_TEXCOORD_1
        in.texcoord_1.x, in.texcoord_1.y,
#else
        0.0, 0.0
#endif
    );

    var pbr_params = get_pbr_params(uvs);
#ifdef MATERIAL_INPUT_ALPHA_MODE_MASK
    if pbr_params.base_color.a < get_alpha_cutoff() {
        discard;
    }
#endif

#ifdef VERTEX_INPUT_NORMAL
    var world_normal: vec3<f32> = normalize(in.normal);
#else
    var pos_dx: vec3<f32> = dpdx(in.world_position);
    var pos_dy: vec3<f32> = dpdy(in.world_position);
    var world_normal: vec3<f32> = cross(pos_dx, pos_dy);
#endif

    // Calculate TBN
#ifdef VERTEX_INPUT_TANGENT
    var world_tangent = normalize(in.tangent.xyz);
    var world_bitangent = normalize(cross(world_normal, world_tangent) * in.tangent.w);
    world_tangent = normalize(cross(world_bitangent, world_normal));
    var tbn = mat3x3<f32>(world_tangent, world_bitangent, world_normal); 
#else
    var pos_dx: vec3<f32> = dpdx(in.world_position);
    var pos_dy: vec3<f32> = dpdy(in.world_position);
#ifdef VERTEX_INPUT_TEXCOORD_0
    var tex_dx: vec3<f32> = dpdx(vec3<f32>(in.texcoord_0, 0.0));
    var tex_dy: vec3<f32> = dpdy(vec3<f32>(in.texcoord_0, 0.0));
#else
    var tex_dx = pos_dx;
    var tex_dy = pos_dy;
#endif
    var t_dpdx = (tex_dy.y * pos_dx - tex_dx.y * pos_dy) / (tex_dx.x * tex_dy.y - tex_dy.x * tex_dx.y);

    var t = normalize(t_dpdx - world_normal * dot(world_normal, t_dpdx));
    var b: vec3<f32> = normalize(cross(world_normal, t));
    var tbn = mat3x3<f32>(t, b, world_normal);
#endif

#ifdef MATERIAL_INPUT_NORMAL_TEXTURE
    var xy: vec2<f32> = 2.0 * pbr_params.normal.rg - 1.0;
    var z: f32 = sqrt(max(0.0, 1.0 - dot(xy, xy)));
    var n_tangent: vec3<f32> = vec3<f32>(xy, z);
    var n_final_normal = normalize(tbn * n_tangent);
#else
    var n_final_normal: vec3<f32> = normalize(tbn[2].xyz);
#endif

    // Calculate motion vector
    var curr_ndc = in.curr_position.xy / in.curr_position.w;
    var prev_ndc = in.prev_position.xy / in.prev_position.w;
    var motion_ndc = curr_ndc - prev_ndc;
    var jitter_diff_clip = in.curr_jitter - in.prev_jitter;
    var final_motion_ndc = motion_ndc - jitter_diff_clip;
    var motion_uv = vec4<f32>(final_motion_ndc * 0.5, 0.0, 1.0);

    // motion_uv = vec4<f32>(curr_ndc * 0.5 + 0.5, 0.0, 1.0);  // Yellow
    // motion_uv = vec4<f32>(prev_ndc * 0.5 + 0.5, 0.0, 1.0);  // Red/green gradient left-bottom is 0 right-top is 1
    // motion_uv = vec4<f32>(motion_ndc * 0.5 + 0.5, 0.0, 1.0);  // Yellow
    // motion_uv = vec4<f32>(jitter_diff_clip * 10.0 + 0.5, 0.0, 1.0);  // Darker yellow

    output.position_metallic = vec4<f32>(in.world_position, pbr_params.occlusion_metallic_roughness.b);
    output.normal_roughness = vec4<f32>(n_final_normal.rgb, pbr_params.occlusion_metallic_roughness.g);
    output.albedo_occlusion = vec4<f32>(pbr_params.base_color.xyz, pbr_params.occlusion_metallic_roughness.r);
    output.emissive = vec4<f32>(pbr_params.emissive.rgb, 1.0);
    output.motion_vector = motion_uv;

    return output;
}