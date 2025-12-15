use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "2dShapes".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let shapes = [
        meshes.add(Circle::new(50.0)),
        meshes.add(CircularSector::new(50.0, 1.0)),
        meshes.add(CircularSegment::new(50.0, 1.25)),
        meshes.add(Ellipse::new(25.0, 50.0)),
        meshes.add(Annulus::new(25.0, 50.0)),
        meshes.add(Capsule2d::new(25.0, 50.0)),
        meshes.add(Rhombus::new(75.0, 100.0)),
        meshes.add(Rectangle::new(50.0, 100.0)),
        meshes.add(RegularPolygon::new(50.0, 6)),
        meshes.add(Triangle2d::new(
            Vec2::Y * 50.0,
            Vec2::new(-50.0, -50.0),
            Vec2::new(50.0, -50.0),
        )),
        meshes.add(Segment2d::new(
            Vec2::new(-50.0, 50.0),
            Vec2::new(50.0, -50.0),
        )),
        meshes.add(Polyline2d::new(vec![
            Vec2::new(-50.0, 50.0),
            Vec2::new(0.0, -50.0),
            Vec2::new(50.0, 50.0),
        ])),
    ];
    let num_shapes = shapes.len();

    for (i, shape) in shapes.into_iter().enumerate() {
        let color = Color::hsl(360. * i as f32 / num_shapes as f32, 0.95, 0.7);

        commands.spawn((
            Mesh2d(shape),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(color))),
            Transform::from_xyz(
                -900.0 / 2. + i as f32 / (num_shapes - 1) as f32 * 900.0,
                0.0,
                0.0,
            ),
        ));
    }
}
