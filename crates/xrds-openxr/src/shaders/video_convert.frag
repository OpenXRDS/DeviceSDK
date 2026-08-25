#version 450

// Copy a decoded video frame into an ordinary RGBA image.
//
// The YUV -> RGB conversion is *not* here: it happens inside the sampler, which
// carries an immutable VkSamplerYcbcrConversion built from the buffer's external
// format. That is the whole reason this pass exists — wgpu cannot express an
// immutable sampler or an external format, so Vulkan does the sampling and hands
// wgpu a normal texture afterwards.
//
// It also means this shader must not use texelFetch, gather, or offset sampling:
// none are permitted on a Ycbcr-converted image.
layout(set = 0, binding = 0) uniform sampler2D u_video;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    // Opaque: a video surface's transparency is the material's business, not the
    // decoder's.
    o_color = vec4(texture(u_video, v_uv).rgb, 1.0);
}
