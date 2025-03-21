#import shader::view_params as View

struct DeferredVertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
}

struct DeferredVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) view_index: i32,
}

struct DeferredFragmentOutput {
    @location(0) final_color: vec4<f32>,
}

// Separate groups per textures because texture_2d_array has multiple binding index
@group(1) @binding(0)
var position_metallic_sampler: sampler;
@group(1) @binding(1)
var position_metallic_texture: texture_2d_array<f32>;
@group(1) @binding(2)
var normal_roughness_sampler: sampler;
@group(1) @binding(3)
var normal_roughness_texture: texture_2d_array<f32>;
@group(1) @binding(4)
var albedo_occlusion_sampler: sampler;
@group(1) @binding(5)
var albedo_occlusion_texture: texture_2d_array<f32>;
@group(1) @binding(6)
var emissive_sampler: sampler;
@group(1) @binding(7)
var emissive_texture: texture_2d_array<f32>;

const PI = 3.14159265359;

@vertex
fn vs_main(in: DeferredVertexInput) -> DeferredVertexOutput {
    var output: DeferredVertexOutput;

    var uv: vec2<f32> = vec2<f32>(f32((in.vertex_index << 1) & 2), f32(in.vertex_index & 2));
    output.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);

    // Flip uv for wgpu texture coordinates correction
    uv.y = 1.0 - uv.y;
    output.uv = uv;
    output.view_index = in.view_index;

    return output;
}

// For test
const LIGHT_COUNT: u32 = 6;

const LIGHT_POSITIONS = array(
    vec3<f32>(0.0, 3.0, 0.0),
    vec3<f32>(0.0, 1.0, 3.0),
    vec3<f32>(2.0, 1.0, 4.0),
    vec3<f32>(0.0, 1.0, 10.0),
    vec3<f32>(10.0, 0.0, 2.0),
    vec3<f32>(5.0, 5.0, 5.0),
);

const LIGHT_COLORS = array(
    vec4<f32>(1.0, 0.0, 0.0, 20.0),
    vec4<f32>(0.0, 1.0, 0.0, 20.0),
    vec4<f32>(0.0, 0.0, 1.0, 20.0),
    vec4<f32>(1.0, 1.0, 0.0, 20.0),
    vec4<f32>(1.0, 0.0, 1.0, 20.0),
    vec4<f32>(0.0, 1.0, 1.0, 20.0),
);

@fragment
fn fs_main(in: DeferredVertexOutput) -> DeferredFragmentOutput {
    // Do deferred lighting
    var output: DeferredFragmentOutput;

    var view_params = View::get_view_params(in.view_index);
    var cam_pos = view_params.world_position;

    var position_metallic = get_position_metallic(in);
    var normal_roughness = get_normal_roughness(in);
    var albedo_occlusion = get_albedo_occlusion(in);
    var emissive: vec4<f32> = get_emissive(in);

    var position: vec3<f32> = position_metallic.rgb;
    var metallic: f32 = position_metallic.a;
    var normal: vec3<f32> = normal_roughness.rgb;
    var roughness: f32 = normal_roughness.a;
    var albedo: vec3<f32> = albedo_occlusion.rgb;
    var occlusion: f32 = albedo_occlusion.a;

    var n: vec3<f32> = normalize(normal);
    var v: vec3<f32> = normalize(cam_pos - position.rgb);
    var lo: vec3<f32> = vec3(0.0);

    // Loop all lights
    for (var i: u32 = 0; i < LIGHT_COUNT; i++) {
        var light_position = LIGHT_POSITIONS[i];
        var light_color = LIGHT_COLORS[i];
    
        var l: vec3<f32> = normalize(light_position - position.rgb);
        var h: vec3<f32> = normalize(v + l);
        var distance: f32 = length(light_position - position.rgb);
        var attenuation: f32 = 1.0 / (distance * distance);
        var radiance: vec3<f32> = light_color.rgb * light_color.a * attenuation;

        var f0: vec3<f32> = vec3(0.04);
        f0 = mix(f0, albedo.rgb, metallic);
        var ndf: f32 = distribution_ggx(n, h, roughness);
        var g: f32 = geometry_smith(n, v, l, roughness);
        var f: vec3<f32> = fresnel_schlick(max(dot(h, v), 1.0), f0);

        var ks: vec3<f32> = f;
        var kd: vec3<f32> = vec3(1.0) - ks;
        kd *= (1.0 - metallic);

        var numerator: vec3<f32> = ndf * g * f;
        var denominator: f32 = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
        var specular: vec3<f32> = numerator / denominator;

        var n_dot_l: f32 = max(dot(n, l), 0.0);
        lo += (kd * albedo.rgb / PI + specular) * radiance * n_dot_l;
    }

    var ambient: vec3<f32> = vec3(0.03) * albedo.rgb * occlusion;
    var color: vec3<f32> = ambient + emissive.rgb + lo;

    output.final_color = vec4<f32>(color, 1.0);

    return output;
}

fn get_position_metallic(in: DeferredVertexOutput) -> vec4<f32> {
    return textureSample(position_metallic_texture, position_metallic_sampler, in.uv, in.view_index);
}

fn get_normal_roughness(in: DeferredVertexOutput) -> vec4<f32> {
    return textureSample(normal_roughness_texture, normal_roughness_sampler, in.uv, in.view_index);
}

fn get_albedo_occlusion(in: DeferredVertexOutput) -> vec4<f32> {
    return textureSample(albedo_occlusion_texture, albedo_occlusion_sampler, in.uv, in.view_index);
}

fn get_emissive(in: DeferredVertexOutput) -> vec4<f32> {
    return textureSample(emissive_texture, emissive_sampler, in.uv, in.view_index);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    var a = roughness * roughness;
    var a2 = a * a;
    var n_dot_h = max(dot(n, h), 0.0);
    var n_dot_h2 = n_dot_h * n_dot_h;
    var num = a2;
    var denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return num / denom;
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    var r = roughness + 1.0;
    var k = (r * r) / 8.0;
    var num = n_dot_v;
    var denom = n_dot_v * (1.0 - k) + k;

    return num / denom;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    var n_dot_v = max(dot(n, v), 0.0);
    var n_dot_l = max(dot(n, l), 0.0);
    var ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    var ggx1 = geometry_schlick_ggx(n_dot_l, roughness);

    return ggx1 * ggx2;
}
