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

//! `Http3Handler`: HTTP/3 over QUIC, request-shaped only.
//!
//! Extracted verbatim from `client.rs`'s `request_http3` + its
//! `send_packet`/`receive_packets`/`send_http3_request`/`handle_http3_events`
//! helpers (Phase 1 of `docs/xrds-net-protocol-handler.md`) — `Client`'s old
//! methods are untouched and still the ones actually called until Phase 2
//! rewires `Client` onto this handler. Shares `create_quic_config` with
//! `QuicHandler` via `quic_shared`.
//!
//! `HTTP3` has no scheme of its own (see `scheme.rs`); it's reached only via
//! the expert `set_protocol` override until real ALPN-based negotiation
//! exists.

use std::time::{Duration, Instant};

use crate::client::context::ClientContext;
use crate::client::error::NetError;
use crate::client::handler::ProtocolHandler;
use crate::common::data_structure::NetResponse;
use crate::common::enums::PROTOCOLS;
use crate::common::{fill_mandatory_http_headers, generate_random_string};
use quiche::h3::NameValue;

use super::quic_shared::{create_quic_config, MAX_DATAGRAM_SIZE};

#[derive(Default)]
pub struct Http3Handler;

impl Http3Handler {
    pub fn new() -> Self {
        Self
    }

