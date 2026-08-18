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
pub mod server;

#[cfg(feature = "protocol-webrtc")]
mod webrtc_ice_config;

pub use client::{
    ClientBuilder, Event, EventStream, ListenOptions, NetChannel, NetError, NetFeed, NetTaskSlot,
    Overflow, RequestOptions, TransferOp, TransferResult, XrdsNet, XrdsNetTask,
};

#[cfg(feature = "protocol-webrtc")]
pub use client::{
    media::{AudioSource, VideoSource},
    webrtc_client::WebRTCClient,
};

pub use common::data_structure::NetResponse;
pub use common::enums::{FtpCommands, PROTOCOLS};

#[cfg(all(test, feature = "protocol-webrtc"))]
mod tests {
    use crate::client::webrtc_client::WebRTCClient;

    #[test]
    fn test_library_exports() {
        let _client = WebRTCClient::new();
    }
}
