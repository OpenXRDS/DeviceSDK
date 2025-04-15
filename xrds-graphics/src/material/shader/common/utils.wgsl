#ifndef COMMON_UTILS
#define COMMON_UTILS

fn inverse_transpose_3x3(in: mat3x3<f32>) -> mat3x3<f32> {
    let x = cross(in[1], in[2]);
    let y = cross(in[2], in[0]);
    let z = cross(in[0], in[1]);
    let det = dot(in[2], z);
    return mat3x3<f32> (
        x / det,
        y / det,
        z / det
    );
}

#endif