#include postproc::types

@group(0) @binding(0)
var input_sampler: sampler;
@group(0) @binding(1)
var input_texture: texture_2d_array<f32>;

struct BloomParams {
    threshold: f32,
    intensity: f32,
    knee_width: f32,
    _padding: u32,
}

@group(1) @binding(0)
var<uniform> u_bloom_params: BloomParams;

// const KERNEL_WEIGHTS_9 = array<f32, 5>(0.153170, 0.144893, 0.122649, 0.092902, 0.063157);
const KERNEL_WEIGHTS_9 = array<f32, 5>(0.235833, 0.198063, 0.117294, 0.048968, 0.014408);

@fragment
fn brightness_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    let tex_dims = vec2<f32>(textureDimensions(input_texture).xy);
    let texel_size = vec2<f32>(1.0 / tex_dims.x, 1.0 / tex_dims.y);
    
    let uv00 = in.uv - texel_size * 0.25;
    let uv10 = uv00 + vec2<f32>(texel_size.x, 0.0);
    let uv01 = uv00 + vec2<f32>(0.0, texel_size.y);
    let uv11 = uv00 + texel_size * 0.5;

    let c00 = textureSample(input_texture, input_sampler, uv00, in.view_index);
    let c10 = textureSample(input_texture, input_sampler, uv10, in.view_index);
    let c01 = textureSample(input_texture, input_sampler, uv01, in.view_index);
    let c11 = textureSample(input_texture, input_sampler, uv11, in.view_index);

    let f00 = filter_brightness(c00.rgb);
    let f10 = filter_brightness(c10.rgb);
    let f01 = filter_brightness(c01.rgb);
    let f11 = filter_brightness(c11.rgb);

    let filtered_color = (f00 + f10 + f01 + f11) * 0.25;
    return vec4<f32>(filtered_color, 1.0);
}

fn filter_brightness(color: vec3<f32>) -> vec3<f32> {
    let brightness: f32 = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));

    let threshold = u_bloom_params.threshold;
    let knee_width = u_bloom_params.knee_width;
    let intensity = u_bloom_params.intensity;
    let epsilon = 1e-5;

    let knee_start = threshold - knee_width;
    let excess = brightness - knee_start;

    var bloom_brightness_factor: f32 = 0.0;

    if knee_width > epsilon && brightness > knee_start {
        if brightness < threshold {
            bloom_brightness_factor = (excess * excess) / (2.0 * knee_width);
        } else {
            bloom_brightness_factor = (brightness - threshold) + (knee_width / 2.0);
        }
    } else {
        bloom_brightness_factor = max(0.0, brightness - threshold);
    }

    bloom_brightness_factor = max(0.0, bloom_brightness_factor);

    let safe_brightness = max(brightness, epsilon);
    let scale_factor = bloom_brightness_factor / safe_brightness;
    let filtered_color = color.rgb * scale_factor;

    return filtered_color;
}

@fragment
fn downsample_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    let tex_dims = vec2<f32>(textureDimensions(input_texture).xy);
    let texel_size = vec2<f32>(1.0 / tex_dims.x, 1.0 / tex_dims.y);
    
    let uv00 = in.uv - texel_size * 0.25;
    let uv10 = uv00 + vec2<f32>(texel_size.x, 0.0);
    let uv01 = uv00 + vec2<f32>(0.0, texel_size.y);
    let uv11 = uv00 + texel_size * 0.5;

    let c00 = textureSample(input_texture, input_sampler, uv00, in.view_index);
    let c10 = textureSample(input_texture, input_sampler, uv10, in.view_index);
    let c01 = textureSample(input_texture, input_sampler, uv01, in.view_index);
    let c11 = textureSample(input_texture, input_sampler, uv11, in.view_index);

    let avg_color = (c00 + c10 + c01 + c11) * 0.25;
    return vec4<f32>(avg_color.rgb, 1.0);
}

@fragment
fn blur_horizontal_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    let tex_dims = vec2<f32>(textureDimensions(input_texture).xy);
    let texel_size = vec2<f32>(1.0 / tex_dims.x, 1.0 / tex_dims.y);
    var blurred_color = textureSample(input_texture, input_sampler, in.uv, in.view_index) * KERNEL_WEIGHTS_9[0];

    for (var i: i32 = 1; i < 5; i = i + 1) {
        let offset = vec2<f32>(texel_size.x, 0.0);
        let weight = KERNEL_WEIGHTS_9[i];

        blurred_color += textureSample(input_texture, input_sampler, in.uv + offset, in.view_index) * weight;
        blurred_color += textureSample(input_texture, input_sampler, in.uv - offset, in.view_index) * weight;
    }

    return vec4<f32>(blurred_color.rgb, 1.0);
}

@fragment
fn blur_vertical_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    let tex_dims = vec2<f32>(textureDimensions(input_texture).xy);
    let texel_size = vec2<f32>(1.0 / tex_dims.x, 1.0 / tex_dims.y);
    var blurred_color = textureSample(input_texture, input_sampler, in.uv, in.view_index) * KERNEL_WEIGHTS_9[0];

    for (var i: i32 = 1; i < 5; i = i + 1) {
        let offset = vec2<f32>(0.0, texel_size.y);
        let weight = KERNEL_WEIGHTS_9[i];

        blurred_color += textureSample(input_texture, input_sampler, in.uv + offset, in.view_index) * weight;
        blurred_color += textureSample(input_texture, input_sampler, in.uv - offset, in.view_index) * weight;
    }

    return vec4<f32>(blurred_color.rgb, 1.0);}

@fragment
fn upsample_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    let low_res_color = textureSample(input_texture, input_sampler, in.uv, in.view_index);
    let scaled_color = low_res_color.rgb;

    return vec4<f32>(scaled_color.rgb, 1.0);
}

#ifdef COMPOSITE_PASS
@group(2) @binding(0)
var bloom_sampler: sampler;
@group(2) @binding(1)
var bloom_texture: texture_2d_array<f32>;

@fragment
fn composite_main(in: SimpleQuadOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(input_texture, input_sampler, in.uv, in.view_index);
    let bloom_color = textureSample(bloom_texture, bloom_sampler, in.uv, in.view_index);
    let scaled_bloom = bloom_color.rgb * u_bloom_params.intensity;
    let final_color = scene_color.rgb + scaled_bloom;

    // return vec4<f32>(scaled_bloom, scene_color.a);
    return vec4<f32>(final_color, scene_color.a);
}
#endif
