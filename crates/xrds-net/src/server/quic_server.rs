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

//! A minimal single-connection raw-QUIC **echo** server, for the crate's own
//! tests only (hence `#[cfg(test)]` in `server/mod.rs` and the `rcgen`
//! dev-dependency). Whatever the client sends on a QUIC stream is written
//! straight back on the same stream. It exists to give the QUIC
//! `NetChannel`/`SessionHandler` a live round-trip target — there is no public
//! raw-QUIC echo endpoint (public QUIC is HTTP/3).
//!
//! Deliberately not production-grade: it uses a self-signed cert (so the
//! client connects with `ClientContext::insecure`), handles exactly one
//! connection at a time, and skips stateless retry / version negotiation
//! (fine for a trusted localhost test where client and server share the
//! protocol version).

use std::io::Write;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: usize = 1350;

/// Handle to a running QUIC echo server. Drop it to shut the server down.
pub(crate) struct QuicServer {
    port: u16,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl QuicServer {
    /// Bind an ephemeral UDP port on `127.0.0.1`, spawn the echo loop on a
    /// background thread, and return a handle.
    pub(crate) fn start() -> std::io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        socket.set_nonblocking(true)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();
        let worker = std::thread::spawn(move || run_echo(socket, stop));

        Ok(Self {
            port,
            shutdown,
            worker: Some(worker),
        })
    }

    /// A `quic://127.0.0.1:<port><path>` URL pointing at this server.
    pub(crate) fn url(&self, path: &str) -> String {
        format!("quic://127.0.0.1:{}{}", self.port, path)
    }
}

impl Drop for QuicServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn build_server_config() -> quiche::Config {
    // Self-signed cert written to temp PEM files (quiche's config only loads
    // from files).
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .expect("generate self-signed cert");
    let cert_pem = cert.cert.pem();
    let key_pem = cert.key_pair.serialize_pem();

    let dir = std::env::temp_dir();
    let tag = crate::common::generate_uuid();
    let cert_path = dir.join(format!("xrdsnet_quic_{tag}.crt"));
    let key_path = dir.join(format!("xrdsnet_quic_{tag}.key"));
    std::fs::File::create(&cert_path)
        .unwrap()
        .write_all(cert_pem.as_bytes())
        .unwrap();
    std::fs::File::create(&key_path)
        .unwrap()
        .write_all(key_pem.as_bytes())
        .unwrap();

    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
    config
        .load_cert_chain_from_pem_file(cert_path.to_str().unwrap())
        .unwrap();
    config
        .load_priv_key_from_pem_file(key_path.to_str().unwrap())
        .unwrap();
    // Same ALPN + transport params as the client (`quic_shared`).
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
    config.verify_peer(false); // servers don't verify the client here

    config
}

fn run_echo(socket: UdpSocket, stop: Arc<AtomicBool>) {
    let mut config = build_server_config();
    let local_addr = socket.local_addr().unwrap();

    let mut conn: Option<quiche::Connection> = None;
    let mut buf = [0u8; 65535];
    let mut out = [0u8; MAX_DATAGRAM_SIZE];
    let mut last_activity = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        // 1. Drain every datagram currently available (non-blocking).
        let mut got_packet = false;
        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, from)) => {
                    got_packet = true;
                    last_activity = Instant::now();

                    if conn.is_none() {
                        // Only start a connection on an Initial packet. Extract
                        // the type in a scope so the header's borrow of `buf`
                        // is released before we hand `buf` to `recv`.
                        let is_initial = {
                            match quiche::Header::from_slice(
                                &mut buf[..len],
                                quiche::MAX_CONN_ID_LEN,
                            ) {
                                Ok(hdr) => hdr.ty == quiche::Type::Initial,
                                Err(_) => false,
                            }
                        };
                        if !is_initial {
                            continue;
                        }
                        let scid_bytes = [0u8; quiche::MAX_CONN_ID_LEN];
                        let scid = quiche::ConnectionId::from_ref(&scid_bytes);
                        match quiche::accept(&scid, None, local_addr, from, &mut config) {
                            Ok(c) => conn = Some(c),
                            Err(_) => continue,
                        }
                    }

                    if let Some(c) = conn.as_mut() {
                        let recv_info = quiche::RecvInfo {
                            to: local_addr,
                            from,
                        };
                        let _ = c.recv(&mut buf[..len], recv_info);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }

        // 2. Drive loss recovery / handshake timers.
        if let Some(c) = conn.as_mut() {
            if let Some(timeout) = c.timeout() {
                if last_activity.elapsed() >= timeout {
                    c.on_timeout();
                }
            }
        }

        // 3. Echo: read each readable stream and write it straight back.
        if let Some(c) = conn.as_mut() {
            if c.is_established() {
                let readable: Vec<u64> = c.readable().collect();
                for sid in readable {
                    let mut sbuf = [0u8; 65535];
                    if let Ok((n, _fin)) = c.stream_recv(sid, &mut sbuf) {
                        if n > 0 {
                            let _ = c.stream_send(sid, &sbuf[..n], false);
                        }
                    }
                }
            }
        }

        // 4. Flush outgoing packets.
        if let Some(c) = conn.as_mut() {
            loop {
                match c.send(&mut out) {
                    Ok((write, send_info)) => {
                        let _ = socket.send_to(&out[..write], send_info.to);
                    }
                    Err(quiche::Error::Done) => break,
                    Err(_) => break,
                }
            }
            if c.is_closed() {
                conn = None;
            }
        }

        if !got_packet {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