    /// Mandatory headers for HTTP3 MUST be included in the request
    ///  - RFC 9114 Section 4.3.1
    ///  - https://datatracker.ietf.org/doc/html/rfc9114#section-4.3.1
    fn request_http3(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        let start_time = Instant::now();
        let max_duration = Duration::from_secs(30);

        let mut response = NetResponse {
            protocol: PROTOCOLS::HTTP3,
            status_code: 0,
            headers: vec![],
            body: Vec::new(),
            error: None,
        };

        // meta data preparation
        let url = ctx
            .url
            .clone()
            .ok_or_else(|| NetError::Network("URL not parsed".to_string()))?;
        let peer_addr = url.socket_addrs().map_err(NetError::Network)?;
        let bind_addr = match peer_addr {
            std::net::SocketAddr::V4(_) => "0.0.0.0:0",
            std::net::SocketAddr::V6(_) => "[::]:0",
        };

        let req_headers = match ctx.req_headers.clone() {
            Some(headers) => {
                fill_mandatory_http_headers(url.clone(), Some(headers), ctx.method.clone())
            }
            None => fill_mandatory_http_headers(url.clone(), None, ctx.method.clone()),
        };

        let mut socket = mio::net::UdpSocket::bind(bind_addr.parse().unwrap()).unwrap();
        let mut poll = mio::Poll::new().unwrap();
        poll.registry()
            .register(&mut socket, mio::Token(0), mio::Interest::READABLE)
            .unwrap();

        let scid = generate_random_string(20);
        let scid = quiche::ConnectionId::from_ref(scid.as_bytes());

        let mut quic_config = create_quic_config();

        let local_addr = socket.local_addr().unwrap();
        let mut conn = quiche::connect(
            Some(url.host.as_str()),
            &scid,
            local_addr,
            peer_addr,
            &mut quic_config,
        )
        .unwrap();

        // QUIC Initialization
        let mut out = [0; MAX_DATAGRAM_SIZE];
        Self::send_packet(&mut socket, &mut conn, &mut out).expect("Initial send failed");

        let h3_config = quiche::h3::Config::new().unwrap();
        let mut http3_conn: Option<quiche::h3::Connection> = None;
        let mut req_sent = false;
        let mut events = mio::Events::with_capacity(1024);
        let mut is_exit = false;

        let handshake_timeout = Duration::from_secs(20); // New: QUIC handshake timeout
        let connection_timeout = Duration::from_secs(25); // HTTP/3 connection timeout
        let response_timeout = Duration::from_secs(30); // Response timeout (increased)

        let handshake_start = Instant::now();
        let connection_start = Instant::now();
        let mut handshake_completed = false;
        let mut connection_established = false;
        let mut request_sent_time: Option<Instant> = None;
        let mut buf = [0; 65535];

        loop {
            // Overall timeout check
            if start_time.elapsed() > max_duration {
                response.error = Some(format!(
                    "Request timed out after {} seconds",
                    max_duration.as_secs()
                ));
                break;
            }

            if is_exit {
                break;
            }

            // Handshake timeout check
            if !handshake_completed && handshake_start.elapsed() > handshake_timeout {
                response.error =
                    Some("QUIC handshake timeout - check server availability".to_string());
                break;
            }

            // Connection timeout check
            if handshake_completed
                && !connection_established
                && connection_start.elapsed() > connection_timeout
            {
                response.error = Some("HTTP/3 connection establishment timeout".to_string());
                break;
            }

            // Adaptive polling based on connection state
            let poll_timeout = if !handshake_completed {
                Some(Duration::from_millis(100)) // More frequent during handshake
            } else if !connection_established || request_sent_time.is_none() {
                Some(Duration::from_millis(50)) // Frequent during HTTP/3 setup
            } else {
                // After request sent, use longer timeouts to avoid missing data
                Some(Duration::from_millis(500)) // Less frequent during response wait
            };

            if poll.poll(&mut events, poll_timeout).is_err() {
                response.error = Some("Polling error".to_string());
                break;
            }

            // Check QUIC handshake completion
            if !handshake_completed && conn.is_established() {
                handshake_completed = true;
                println!(
                    "QUIC handshake completed in {:?}",
                    handshake_start.elapsed()
                );
            }

            // Handle packet reception with better error handling
            if let Err(e) = Self::receive_packets(
                &mut socket,
                &mut conn,
                &mut buf,
                local_addr,
                &mut http3_conn,
                &h3_config,
            ) {
                // Don't fail on recoverable errors
                if !e.contains("would block") && !e.contains("Done") {
                    response.error = Some(e);
                    break;
                }
            }

            if let Some(h3) = http3_conn.as_mut() {
                if !connection_established {
                    connection_established = true;
                    println!(
                        "HTTP/3 connection established in {:?}",
                        connection_start.elapsed()
                    );
                }

                if !req_sent {
                    match Self::send_http3_request(h3, &mut conn, req_headers.as_slice()) {
                        Ok(_) => {
                            req_sent = true;
                            request_sent_time = Some(Instant::now());
                            println!("HTTP/3 request sent successfully");
                        }
                        Err(e) => {
                            // Retry on certain errors
                            if e.contains("stream limit") || e.contains("would block") {
                                continue; // Retry on next iteration
                            } else {
                                response.error = Some(e);
                                break;
                            }
                        }
                    }
                }

                // Response timeout check (only after request sent)
                if let Some(sent_time) = request_sent_time {
                    if sent_time.elapsed() > response_timeout {
                        response.error = Some(format!(
                            "Response timeout after {} seconds",
                            response_timeout.as_secs()
                        ));
                        break;
                    }
                }

                is_exit = match Self::handle_http3_events(h3, &mut conn, &mut buf, &mut response) {
                    Ok(exit) => exit,
                    Err(err) => {
                        response.error = Some(err);
                        true
                    }
                };
            }

            // Send packets with better error handling
            if let Err(e) = Self::send_packet(&mut socket, &mut conn, &mut out) {
                if !e.contains("Done") && !e.contains("would block") {
                    response.error = Some(e);
                    break;
                }
            }

            if conn.is_closed() {
                if response.status_code == 0 && response.body.is_empty() {
                    response.error =
                        Some("Connection closed without receiving response".to_string());
                } else {
                    println!("Connection closed after receiving partial response");
                }
                break;
            }

            if conn.is_draining() {
                println!("Connection is draining");
                if response.status_code > 0 {
                    println!("Received response before connection drain");
                    break;
                }
            }
        }

        Ok(response)
    }

