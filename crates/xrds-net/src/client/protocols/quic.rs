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

//! `QuicHandler`: raw QUIC as a [`StreamHandler`] (no HTTP/3 framing —
//! `quic://` is an SDK-specific convention, see `scheme.rs`).
//!
//! Extracted verbatim from `client.rs`'s `connect_quic`/`send_quic`/
//! `rcv_quic` + `event_loop`/`handle_read`/`handle_write`/
//! `send_initial_packet` (Phase 1 of `docs/done/xrds-net-protocol-handler.md`) —
//! `Client`'s old methods are untouched and still the ones actually called
//! until Phase 2 rewires `Client` onto this handler. Shares
//! `create_quic_config` with `Http3Handler` via `quic_shared`.
//!
//! One addition: `close()` didn't exist on the old `Client` (nothing ever
//! tore a QUIC connection down); this is new code, not an extraction.

use std::sync::{Arc, Mutex};

use mio::{Events, Poll};
use quiche::{Connection, RecvInfo};

use crate::client::categories::{SessionHandler, StreamHandler};
use crate::client::context::ClientContext;
use crate::client::error::NetError;
use crate::client::event::Event;
use crate::client::handler::ProtocolHandler;
use crate::common::generate_random_string;

use super::quic_shared::{create_quic_config, create_quic_config_insecure, MAX_DATAGRAM_SIZE};

fn net_err(e: impl std::fmt::Display) -> NetError {
    NetError::Network(e.to_string())
}

#[derive(Default)]
pub struct QuicHandler {
    connection: Option<Arc<Mutex<Connection>>>,
    socket: Option<Arc<Mutex<mio::net::UdpSocket>>>,
    poll: Option<Arc<Mutex<Poll>>>,
}

impl QuicHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// The QUIC handshake, shared by the `StreamHandler` and `SessionHandler`
    /// `connect` impls: bind a UDP socket, run the initial handshake to
    /// "established", and store the connection/socket/poll (all
    /// `Arc<Mutex<_>>` so send and the receive poll can share them).
    fn establish(&mut self, ctx: &ClientContext) -> Result<(), NetError> {
        let url = ctx
            .url
            .as_ref()
            .ok_or_else(|| NetError::Network("URL not set".to_string()))?;
        let peer_addr = url.socket_addrs().map_err(NetError::Network)?;
        let bind_addr = "0.0.0.0:0".to_string();

        let mut socket: mio::net::UdpSocket =
            mio::net::UdpSocket::bind(bind_addr.parse().unwrap()).unwrap();
        let local_addr = socket.local_addr().unwrap();

        let mut quic_config = if ctx.insecure {
            create_quic_config_insecure()
        } else {
            create_quic_config()
        };

        // scid MUST be 20 bytes long
        let scid = generate_random_string(20);
        let scid = quiche::ConnectionId::from_ref(scid.as_bytes());

        let mut poll = mio::Poll::new().unwrap();
        poll.registry()
            .register(&mut socket, mio::Token(0), mio::Interest::READABLE)
            .map_err(net_err)?;

        let host = ctx
            .host
            .clone()
            .ok_or_else(|| NetError::Network("host not set".to_string()))?;

        let mut conn = quiche::connect(Some(host.as_str()), &scid, local_addr, peer_addr, &mut quic_config)
            .map_err(net_err)?;

        // Start the QUIC connection
        Self::send_initial_packet(&mut socket, &mut conn).map_err(NetError::Network)?;

        // Condition of breaking the loop: Connection is closed or established
        Self::event_loop(&mut socket, &mut conn, &mut poll).map_err(NetError::Network)?;

        if conn.is_closed() {
            return Err(NetError::Network("Connection closed.".to_string()));
        }

        self.connection = Some(Arc::new(Mutex::new(conn)));
        self.socket = Some(Arc::new(Mutex::new(socket)));
        self.poll = Some(Arc::new(Mutex::new(poll)));

        Ok(())
    }

    fn send_initial_packet(socket: &mut mio::net::UdpSocket, conn: &mut Connection) -> Result<(), String> {
        let mut out = [0; MAX_DATAGRAM_SIZE];
        let (write, send_info) = conn.send(&mut out).expect("initial send failed");
        while let Err(e) = socket.send_to(&out[..write], send_info.to) {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                continue;
            }
            return Err(format!("send() failed: {:?}", e));
        }
        Ok(())
    }

    /* Used for initial handshake */
    fn event_loop(
        socket: &mut mio::net::UdpSocket,
        conn: &mut Connection,
        poll: &mut Poll,
    ) -> Result<(), String> {
        let mut events = Events::with_capacity(1024);
        let mut buf = [0; 65535];
        let mut out = [0; MAX_DATAGRAM_SIZE];

        loop {
            poll.poll(&mut events, conn.timeout())
                .map_err(|e| e.to_string())?;
            if conn.is_closed() {
                break;
            }

            if conn.is_established() {
                break;
            }

            Self::handle_read(socket, conn, &mut buf)?;
            Self::handle_write(socket, conn, &mut out)?;
        }

        Ok(())
    }

    fn handle_read(
        socket: &mut mio::net::UdpSocket,
        conn: &mut Connection,
        buf: &mut [u8],
    ) -> Result<(), String> {
        let (len, from) = match socket.recv_from(buf) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) => return Err(format!("recv() failed: {:?}", e)),
        };

        let recv_info = RecvInfo {
            to: socket.local_addr().unwrap(),
            from,
        };

        conn.recv(&mut buf[..len], recv_info)
            .map_err(|e| format!("recv failed: {:?}", e))?;
        Ok(())
    }

    fn handle_write(
        socket: &mut mio::net::UdpSocket,
        conn: &mut Connection,
        out: &mut [u8],
    ) -> Result<(), String> {
        loop {
            let (write, send_info) = match conn.send(out) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => {
                    conn.close(false, 0x1, b"fail").ok();
                    return Err(format!("send failed: {:?}", e));
                }
            };

            if let Err(e) = socket.send_to(&out[..write], send_info.to) {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    break;
                }
                return Err(format!("send() failed: {:?}", e));
            }
        }

        Ok(())
    }
}

