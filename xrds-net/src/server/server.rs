use crate::common::enums::PROTOCOLS;

pub struct XRNetServer {
    pub protocol: PROTOCOLS,
    pub port: u32,

    // Optional fields
    pub greeting: Option<String>,
}

impl XRNetServer {
    pub fn new(protocol: PROTOCOLS, port: u32) -> XRNetServer {
        XRNetServer {
            protocol,
            port,
            greeting: None,
        }
    }

    pub fn set_greeting(&mut self, greeting: String) {
        self.greeting = Some(greeting);
    }

    pub async fn start(&self) {
        println!("Server started on port {}", self.port);
    }
}

