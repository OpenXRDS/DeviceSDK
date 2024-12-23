use crate::runtime::Runtime;

#[derive(Default)]
pub struct RuntimeBuilder {
    application_name: String,
}

pub fn new() -> RuntimeBuilder {
    RuntimeBuilder {
        ..Default::default()
    }
}

impl RuntimeBuilder {
    pub fn set_application_name(&mut self, application_name: &str) -> &mut Self {
        self.application_name = application_name.to_owned();
        self
    }

    pub fn build(self) -> Runtime {
        Runtime {}
    }
}
