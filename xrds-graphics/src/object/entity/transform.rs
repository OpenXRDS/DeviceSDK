use uuid::Uuid;
use xrds_core::Transform;

#[derive(Debug, Default, Clone)]
pub struct TransformComponent {
    pub local_transform: Transform,
    pub global_transform: Transform,
    pub parent: Option<Uuid>,
    pub childs: Vec<Uuid>,
}

impl TransformComponent {
    pub fn with_local_transform(mut self, transform: Transform) -> Self {
        self.local_transform = transform;
        self
    }

    pub fn with_global_transform(mut self, transform: Transform) -> Self {
        self.global_transform = transform;
        self
    }

    pub fn with_parent(mut self, parent: &Uuid) -> Self {
        self.parent = Some(parent.clone());
        self
    }

    pub fn with_childs(mut self, childs: &[Uuid]) -> Self {
        self.childs = childs.to_vec();
        self
    }

    pub fn set_parent(&mut self, parent: &Uuid) {
        self.parent = Some(parent.clone());
    }

    pub fn set_childs(&mut self, childs: &[Uuid]) {
        self.childs = childs.to_vec();
    }

    pub fn add_child(&mut self, child: &Uuid) {
        self.childs.push(child.clone());
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}
