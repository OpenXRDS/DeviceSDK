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

//! Shared QUIC transport config used by both `Http3Handler` and
//! `QuicHandler` — extracted verbatim from `client.rs`'s
//! `create_quic_config`/`MAX_DATAGRAM_SIZE` (Phase 1 of
//! `docs/done/xrds-net-protocol-handler.md`).

pub(crate) const MAX_DATAGRAM_SIZE: usize = 1350;

/// Client QUIC config with peer verification **on** (the normal path).
pub(crate) fn create_quic_config() -> quiche::Config {
    build_quic_config(true)
}

/// Client QUIC config with peer verification **off** — for self-signed / dev
/// servers (`ClientContext::insecure`), including the crate's own QUIC test
/// server. Never the default.
pub(crate) fn create_quic_config_insecure() -> quiche::Config {
    build_quic_config(false)
}

fn build_quic_config(verify_peer: bool) -> quiche::Config {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();

    config
        .set_application_protos(quiche::h3::APPLICATION_PROTOCOL)
        .unwrap();
    config.set_max_idle_timeout(30_000);
    config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_initial_max_data(10_000_000);
    config.set_initial_max_stream_data_bidi_local(1_000_000);
    config.set_initial_max_stream_data_bidi_remote(1_000_000);
    config.set_initial_max_stream_data_uni(1_000_000);
    config.set_initial_max_streams_bidi(100);
    config.set_initial_max_streams_uni(100);
    config.verify_peer(verify_peer);

    config
}
