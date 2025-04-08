#[allow(dead_code)]
struct App {
    // objects: Vec<xrds::Object>,
}

use std::time::Duration;

use xrds::*;

impl RuntimeHandler for App {
    fn on_construct(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_begin(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        println!("[SimpleTriangle] on_begin()");
        Ok(())
    }

    fn on_deconstruct(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_end(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_resumed(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_suspended(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_update(&mut self, _context: &mut xrds::Context, _diff: Duration) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn run() -> anyhow::Result<()> {
    let runtime = Runtime::new().expect("Could not create xrds runtime");
    let app = App {};

    runtime.run(app)?;

    Ok(())
}
