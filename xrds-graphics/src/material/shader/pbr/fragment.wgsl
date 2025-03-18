#import shader::pbr::vertex_params as Vertex
#import shader::pbr::fragment_params as Fragment
#import shader::pbr::material_params as Material
#import shader::view_params as View

@fragment
fn main(in: Vertex::Output) -> Fragment::GBuffer {
    var output: Fragment::GBuffer;

    var base_color: vec4<f32> = Material::get_base_color_texture(in.texcoord_0n);
    var emissive: vec4<f32> = Material::get_emissive_texture(in.texcoord_0n);
    var metallic_roughness: vec4<f32> = Material::get_metallic_roughness_texture(in.texcoord_0n);  // todo: select texcoord
    var normal: vec4<f32> = Material::get_normal_texture(in.texcoord_0n);
    var occlusion: vec4<f32> = Material::get_occlusion_texture(in.texcoord_0n);

    output.position_metallic = vec4<f32>(in.world_position, metallic_roughness.x);
    output.normal_roughness = vec4<f32>(normal.xyz, metallic_roughness.y);
    output.albedo_occlusion = vec4<f32>(base_color.xyz, occlusion.x);
    output.emissive = vec4<f32>(emissive.xyz, 1.0);

    return output;
}