#version 450

// Fullscreen triangle, no vertex buffer.
//
// Three vertices generated from gl_VertexIndex cover the whole viewport with one
// oversized triangle. Cheaper than a quad (no diagonal seam, no second triangle,
// no buffer to allocate or bind) and it is the standard shape for a pass whose
// only job is to run the fragment shader once per pixel.
layout(location = 0) out vec2 v_uv;

void main() {
    v_uv = vec2(float((gl_VertexIndex << 1) & 2), float(gl_VertexIndex & 2));
    gl_Position = vec4(v_uv * 2.0 - 1.0, 0.0, 1.0);
}
