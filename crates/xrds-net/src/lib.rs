/*
Copyright 2025 KETI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

pub mod client;
pub mod common;
#[cfg(feature = "bevy_plugin")]
pub mod plugin_net_bevy;
pub mod server;

#[cfg(feature = "bevy_plugin")]
pub use plugin_net_bevy::{
    process_net_commands, NetClientState, NetCommand, NetOutput, NetPlugin, NetPluginConfig,
};

pub use client::{webrtc_client::WebRTCClient, ClientBuilder};

pub use common::enums::{FtpCommands, PROTOCOLS};

#[cfg(test)]
mod tests {
    use crate::client::webrtc_client::WebRTCClient;

    #[test]
    fn test_library_exports() {
        let _client = WebRTCClient::new();
    }
}
