struct App {}

use xrds::*;

impl RuntimeHandler for App {
    fn on_begin(&mut self) {
        println!("[SimpleTriangle] on_begin()")
    }
    fn on_construct(&mut self) {}
    fn on_deconstruct(&mut self) {}
    fn on_end(&mut self) {}
    fn on_resumed(&mut self) {}
    fn on_suspended(&mut self) {}
    fn on_update(&mut self) {}
}

pub fn run() {
    let runtime = Runtime::new().expect("Could not create xrds runtime");
    let app = App {};

    runtime.run(app).expect("Could not run application");
}
