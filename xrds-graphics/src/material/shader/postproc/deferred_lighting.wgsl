#include postproc::types
#include common::view_params
#include common::light_params
#include pbr::gbuffer_params

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

@fragment
fn fs_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    // Do deferred lighting
    var view_params = get_view_params(in.view_index);
    var cam_pos = view_params.world_position;

    var albedo_occlusion = get_albedo_occlusion(in.uv, in.view_index);
    var position_metallic = get_position_metallic(in.uv, in.view_index);
    var normal_roughness = get_normal_roughness(in.uv, in.view_index);
    var emissive: vec4<f32> = get_emissive(in.uv, in.view_index);

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


    var color = vec3<f32>(0.0, 0.0, 0.0);
    var view = normalize(cam_pos - position);
    
    var light_direction = vec3<f32>(0.0, 0.0, 0.0);

    var light_count: u32 = get_light_count();
    for (var i: u32 = 0; i < light_count; i++) {
        var light: Light = get_light_ith(i32(i));
        var light_type: u32 = light.ty;

        if light_type == LIGHT_TYPE_DIRECTIONAL {
            light_direction = light.direction;
            var shade: vec3<f32> = get_point_shade(material_info, normalize(-light.direction), normal, view);
            color += light.intensity * light.color * shade;
        } else if light_type == LIGHT_TYPE_POINT {
            var point_to_light: vec3<f32> = light.position - position;
            var distance: f32 = length(point_to_light);
            var attenuation: f32 = get_range_attenuation(light.range, distance);
            var shade: vec3<f32> = get_point_shade(material_info, point_to_light, normal, view);

            color += attenuation * light.intensity * light.color * shade;
        } else if light_type == LIGHT_TYPE_SPOT {
            // todo!()
        }
    }

    // occlusion
    color = color * occlusion;

    // emissive
    color += emissive.rgb;

    color = vec3<f32>(get_shadowmap(0, in.uv), 0.0);

    // color = normal * 0.5 + 0.5;

    // color = light_direction;
    // color = normalize(vec3<f32>(-1.0, 1.0, 1.0));
    // color = normal;
    // color = position;
    // color = vec3<f32>(metallic, roughness, occlusion);

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