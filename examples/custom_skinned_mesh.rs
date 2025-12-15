use std::f32::consts::FRAC_PI_2;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
        Indices, VertexAttributeValues,
    },
};
use rand::{Rng, SeedableRng};
use wgpu::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat};
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "CustomSkinnedMesh".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        on_update.add_systems(joint_animation);
    }
}

#[derive(Component)]
struct AnimatedJoint(isize);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut skinned_mesh_inverse_bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    let debug_image = images.add(uv_debug_texture());
    // Create a camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.5, 2.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Create inverse bindpose matrices for a skeleton consists of 2 joints
    let inverse_bindposes = skinned_mesh_inverse_bindposes_assets.add(vec![
        Mat4::from_translation(Vec3::new(-0.5, -1.0, 0.0)),
        Mat4::from_translation(Vec3::new(-0.5, -1.0, 0.0)),
    ]);

    // Create a mesh
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    // Set mesh vertex positions
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [1.0, 0.5, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.5, 0.0],
            [1.0, 1.5, 0.0],
            [0.0, 2.0, 0.0],
            [1.0, 2.0, 0.0],
        ],
    )
    // Add UV coordinates that map the left half of the texture since its a 1 x
    // 2 rectangle.
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![
            [0.0, 0.00],
            [0.5, 0.00],
            [0.0, 0.25],
            [0.5, 0.25],
            [0.0, 0.50],
            [0.5, 0.50],
            [0.0, 0.75],
            [0.5, 0.75],
            [0.0, 1.00],
            [0.5, 1.00],
        ],
    )
    // Set mesh vertex normals
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 10])
    // Set mesh vertex joint indices for mesh skinning.
    // Each vertex gets 4 indices used to address the `JointTransforms` array in the vertex shader
    //  as well as `SkinnedMeshJoint` array in the `SkinnedMesh` component.
    // This means that a maximum of 4 joints can affect a single vertex.
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        // Need to be explicit here as [u16; 4] could be either Uint16x4 or Unorm16x4.
        VertexAttributeValues::Uint16x4(vec![
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
        ]),
    )
    // Set mesh vertex joint weights for mesh skinning.
    // Each vertex gets 4 joint weights corresponding to the 4 joint indices assigned to it.
    // The sum of these weights should equal to 1.
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_JOINT_WEIGHT,
        vec![
            [1.00, 0.00, 0.0, 0.0],
            [1.00, 0.00, 0.0, 0.0],
            [0.75, 0.25, 0.0, 0.0],
            [0.75, 0.25, 0.0, 0.0],
            [0.50, 0.50, 0.0, 0.0],
            [0.50, 0.50, 0.0, 0.0],
            [0.25, 0.75, 0.0, 0.0],
            [0.25, 0.75, 0.0, 0.0],
            [0.00, 1.00, 0.0, 0.0],
            [0.00, 1.00, 0.0, 0.0],
        ],
    )
    // Tell bevy to construct triangles from a list of vertex indices,
    // where each 3 vertex indices form a triangle.
    .with_inserted_indices(Indices::U16(vec![
        0, 1, 3, 0, 3, 2, 2, 3, 5, 2, 5, 4, 4, 5, 7, 4, 7, 6, 6, 7, 9, 6, 9, 8,
    ]));

    let mesh = meshes.add(mesh);

    // We're seeding the PRNG here to make this example deterministic for testing purposes.
    // This isn't strictly required in practical use unless you need your app to be deterministic.
    let mut rng = rand::rngs::StdRng::from_os_rng();

    for i in -5..5 {
        // Create joint entities
        let joint_0 = commands
            .spawn(Transform::from_xyz(
                i as f32 * 1.5,
                0.0,
                // Move quads back a small amount to avoid Z-fighting and not
                // obscure the transform gizmos.
                -(i as f32 * 0.01).abs(),
            ))
            .id();
        let joint_1 = commands.spawn((AnimatedJoint(i), Transform::IDENTITY)).id();

        // Set joint_1 as a child of joint_0.
        commands.entity(joint_0).add_children(&[joint_1]);

        // Each joint in this vector corresponds to each inverse bindpose matrix in `SkinnedMeshInverseBindposes`.
        let joint_entities = vec![joint_0, joint_1];

        // Create skinned mesh renderer. Note that its transform doesn't affect the position of the mesh.
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(
                    rng.random_range(0.0..1.0),
                    rng.random_range(0.0..1.0),
                    rng.random_range(0.0..1.0),
                ),
                base_color_texture: Some(debug_image.clone()),
                ..default()
            })),
            SkinnedMesh {
                inverse_bindposes: inverse_bindposes.clone(),
                joints: joint_entities,
            },
        ));
    }
}

fn joint_animation(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &AnimatedJoint)>,
    mut gizmos: Gizmos,
) {
    for (mut transform, animated_joint) in &mut query {
        match animated_joint.0 {
            -5 => {
                transform.rotation =
                    Quat::from_rotation_x(FRAC_PI_2 * ops::sin(time.elapsed_secs()));
            }
            -4 => {
                transform.rotation =
                    Quat::from_rotation_y(FRAC_PI_2 * ops::sin(time.elapsed_secs()));
            }
            -3 => {
                transform.rotation =
                    Quat::from_rotation_z(FRAC_PI_2 * ops::sin(time.elapsed_secs()));
            }
            -2 => {
                transform.scale.x = ops::sin(time.elapsed_secs()) + 1.0;
            }
            -1 => {
                transform.scale.y = ops::sin(time.elapsed_secs()) + 1.0;
            }
            0 => {
                transform.translation.x = 0.5 * ops::sin(time.elapsed_secs());
                transform.translation.y = ops::cos(time.elapsed_secs());
            }
            1 => {
                transform.translation.y = ops::sin(time.elapsed_secs());
                transform.translation.z = ops::cos(time.elapsed_secs());
            }
            2 => {
                transform.translation.x = ops::sin(time.elapsed_secs());
            }
            3 => {
                transform.translation.y = ops::sin(time.elapsed_secs());
                transform.scale.x = ops::sin(time.elapsed_secs()) + 1.0;
            }
            _ => (),
        }
        // Show transform
        let mut axis = *transform;
        axis.translation.x += animated_joint.0 as f32 * 1.5;
        gizmos.axes(axis, 1.0);
    }
}

fn uv_debug_texture() -> Image {
    const TEXTURE_SIZE: usize = 8;

    let mut palette: [u8; 32] = [
        255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
        198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
    ];

    let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
    for y in 0..TEXTURE_SIZE {
        let offset = TEXTURE_SIZE * y * 4;
        texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
        palette.rotate_right(4);
    }

    Image::new_fill(
        Extent3d {
            width: TEXTURE_SIZE as u32,
            height: TEXTURE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
