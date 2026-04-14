use xrds::sdk::{
    primitives::XrdsCube,
    world::{lights::XrdsAmbientLight, XrdsCamera},
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp};

#[derive(Default)]
struct DescriptorApp {
    cube_handle: Option<Handle<XrdsCube>>,
}

impl XrdsApp for DescriptorApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("DescriptorCamera")
                .near(0.1)
                .far(200.0)
                .order(0)
                .at([7.5, 5.5, 9.0])
                .looking_at([0.0, 1.2, 0.0])
        });

        let _ambient = api.spawn(&{
            let mut light = XrdsAmbientLight::new().with_name("DescriptorAmbient");
            light.brightness = 120.0;
            light
        });

        let mut cube = XrdsCube::new().with_name("DescriptorCube");
        cube.size = [1.5, 1.5, 1.5];
        cube.transform.translation = [0.0, 1.1, 0.0];
        self.cube_handle = Some(api.spawn(&cube));
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(DescriptorApp::default())
        .expect("failed to run descriptor_app");
}
