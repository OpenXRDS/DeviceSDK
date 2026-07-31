use xrds::sdk::{
    primitives::{XrdsCube, XrdsPlane3D, XrdsSphere},
    world::{
        lights::{XrdsAmbientLight, XrdsDirectionalLight, XrdsPointLight},
        XrdsCamera,
    },
    XrdsColor, XrdsId,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

#[derive(Default)]
struct ParentChildQueuedApp {
    plane_id: Option<XrdsId>,
    cube_id: Option<XrdsId>,
    sphere_id: Option<XrdsId>,
    materials_applied: bool,
}

impl XrdsApp for ParentChildQueuedApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let _camera = api.spawn(&{
            XrdsCamera::perspective(48.0)
                .with_name("QueuedHierarchyCamera")
                .near(0.1)
                .far(250.0)
                .order(0)
                .at([8.5, 6.0, 10.5])
                .looking_at([0.0, 1.8, 0.0])
        });

        let _ambient = api.spawn(&{
            let mut light = XrdsAmbientLight::new();
            light.brightness = 100.0;
            light
        });

        let _sun = api.spawn(&{
            let mut light = XrdsDirectionalLight::new().with_name("QueuedHierarchySun");
            light.transform.translation = [6.0, 9.0, 5.0];
            light.illuminance = 14_000.0;
            light.shadows = true;
            light
        });

        let _point = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("QueuedHierarchyPointLight");
            light.transform.translation = [-4.0, 4.5, 2.5];
            light.intensity = 180_000.0;
            light.range = 30.0;
            light.shadows = true;
            light.color = XrdsColor::srgb(0.86, 0.92, 1.0);
            light
        });

        let light22 = XrdsPointLight::new().with_name("QueuedHierarchyPointLight2");
        let _lighthandle = api.spawn(&light22);

        let mut plane = XrdsPlane3D::new().with_name("RootPlane");
        plane.transform.translation = [0.0, 0.8, 0.0];
        plane.transform.rotation_euler_xyz_deg = [-90.0, 0.0, 0.0];
        let plane_id = api.queue_spawn(plane);

        let mut cube = XrdsCube::new().with_name("ChildCube");
        cube.transform.translation = [0.0, 1.4, 0.0];
        let cube_id = api.queue_spawn(cube);
        api.queue_set_parent(cube_id, Some(plane_id));

        let mut sphere = XrdsSphere::new().with_name("GrandchildSphere");
        sphere.transform.translation = [1.8, 1.2, 0.0];
        let sphere_id = api.queue_spawn(sphere);
        api.queue_set_parent(sphere_id, Some(cube_id));

        self.plane_id = Some(plane_id);
        self.cube_id = Some(cube_id);
        self.sphere_id = Some(sphere_id);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        if self.materials_applied {
            return;
        }

        let Some(plane) = self
            .plane_id
            .and_then(|id| ctx.handle_of::<XrdsPlane3D>(id))
        else {
            return;
        };
        let Some(cube) = self.cube_id.and_then(|id| ctx.handle_of::<XrdsCube>(id)) else {
            return;
        };
        let Some(sphere) = self
            .sphere_id
            .and_then(|id| ctx.handle_of::<XrdsSphere>(id))
        else {
            return;
        };

        ctx.set_material_base_color(&plane, XrdsColor::srgb(0.92, 0.84, 0.32));
        ctx.set_material_base_color(&cube, XrdsColor::srgb(0.84, 0.32, 0.2));
        ctx.set_material_base_color(&sphere, XrdsColor::srgb(0.18, 0.7, 0.9));
        self.materials_applied = true;
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(ParentChildQueuedApp::default())
        .expect("failed to run parent_child_queued");
}
