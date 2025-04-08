use crate::XrdsMesh;

#[derive(Debug, Clone)]
pub struct MeshComponent {
    pub mesh: XrdsMesh,
}

impl MeshComponent {
    pub fn new(mesh: XrdsMesh) -> Self {
        Self { mesh }
    }

    pub fn mesh(&self) -> &XrdsMesh {
        &self.mesh
    }

    pub fn mesh_mut(&mut self) -> &mut XrdsMesh {
        &mut self.mesh
    }
}
