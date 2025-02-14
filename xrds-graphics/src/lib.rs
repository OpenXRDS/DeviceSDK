mod buffer;
mod error;
mod graphics;
mod material;
mod object;

pub use buffer::*;
pub use error::*;
pub use graphics::*;
pub use material::*;
pub use object::*;

pub struct Renderer {}

impl Renderer {
    pub fn load_scene(&mut self) -> anyhow::Result<Scene> {
        Ok(Scene {})
    }
}
