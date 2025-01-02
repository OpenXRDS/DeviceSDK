use scene::Scene;

pub mod scene;

pub struct Graphics {}

pub struct Renderer {}

impl Renderer {
    pub fn load_scene(&mut self) -> anyhow::Result<Scene> {
        Ok(Scene {})
    }
}
