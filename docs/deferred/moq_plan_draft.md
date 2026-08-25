```markdown
# Implementation Plan: `moq-sdk` (MoQ Abstraction Layer)

## 1. Project Overview
**Goal:** Create a high-level, ergonomic Rust crate that abstracts the modular complexity of the moq-dev/moq ecosystem. 
**Target Audience:** Developers who want to integrate Media over QUIC (MoQ) pub/sub, live media streaming, or embed a MoQ relay into their application without needing to manually wire QUIC endpoints, multiplexers, and media catalogs.

### Core Dependencies
*   `moq-net`: Core MoQ transport logic and state machines.
*   `moq-native`: QUIC configuration (Quinn wrapper).
*   `moq-relay`: Relay server routing logic.
*   `hang` & `moq-mux`: Media packaging and cataloging.
*   `moq-token`: Authentication.

---

## 2. Phase 1: Foundation and Error Handling
Unify the disparate error types from the underlying crates into a single, developer-friendly interface.

### Tasks:
- [ ] Initialize library crate (`cargo new --lib moq-sdk`).
- [ ] Set up `Cargo.toml` with feature flags to prevent compiling heavy media logic for users who only want generic pub/sub (`features = ["media", "relay", "auth"]`).
- [ ] Create `src/error.rs` using `thiserror`.

### Example API:
```rust
#[derive(thiserror::Error, Debug)]
pub enum MoqError {
    #[error("Network error: {0}")]
    Network(#[from] moq_net::Error),
    #[error("QUIC connection failed: {0}")]
    Connection(#[from] quinn::ConnectError),
    #[error("Media encoding/decoding error: {0}")]
    Media(#[from] hang::Error),
    #[error("Authentication failed: {0}")]
    Auth(String),
}
```

---

## 2. Phase 2: Client Session Management (The Core)

Wrap `moq-native` and `moq-net` to establish a QUIC connection and manage the background Tokio tasks required to keep the session alive.

### Tasks:

* [ ] Implement `ClientBuilder` in `src/client/builder.rs` to handle URL parsing, TLS config, and optional authentication.
* [ ] Implement `Session` in `src/client/session.rs` to wrap `moq_net::Session`.
* [ ] Implement a background task manager that polls the underlying `moq_net::Session` so the user doesn't have to manually drive the future.

### Example API:

```rust
pub struct ClientBuilder {
    url: String,
    auth_token: Option<String>,
    cert_paths: Vec<PathBuf>,
}

impl ClientBuilder {
    pub async fn connect(self) -> Result<MoqClient, MoqError> {
        // 1. Setup Quinn endpoint via moq-native
        // 2. Connect to relay
        // 3. Negotiate moq-net session
        // 4. Spawn background task to drive session
        unimplemented!()
    }
}
```

---

## 3. Phase 3: Client Publisher & Subscriber (Generic Data)

Expose the ability to publish and subscribe to generic byte streams.

### Tasks:

* [ ] Implement `Publisher` in `src/client/publish.rs` wrapping `moq_net::Publisher`. Provide simple methods for creating tracks and pushing data frames.
* [ ] Implement `Subscriber` in `src/client/subscribe.rs` wrapping `moq_net::Subscriber`. Provide a `Stream` (Async Iterator) interface to consume incoming frames.

### Example API:

```rust
impl MoqClient {
    /// Announces a namespace and prepares to publish tracks
    pub async fn publisher(&self, namespace: &str) -> Result<Publisher, MoqError> {
        unimplemented!()
    }
  
    /// Subscribes to a specific track within a namespace
    pub async fn subscribe(&self, namespace: &str, track: &str) -> Result<Subscriber, MoqError> {
        unimplemented!()
    }
}
```

---

## 4. Phase 4: Relay Server Abstraction

Allow users to easily embed a MoQ relay inside their own Rust backend (e.g., inside an Axum or Tonic application).

### Tasks:

* [ ] Create `src/relay/server.rs`.
* [ ] Implement `RelayBuilder` to configure binding addresses, TLS certificates, and routing hooks (allowing the host app to accept/reject connections).
* [ ] Provide a `RelayHandle` to gracefully shut down the embedded server.

### Example API:

```rust
pub struct RelayBuilder {
    bind_addr: std::net::SocketAddr,
    // tls_config: TlsConfig,
}

impl RelayBuilder {
    pub async fn serve(self) -> Result<RelayHandle, MoqError> {
        // 1. Initialize moq-native server endpoint
        // 2. Initialize moq-relay node
        // 3. Spawn Tokio accept loop
        unimplemented!()
    }
}

pub struct RelayHandle { /* ... */ }
```

---

## 5. Phase 5: Media Pipeline Abstraction

Wrap `hang` and `moq-mux` to hide the concept of "catalogs" and "groups" behind simple concepts like "Video Source" and "Audio Source".

### Tasks:

* [ ] Create `src/media/mod.rs` (gated behind `#[cfg(feature = "media")]`).
* [ ] Implement `MediaSource` to read standard formats (e.g., fMP4) and automatically chunk them into `hang` frames.
* [ ] Implement `MediaSink` to reconstruct incoming MoQ frames back into usable byte streams for players.

### Example API:

```rust
pub struct MediaSource {
    // Wraps hang and moq-mux state
}

impl Publisher {
    /// Automatically manages hang catalogs and group increments
    pub async fn publish_media(&self, track_name: &str, source: MediaSource) -> Result<(), MoqError> {
        unimplemented!()
    }
}

```

---

## 6. Phase 6: Testing & Ergonomics

Build end-to-end local tests to prove it works without external dependencies.

### Tasks:

* [ ] Create `tests/loopback.rs`.
* [ ] Write a test that spins up the embedded `RelayServer` on `localhost`.
* [ ] Connect a `ClientBuilder` publisher and a `ClientBuilder` subscriber to the local relay.
* [ ] Send 100 frames and verify they are received.
* [ ] Populate the `examples/` directory (`chat_cli.rs`, `video_streamer.rs`, `embedded_relay.rs`).

```

```
