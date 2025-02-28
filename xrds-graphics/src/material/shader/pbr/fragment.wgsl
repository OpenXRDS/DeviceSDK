#import shader::pbr::vertex_params as Vertex
#import shader::pbr::fragment_params as Fragment
#import shader::view_params as View

@fragment
fn main(in: Vertex::Output) -> Fragment::GBuffer {
    var output: Fragment::GBuffer;
    
    output.position_metallic = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    output.normal_roughness = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    output.albedo_occlusion = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    output.emissive = vec4<f32>(1.0, 1.0, 1.0, 1.0);

    return output;
}