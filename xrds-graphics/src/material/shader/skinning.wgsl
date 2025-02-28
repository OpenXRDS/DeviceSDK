#define_import_path shader::skinning

#ifdef VERTEX_INPUT_SKINNED
@group(2) @binding(0)
var<uniform> u_joint_matrices: array<mat4x4<f32>, 256u>
#endif

fn get_skinning_model(indices: vec4<u32>, weights: vec4<f32>) -> mat4x4<f32> {
    return 
        weights.x * u_joint_matrices.data[indices.x] +
        weights.y * u_joint_matrices.data[indices.y] +
        weights.z * u_joint_matrices.data[indices.z] + 
        weights.w * u_joint_matrices.data[indices.w];
}

fn inverse_transpose_3x3(in: mat3x3<f32>) -> mat3x3<f32> {
    let x = cross(in[1], in[2]);
    let y = cross(in[2], in[0]);
    let z = cross(in[0], in[1]);
    let det = dot(in[2], z);
    return mat3x3<f32> {
        x / det,
        y / det,
        z / det
    };
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