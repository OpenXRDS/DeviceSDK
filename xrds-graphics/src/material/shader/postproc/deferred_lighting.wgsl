#include common::view_params
#include postproc::types

#define LIGHT_PARAMS_INPUT
#include common::light_params

struct MaterialInfo {
    roughness: f32,
    alpha_roughness: f32,
    diffuse: vec3<f32>,
    specular: vec3<f32>,
    reflectance_0: vec3<f32>,
    reflectance_90: vec3<f32>,
}

struct AngularInfo {
    n_dot_l: f32,
    n_dot_v: f32,
    n_dot_h: f32,
    l_dot_h: f32,
    v_dot_h: f32,
}

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

const CONST_PI = 3.14159265359;

// For test
const LIGHT_COUNT: u32 = 3;

const LIGHT_POSITIONS = array(
    vec3<f32>(3.0, 0.0, 0.0),
    vec3<f32>(0.0, 3.0, 0.0),
    vec3<f32>(0.0, 0.0, 3.0),
    vec3<f32>(-3.0, 0.0, 0.0),
    vec3<f32>(0.0, -3.0, 0.0),
    vec3<f32>(0.0, 0.0, -3.0),
);

const LIGHT_COLORS = array(
    vec4<f32>(1.0, 0.0, 0.0, 4.0),
    vec4<f32>(0.0, 1.0, 0.0, 4.0),
    vec4<f32>(0.0, 0.0, 1.0, 4.0),
    vec4<f32>(1.0, 1.0, 0.0, 4.0),
    vec4<f32>(1.0, 0.0, 1.0, 4.0),
    vec4<f32>(0.0, 1.0, 1.0, 4.0),
);

const DIRECTIONAL_LIGHT_DIRECTION: vec3<f32> = vec3<f32>(1.0, 1.0, 0.0);
const DIRECTIONAL_LIGHT_COLOR: vec4<f32> = vec4<f32>(1.0, 1.0, 1.0, 3.0);

@fragment
fn fs_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    // Do deferred lighting
    var view_params = get_view_params(in.view_index);
    var cam_pos = view_params.world_position;

    var albedo_occlusion = get_albedo_occlusion(in);
    var position_metallic = get_position_metallic(in);
    var normal_roughness = get_normal_roughness(in);
    var emissive: vec4<f32> = get_emissive(in);

    var position: vec3<f32> = position_metallic.rgb;
    var metallic: f32 = position_metallic.a;
    var normal: vec3<f32> = normal_roughness.rgb;
    var roughness: f32 = normal_roughness.a;
    var albedo: vec3<f32> = albedo_occlusion.rgb;
    var occlusion: f32 = albedo_occlusion.a;

#ifdef DEFERRED_LIGHTING_UNLIT
    // Return unlit color
    return vec4<f32>(albedo.rgb, 1.0);
#endif

    // Convert to specular glossiness
    var f0 = vec3(0.04, 0.04, 0.04);
    var diffuse = albedo * (vec3(1.0, 1.0, 1.0) - f0) * (1.0 - metallic);
    var specular = mix(f0, albedo, metallic);

    var alpha_roughness = roughness * roughness;
    var reflectance = max(max(specular.r, specular.g), specular.b);
    var reflectance_0 = specular.rgb;
    var reflectance_90 = vec3<f32>(clamp(reflectance * 50.0, 0.0, 1.0));

    var material_info: MaterialInfo;
    material_info.roughness = roughness;
    material_info.alpha_roughness = alpha_roughness;
    material_info.diffuse = diffuse;
    material_info.specular = specular;
    material_info.reflectance_0 = reflectance_0;
    material_info.reflectance_90 = reflectance_90;

#ifdef DOUBLE_SIDED
    if dot(normal, view) < 0 {
        normal = -normal;
    }
#endif

    var color = vec3<f32>(0.0, 0.0, 0.0);
    var view = normalize(cam_pos - position);

    {
        var point_to_light: vec3<f32> = DIRECTIONAL_LIGHT_DIRECTION;
        var shade: vec3<f32> = get_point_shade(material_info, point_to_light, normal, view);

        color += DIRECTIONAL_LIGHT_COLOR.a * DIRECTIONAL_LIGHT_COLOR.rgb * shade;
    }

    for (var i: u32 = 0; i < 6; i++) {
        // var light = u_lights[i];
        // var shadow_factor: f32 = do_spot_shadow(position, light);
        // if light.type == 0 {  // directional light
        //     color += do_directional_light(light, );
        // } else if light.type == 1 {  // pointlight
            var point_to_light: vec3<f32> = LIGHT_POSITIONS[i] - position;
            var distance: f32 = length(point_to_light);
            var attenuation: f32 = get_range_attenuation(5.0, distance);
            var shade: vec3<f32> = get_point_shade(material_info, point_to_light, normal, view);

            color += attenuation * LIGHT_COLORS[i].a * LIGHT_COLORS[i].rgb * shade;
        // } else if light.type == 2 {  // spotlight
        //     color += do_spot_light(light, );
        // }
    }

    // occlusion
    color = color * occlusion;

    // emissive
    color += emissive.rgb;

    return vec4<f32>(color, 1.0);
}