impl StreamHandler for QuicHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError> {
        self.establish(ctx)
    }

    fn send(&mut self, _ctx: &ClientContext, _topic: Option<&str>, mut data: Vec<u8>) -> Result<(), NetError> {
        let conn = self
            .connection
            .as_mut()
            .ok_or_else(|| NetError::Network("QUIC connection is not initialized".to_string()))?;

        Self::handle_write(
            &mut self.socket.as_mut().unwrap().lock().unwrap(),
            &mut conn.lock().unwrap(),
            &mut data,
        )
        .map_err(NetError::Network)
    }

    fn recv(&mut self, _ctx: &ClientContext) -> Result<Event, NetError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| NetError::Network("QUIC connection is not initialized".to_string()))?;
        let mut buf = [0; 65535];
        let mut socket = self.socket.as_ref().unwrap().lock().unwrap();
        let mut conn = conn.lock().unwrap();

        Self::handle_read(&mut socket, &mut conn, &mut buf).map_err(NetError::Network)?;

        Ok(Event::new(None, buf.to_vec()))
    }

    fn close(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
        if let Some(conn) = &self.connection {
            conn.lock().unwrap().close(true, 0x0, b"bye").ok();
        }
        Ok(())
    }
}

/// The single client-initiated bidirectional stream a QUIC `NetChannel` uses.
/// (Client bidi stream ids are 0, 4, 8, …; 0 is the first.)
const SESSION_STREAM_ID: u64 = 0;

