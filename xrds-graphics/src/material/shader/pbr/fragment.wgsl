#include common::view_params
#include pbr::vertex_params
#include pbr::fragment_params
#include pbr::material_params

@fragment
fn main(in: VertexOutput) -> GBuffer {
    var output: GBuffer;

    var uvs = vec4<f32>(
        in.texcoord_0.x, in.texcoord_0.y,
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

    var model = mat4x4<f32>(in.model_0, in.model_1, in.model_2, in.model_3);

    // Calculate TBN
#ifdef VERTEX_INPUT_TANGENT
    // Normal must exists
    var mt: vec4<f32> = model * vec4<f32>(in.tangent.xyz, 0.0);
    var t: vec3<f32> = normalize(mt.xyz);
    var bn: vec3<f32> = cross(in.normal, t) * in.tangent.w;
    var tbn: mat3x3<f32> = mat3x3<f32>(t, bn, in.normal);
#else
    var pos_dx: vec3<f32> = dpdx(in.world_position);
    var pos_dy: vec3<f32> = dpdy(in.world_position);
    var tex_dx: vec3<f32> = dpdx(vec3<f32>(in.texcoord_0, 0.0));
    var tex_dy: vec3<f32> = dpdy(vec3<f32>(in.texcoord_0, 0.0));
    var t = (tex_dy.y * pos_dx - tex_dx.y * pos_dy) / (tex_dx.x * tex_dy.y - tex_dy.x * tex_dx.y);

#ifdef VERTEX_INPUT_NORMAL
    var ng: vec3<f32> = normalize(in.normal);
#else
    var ng: vec3<f32> = cross(pos_dx, pos_dy);
#endif

    t = normalize(t - ng * dot(ng, t));
    var b: vec3<f32> = normalize(cross(ng, t));
    var tbn = mat3x3<f32>(t, b, ng);
#endif

#ifdef MATERIAL_INPUT_NORMAL_TEXTURE
    var xy: vec2<f32> = 2.0 * pbr_params.normal.rg - 1.0;
    var z: f32 = sqrt(1.0 - dot(xy, xy));
    var n: vec3<f32> = vec3<f32>(xy, z);
    n = normalize(tbn * n);
#else
    var n: vec3<f32> = normalize(tbn[2].xyz);
#endif

    output.position_metallic = vec4<f32>(in.world_position, pbr_params.occlusion_metallic_roughness.b);
    output.normal_roughness = vec4<f32>(n.rgb, pbr_params.occlusion_metallic_roughness.g);
    output.albedo_occlusion = vec4<f32>(pbr_params.base_color.xyz, pbr_params.occlusion_metallic_roughness.r);
    output.emissive = vec4<f32>(pbr_params.emissive.rgb, 1.0);

    return output;
}