    fn send_packet(
        socket: &mut mio::net::UdpSocket,
        conn: &mut quiche::Connection,
        out: &mut [u8],
    ) -> Result<(), String> {
        while let Ok((write, send_info)) = conn.send(out) {
            socket
                .send_to(&out[..write], send_info.to)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn receive_packets(
        socket: &mut mio::net::UdpSocket,
        conn: &mut quiche::Connection,
        buf: &mut [u8],
        local_addr: std::net::SocketAddr,
        http3_conn: &mut Option<quiche::h3::Connection>,
        h3_config: &quiche::h3::Config,
    ) -> Result<(), String> {
        while let Ok((len, from)) = socket.recv_from(buf) {
            let recv_info = quiche::RecvInfo {
                to: local_addr,
                from,
            };
            if let Err(e) = conn.recv(&mut buf[..len], recv_info) {
                return Err(format!("QUIC recv failed: {:?}", e));
            }
            if conn.is_established() && http3_conn.is_none() {
                *http3_conn = Some(
                    quiche::h3::Connection::with_transport(conn, h3_config)
                        .map_err(|e| format!("HTTP3 connection failed: {:?}", e))?,
                );
            }
        }
        Ok(())
    }

    fn send_http3_request(
        h3: &mut quiche::h3::Connection,
        conn: &mut quiche::Connection,
        req: &[quiche::h3::Header],
    ) -> Result<(), String> {
        let result = h3.send_request(conn, req, true).map_err(|e| e.to_string());
        if result.is_err() {
            Err(result.err().unwrap())
        } else {
            Ok(())
        }
    }

    fn handle_http3_events(
        h3: &mut quiche::h3::Connection,
        conn: &mut quiche::Connection,
        buf: &mut [u8],
        response: &mut NetResponse,
    ) -> Result<bool, String> {
        let mut events_processed = 0;
        let max_events_per_iteration = 100;

        while events_processed < max_events_per_iteration {
            match h3.poll(conn) {
                Ok((stream_id, event)) => {
                    events_processed += 1;
                    println!("HTTP/3 event on stream {}: {:?}", stream_id, event);

                    match event {
                        quiche::h3::Event::Headers { list, more_frames } => {
                            println!(
                                "Received headers (count: {}, more_frames: {})",
                                list.len(),
                                more_frames
                            );
                            for header in list {
                                let name = String::from_utf8_lossy(header.name());
                                let value = String::from_utf8_lossy(header.value());
                                println!("  {}: {}", name, value);

                                if name == ":status" {
                                    if let Ok(status_code) = value.parse::<u16>() {
                                        response.status_code = status_code as u32;
                                        println!("Status code set to: {}", status_code);
                                    }
                                }

                                response.headers.push((name.to_string(), value.to_string()));
                            }

                            // If there's no body and no more frames, we might be done
                            if !more_frames && response.status_code > 0 {
                                println!("Response complete (headers only)");
                                return Ok(true);
                            }
                        }
                        quiche::h3::Event::Data => {
                            let mut total_read = 0;
                            loop {
                                match h3.recv_body(conn, stream_id, buf) {
                                    Ok(read) => {
                                        if read > 0 {
                                            response.body.extend_from_slice(&buf[..read]);
                                            total_read += read;
                                            println!(
                                                "Received {} bytes of data (total this event: {})",
                                                read, total_read
                                            );
                                        } else {
                                            break;
                                        }
                                    }
                                    Err(quiche::h3::Error::Done) => break,
                                    Err(e) => {
                                        println!("Error reading body: {:?}", e);
                                        break;
                                    }
                                }
                            }

                            if total_read > 0 {
                                println!("Total response body size: {} bytes", response.body.len());
                            }
                        }
                        quiche::h3::Event::Finished => {
                            println!(
                                "Stream {} finished. Final response - Status: {}, Body size: {}",
                                stream_id,
                                response.status_code,
                                response.body.len()
                            );
                            return Ok(true);
                        }
                        quiche::h3::Event::Reset(error_code) => {
                            println!("Stream {} reset with error: {}", stream_id, error_code);
                            return Err(format!("Stream reset with error: {}", error_code));
                        }
                        quiche::h3::Event::GoAway => {
                            println!("Received GoAway");
                            if response.status_code > 0 {
                                return Ok(true);
                            }
                            return Err("Server sent GoAway".to_string());
                        }
                        _ => {
                            println!("Other HTTP/3 event: {:?}", event);
                        }
                    }
                }
                Err(quiche::h3::Error::Done) => {
                    break; // No more events
                }
                Err(e) => {
                    println!("HTTP/3 poll error: {:?}", e);
                    return Err(format!("HTTP/3 poll failed: {:?}", e));
                }
            }
        }

        Ok(false)
    }
}

impl ProtocolHandler for Http3Handler {
    fn request(&self, ctx: &ClientContext) -> Result<NetResponse, NetError> {
        self.request_http3(ctx)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real HTTP/3 round-trips against a live server are covered by
    // `client::tests` (with the retry/spacing helpers a flaky QUIC endpoint
    // needs) and by Phase 4's verification pass — this just proves the
    // handler is reachable through the trait object without needing
    // network access.
    #[test]
    fn request_without_a_parsed_url_is_a_network_error_not_a_panic() {
        let handler = Http3Handler::new();
        let ctx = ClientContext::new(PROTOCOLS::HTTP3, "test-id".to_string());

        let err = handler.request(&ctx).expect_err("unparsed url should fail");
        assert!(matches!(err, NetError::Network(_)));
    }
}
