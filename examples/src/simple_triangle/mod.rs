#[allow(dead_code)]
struct App {
    objects: Vec<xrds::Object>,
}

use xrds::*;

impl RuntimeHandler for App {
    fn on_construct(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_begin(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        println!("[SimpleTriangle] on_begin()");
        Ok(())
    }

    fn on_deconstruct(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_end(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_resumed(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_suspended(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_update(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn run() -> anyhow::Result<()> {
    let runtime = Runtime::new().expect("Could not create xrds runtime");
    let app = App { objects: vec![] };

    runtime.run(app)?;

    Ok(())
}
