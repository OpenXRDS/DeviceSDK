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

//! One `ProtocolHandler` implementation per protocol. See
//! `docs/done/xrds-net-protocol-handler.md`'s "Method -> handler mapping" for the
//! full extraction plan; modules land here one at a time (Phase 1).

#[cfg(feature = "protocol-coap")]
pub mod coap;
#[cfg(feature = "protocol-ftp")]
pub mod ftp;
#[cfg(feature = "protocol-http")]
pub mod http;
// HTTP/3 and QUIC are both quiche, so they share one feature — see Cargo.toml.
#[cfg(feature = "protocol-quic")]
pub mod http3;
#[cfg(feature = "protocol-mqtt")]
pub mod mqtt;
#[cfg(feature = "protocol-quic")]
pub mod quic;
#[cfg(feature = "protocol-quic")]
mod quic_shared;
#[cfg(feature = "protocol-ws")]
pub mod ws;