impl SessionHandler for QuicHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError> {
        self.establish(ctx)
    }

    fn send(&mut self, _ctx: &ClientContext, data: Vec<u8>) -> Result<(), NetError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| NetError::Network("QUIC connection is not initialized".to_string()))?;
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| NetError::Network("QUIC socket is not initialized".to_string()))?;
        let mut conn = conn.lock().unwrap();
        let mut socket = socket.lock().unwrap();

        // Application bytes go on the session stream (fin = false keeps the
        // channel open for further sends) — this is the real QUIC stream API,
        // unlike the `StreamHandler` path which only pumps datagrams.
        conn.stream_send(SESSION_STREAM_ID, &data, false)
            .map_err(net_err)?;

        // Flush the resulting packets out.
        let mut out = [0; MAX_DATAGRAM_SIZE];
        Self::handle_write(&mut socket, &mut conn, &mut out).map_err(NetError::Network)?;
        Ok(())
    }

    fn poll_recv(&mut self, _ctx: &ClientContext) -> Result<Option<Event>, NetError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| NetError::Network("QUIC connection is not initialized".to_string()))?;
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| NetError::Network("QUIC socket is not initialized".to_string()))?;
        let mut conn = conn.lock().unwrap();
        let mut socket = socket.lock().unwrap();

        // 1. Drain every UDP datagram currently available (non-blocking) into
        //    the connection.
        let mut dbuf = [0; 65535];
        loop {
            match socket.recv_from(&mut dbuf) {
                Ok((len, from)) => {
                    let recv_info = RecvInfo {
                        to: socket.local_addr().unwrap(),
                        from,
                    };
                    conn.recv(&mut dbuf[..len], recv_info).map_err(net_err)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(NetError::Network(format!("recv() failed: {e:?}"))),
            }
        }

        // 2. Flush anything the incoming data prompted (ACKs, etc.).
        let mut out = [0; MAX_DATAGRAM_SIZE];
        let _ = Self::handle_write(&mut socket, &mut conn, &mut out);

        // 3. Return the first non-empty chunk of application stream data;
        //    `None` when nothing is available this poll.
        let readable: Vec<u64> = conn.readable().collect();
        for stream_id in readable {
            let mut sbuf = [0; 65535];
            match conn.stream_recv(stream_id, &mut sbuf) {
                Ok((len, _fin)) if len > 0 => {
                    return Ok(Some(Event::new(None, sbuf[..len].to_vec())));
                }
                Ok(_) => {}                    // 0 bytes / fin-only
                Err(quiche::Error::Done) => {} // nothing on this stream right now
                Err(e) => return Err(NetError::Network(format!("stream_recv failed: {e:?}"))),
            }
        }
        Ok(None)
    }

    fn close(&mut self, _ctx: &ClientContext) -> Result<(), NetError> {
        if let Some(conn) = &self.connection {
            conn.lock().unwrap().close(true, 0x0, b"bye").ok();
        }
        Ok(())
    }
}

impl ProtocolHandler for QuicHandler {
    fn as_stream(&mut self) -> Option<&mut dyn StreamHandler> {
        Some(self)
    }

    fn as_session(&mut self) -> Option<&mut dyn SessionHandler> {
        Some(self)
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
    use crate::common::enums::PROTOCOLS;

    fn ctx_for(url: &str) -> ClientContext {
        let mut ctx = ClientContext::new(PROTOCOLS::QUIC, "test-id".to_string());
        ctx.raw_url = url.to_string();
        ctx.parse_url_into_self().expect("should parse");
        ctx
    }

    #[test]
    fn connect_without_a_reachable_peer_is_a_network_error_not_a_panic() {
        let mut handler = QuicHandler::new();
        let ctx = ctx_for("quic://127.0.0.1:1");

        // `connect` is now on two traits (Stream + Session); go through the
        // shared handshake directly.
        let err = handler.establish(&ctx).expect_err("bad peer should fail");
        assert!(matches!(err, NetError::Network(_)));
    }

    #[test]
    fn exposes_itself_as_a_stream_handler() {
        let mut handler = QuicHandler::new();
        assert!(handler.as_stream().is_some());
    }

    #[test]
    fn exposes_itself_as_a_session_handler() {
        // QUIC is the one protocol backing `XrdsNet::open` today.
        let mut handler = QuicHandler::new();
        assert!(handler.as_session().is_some());
    }

    #[test]
    fn quic_session_round_trips_against_the_test_server() {
        use crate::client::net_channel::NetChannel;
        use crate::server::quic_server::QuicServer;
        use std::time::Duration;

        // Spin up the local raw-QUIC echo server.
        let server = QuicServer::start().expect("start quic echo server");

        // Build an insecure QUIC session (the server uses a self-signed cert)
        // and drive it through the real `QuicHandler` + `NetChannel` path.
        let mut ctx = ClientContext::new(PROTOCOLS::QUIC, "test-id".to_string());
        ctx.raw_url = server.url("/");
        ctx.parse_url_into_self().expect("parse url");
        ctx.insecure = true;

        let mut handler = QuicHandler::new();
        SessionHandler::connect(&mut handler, &ctx).expect("quic session connect");

        let mut chan = NetChannel::new(ctx, Box::new(handler));
        chan.send(b"hello quic".to_vec()).expect("send on the session");

        let echoed = chan
            .recv_timeout(Duration::from_secs(5))
            .expect("should receive the echo back");
        assert_eq!(echoed.payload, b"hello quic".to_vec());

        let _ = chan.close();
    }
}
