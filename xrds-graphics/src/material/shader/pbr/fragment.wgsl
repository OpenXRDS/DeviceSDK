#import shader::pbr::vertex_params as Vertex
#import shader::pbr::fragment_params as Fragment
#import shader::pbr::material_params as Material
#import shader::view_params as View

@fragment
fn main(in: Vertex::Output) -> Fragment::GBuffer {
    var output: Fragment::GBuffer;

    var model = mat4x4<f32>(in.model_0n, in.model_1n, in.model_2n, in.model_3n);

    // Calculate TBN
#ifdef VERTEX_INPUT_NORMAL
    var vertex_normal: vec3<f32> = in.normal;
#else
    var vertex_normal: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);
#endif
#ifdef VERTEX_INPUT_TANGENT
    var tangent: vec4<f32> = in.tangent;
    var t = normalize(model * tangent);
    var bitangent = vec4<f32>(cross(vertex_normal, tangent.rgb), 0.0);
    var b = normalize(model * bitangent);
    var n = normalize(model * vec4<f32>(vertex_normal, 0.0));
    var tbn: mat4x4<f32> = mat4x4<f32>(t, b, n, vec4<f32>(0.0));
#else
    var denorm_tangent = dpdx(in.texcoord_0n.y) * dpdy(in.world_position).rgb - dpdx(in.world_position).rgb * dpdy(in.texcoord_0n.y);
    var tangent: vec3<f32> = normalize(denorm_tangent - vertex_normal * dot(vertex_normal, denorm_tangent));
    var normalized_normal: vec3<f32> = normalize(vertex_normal);
    var bitangent: vec3<f32> = cross(normalized_normal, tangent);
    var tbn = mat4x4<f32>(
        model * vec4<f32>(tangent, 0.0),
        model * vec4<f32>(bitangent, 0.0),
        model * vec4<f32>(normalized_normal, 0.0),
        vec4<f32>(0.0)
    );
#endif

    var base_color: vec4<f32> = Material::get_base_color_texture(in.texcoord_0n);
    var emissive: vec4<f32> = Material::get_emissive_texture(in.texcoord_0n);
    var metallic_roughness: vec4<f32> = Material::get_metallic_roughness_texture(in.texcoord_0n);  // todo: select texcoord
    var normal: vec4<f32> = Material::get_normal_texture(in.texcoord_0n);
    var occlusion: vec4<f32> = Material::get_occlusion_texture(in.texcoord_0n);

    var surface_normal = tbn * vec4<f32>(normal.rgb, 0.0);

    output.position_metallic = vec4<f32>(in.world_position, metallic_roughness.x);
    output.normal_roughness = vec4<f32>(surface_normal.rgb, metallic_roughness.y);
    output.albedo_occlusion = vec4<f32>(base_color.xyz, occlusion.x);
    output.emissive = vec4<f32>(emissive.xyz, 1.0);

    return output;
}