#import shader::pbr::vertex as Vertex
#import shader::pbr::fragment as Fragment

@fragment
fn fs_main(in: Vertex::Output) -> Fragment::Output {
    var output: Fragment::Output;
    
#ifdef FRAGMENT_OUTPUT_FINAL_COLOR
    output.final_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
#ifdef FRAGMENT_OUTPUT_SPECULAR_ROUGHNESS
    output.specular_roughness = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
#ifdef FRAGMENT_OUTPUT_DIFFUSE
    output.diffuse = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
#ifdef FRAGMENT_OUTPUT_NORMAL
    output.normal = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
#ifdef FRAGMENT_OUTPUT_UPSCALE_REACTIVE
    output.upscale_reactive = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
#ifdef FRAGMENT_OUTPUT_UPSCALE_TRANSPARENCY_AND_COMPOSITION
    output.upscale_transparency_and_composition = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
#ifdef FRAGMENT_OUTPUT_MOTION_VECTOR
    output.motion_vector = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif

    return output;
}