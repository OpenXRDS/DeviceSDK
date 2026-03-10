struct App {}

use xrds::*;

impl RuntimeHandler for App {}

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "SimpleXRScene".to_owned(),
        enable_xr: true,
    });
    let app = App {};

    runtime.run(app).expect("Could not run application");
}