fn get_range_attenuation(range: f32, distance: f32) -> f32 {
    if range < 0.0 {
        return 1.0;
    }
    return max(min(1.0 - pow(distance / range, 4.0), 1.0), 0.0);
}

fn get_point_shade(material_info: MaterialInfo, point_to_light: vec3<f32>, normal: vec3<f32>, view: vec3<f32>) -> vec3<f32> {
    var angular_info: AngularInfo = get_angular_info(point_to_light, normal, view);

    if angular_info.n_dot_l > 0.0 && angular_info.n_dot_v > 0.0 {
        var f: vec3<f32> = get_specular_reflection(material_info, angular_info);
        var vis: f32 = get_visibility_occlusion(material_info, angular_info);
        var d = get_microfacet_distribution(material_info, angular_info);

        var diffuse_contrib: vec3<f32> = (1.0 - f) * (material_info.diffuse / CONST_PI);
        var specular_contrib: vec3<f32> = f * vis * d;

        return angular_info.n_dot_l * (diffuse_contrib + specular_contrib);
    } else {
        return vec3(0.0, 0.0, 0.0);
    }
}

fn get_angular_info(point_to_light: vec3<f32>, normal: vec3<f32>, view: vec3<f32>) -> AngularInfo {
    var out: AngularInfo;

    var n = normalize(normal);  // Outward direction of surface point
    var v = normalize(view);  // Direction from surface point to view
    var l = normalize(point_to_light);  // Direction from surface point to light
    var h = normalize(l + view);  // Direction of the vector between l and v

    out.n_dot_l = clamp(dot(n, l), 0.0, 1.0);
    out.n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    out.n_dot_h = clamp(dot(n, h), 0.0, 1.0);
    out.l_dot_h = clamp(dot(l, h), 0.0, 1.0);
    out.v_dot_h = clamp(dot(v, h), 0.0, 1.0);

    return out;    
}

fn get_specular_reflection(material_info: MaterialInfo, angular_info: AngularInfo) -> vec3<f32> {
    return material_info.reflectance_0 +
        (material_info.reflectance_90 - material_info.reflectance_0) *
        pow(clamp(1.0 - angular_info.v_dot_h, 0.0, 1.0), 5.0);
}

fn get_visibility_occlusion(material_info: MaterialInfo, angular_info: AngularInfo) -> f32 {
    var n_dot_l: f32 = angular_info.n_dot_l;
    var n_dot_v: f32 = angular_info.n_dot_v;
    var alpha_roughness_sq: f32 = material_info.alpha_roughness * material_info.alpha_roughness;
    var ggxv: f32 = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - alpha_roughness_sq) + alpha_roughness_sq);
    var ggxl: f32 = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - alpha_roughness_sq) + alpha_roughness_sq);
    var ggx: f32 = ggxv + ggxl;
    
    if ggx > 0.0 {
        return 0.5 / ggx;
    } else {
        return 0.0;
    }
}

fn get_microfacet_distribution(material_info: MaterialInfo, angular_info: AngularInfo) -> f32 {
    var alpha_roughness_sq: f32 = material_info.alpha_roughness * material_info.alpha_roughness;
    // var f: f32 = (angular_info.n_dot_h * alpha_roughness_sq - angular_info.n_dot_h) * angular_info.n_dot_h * 1.0;
    var f: f32 = (angular_info.n_dot_h * angular_info.n_dot_h * (alpha_roughness_sq - 1.0) + 1.0);
    return alpha_roughness_sq / (CONST_PI * f * f + 0.000001);
}

fn get_position_metallic(in: SimpleQuadOutput) -> vec4<f32> {
    return textureSample(position_metallic_texture, position_metallic_sampler, in.uv, in.view_index);
}

fn get_normal_roughness(in: SimpleQuadOutput) -> vec4<f32> {
    return textureSample(normal_roughness_texture, normal_roughness_sampler, in.uv, in.view_index);
}

fn get_albedo_occlusion(in: SimpleQuadOutput) -> vec4<f32> {
    return textureSample(albedo_occlusion_texture, albedo_occlusion_sampler, in.uv, in.view_index);
}

fn get_emissive(in: SimpleQuadOutput) -> vec4<f32> {
    return textureSample(emissive_texture, emissive_sampler, in.uv, in.view_index);
}
