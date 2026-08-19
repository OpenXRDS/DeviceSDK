use xrds::sdk::{
    primitives::{XrdsCube, XrdsPlane3D, XrdsSphere, XrdsTetrahedron},
    world::{
        lights::{XrdsAmbientLight, XrdsDirectionalLight, XrdsPointLight},
        XrdsCamera,
    },
    Plane3DGeometryParams, SphereGeometryParams, TransformParams, XrdsColor,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

#[derive(Default)]
struct ParentChildApp {
    cube_handle: Option<Handle<XrdsCube>>,
    sphere_handle: Option<Handle<XrdsSphere>>,
    plane_handle: Option<Handle<XrdsPlane3D>>,
}

impl XrdsApp for ParentChildApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("HierarchyCamera")
                .near(0.1)
                .far(200.0)
                .order(0)
                .at([7.5, 5.5, 9.0])
                .looking_at([0.0, 1.2, 0.0])
        });

        let _ambient = api.spawn(&{
            let mut light = XrdsAmbientLight::new().with_name("HierarchyAmbient");
            light.brightness = 120.0;
            light
        });

        let _sun = api.spawn(&{
            let mut light = XrdsDirectionalLight::new().with_name("HierarchySun");
            light.transform.translation = [5.0, 8.0, 4.0];
            light.illuminance = 12_000.0;
            light.shadows = true;
            light
        });

        let _point = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("HierarchyPointLight");
            light.transform.translation = [-3.0, 4.0, 3.0];
            light.intensity = 200_000.0;
            light.range = 25.0;
            light.shadows = true;
            light.color = XrdsColor::srgb(1.0, 0.92, 0.82);
            light
        });

        let cube_handle = api.spawn(&{
            let mut cube = XrdsCube::new().with_name("ParentCube");
            cube.transform.translation = [0.0, 1.1, 0.0];
            cube.size = [1.5, 1.5, 1.5];
            cube
        });
        api.set_material_base_color(&cube_handle, XrdsColor::srgb(0.88, 0.38, 0.22));

        let sphere_handle = api.spawn(&{
            let mut sphere = XrdsSphere::new().with_name("ChildSphere");
            sphere.transform.translation = [2.2, 1.0, 0.0];
            sphere
        });
        api.set_material_base_color(&sphere_handle, XrdsColor::srgb(0.2, 0.72, 0.92));
        api.queue_update(&sphere_handle, SphereGeometryParams { radius: 0.7 });

        let plane_handle = api.spawn(&{
            let mut plane = XrdsPlane3D::new().with_name("GrandchildPlane");
            plane.transform.translation = [0.0, 1.2, 0.0];
            // Was `rotation_euler_xyz_deg = [-90, 0, 0]`, which did nothing at all:
            // the runtime reads the quaternion, and this example had been shipping
            // an unrotated plane. `set_euler_deg` actually applies it.
            plane.transform.set_euler_deg(-90.0, 0.0, 0.0);
            plane
        });
        api.set_material_base_color(&plane_handle, XrdsColor::srgb(0.95, 0.88, 0.38));
        api.queue_update(&plane_handle, Plane3DGeometryParams { size: [1.4, 1.4] });

        let mut tetra = XrdsTetrahedron::new();
        tetra.transform.translation = [-2.2, 1.0, 0.0];
        tetra.vertices = [
            [0.0, 0.5, 0.0],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.0, -0.5, -0.5],
        ]
        .map(Into::into);
        api.spawn(&tetra);

        // Hierarchy is established after the entities exist.
        let cube_id = api
            .id_of(&cube_handle)
            .expect("cube should have a registered XRDS id");
        let sphere_id = api
            .id_of(&sphere_handle)
            .expect("sphere should have a registered XRDS id");
        api.set_parent(&sphere_handle, Some(cube_id));
        api.set_parent(&plane_handle, Some(sphere_id));

        self.cube_handle = Some(cube_handle);
        self.sphere_handle = Some(sphere_handle);
        self.plane_handle = Some(plane_handle);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let cube = self
            .cube_handle
            .as_ref()
            .expect("cube handle should be initialized during setup");
        let sphere = self
            .sphere_handle
            .as_ref()
            .expect("sphere handle should be initialized during setup");
        let plane = self
            .plane_handle
            .as_ref()
            .expect("plane handle should be initialized during setup");

        let t = ctx.elapsed_secs();

        // Move and rotate the parent cube. Children inherit this motion.
        let cube_yaw = t * 0.8;
        let cube_half_yaw = cube_yaw * 0.5;
        ctx.queue_update(
            cube,
            TransformParams {
                translation: [(t * 0.7).sin() * 1.2, 1.1 + 0.25 * (t * 1.4).sin(), 0.0],
                rotation_quat_xyzw: [0.0, cube_half_yaw.sin(), 0.0, cube_half_yaw.cos()],
                scale: [1.0, 1.0, 1.0],
            },
        );

        // Animate the sphere in the cube's local space.
        let sphere_pitch = t * 1.9;
        let sphere_half_pitch = sphere_pitch * 0.5;
        ctx.queue_update(
            sphere,
            TransformParams {
                translation: [
                    2.2 + 0.3 * (t * 2.0).cos(),
                    1.0 + 0.25 * (t * 3.0).sin(),
                    0.0,
                ],
                rotation_quat_xyzw: [sphere_half_pitch.sin(), 0.0, 0.0, sphere_half_pitch.cos()],
                scale: [1.0, 1.0, 1.0],
            },
        );

        // The plane is a grandchild, so it inherits both cube and sphere motion.
        let plane_roll = t * 2.4;
        let plane_half_roll = plane_roll * 0.5;
        ctx.queue_update(
            plane,
            TransformParams {
                translation: [0.0, 1.2, 0.0],
                rotation_quat_xyzw: [0.0, 0.0, plane_half_roll.sin(), plane_half_roll.cos()],
                scale: [1.0, 1.0, 1.0],
            },
        );
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(ParentChildApp::default())
        .expect("failed to run parent_child");
}
