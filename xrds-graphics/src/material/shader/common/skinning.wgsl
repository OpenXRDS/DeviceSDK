#include common::utils

#ifndef SKINNING_WGSL
#define SKINNING_WGSL

#ifndef SKINNING_GROUP_INDEX
#define SKINNING_GROUP_INDEX 2
#endif

#ifdef VERTEX_INPUT_SKINNED
@group(${SKINNING_GROUP_INDEX}) @binding(0)
var<uniform> u_joint_matrices: array<mat4x4<f32>, 256u>

fn get_skinning_model(indices: vec4<u32>, weights: vec4<f32>) -> mat4x4<f32> {
    return 
        weights.x * u_joint_matrices.data[indices.x] +
        weights.y * u_joint_matrices.data[indices.y] +
        weights.z * u_joint_matrices.data[indices.z] + 
        weights.w * u_joint_matrices.data[indices.w];
}

fn skin_normal(model: mat4x4<f32>, normal: vec3<f32>) -> vec3<f32> {
    return inverse_transpose_3x3(
        mat3x3<f32>(
            model[0].xyz,
            model[1].xyz,
            model[2].xyz
        )
    ) * normal;
}
#endif

#endif  // SKINNING_WGSL