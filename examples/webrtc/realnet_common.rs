// Shared support for the webrtc_realnet_* binaries (signaling server,
// publisher, subscriber) — see docs/done/xrds-net-webrtc-realnet-binaries.md.
// Included via `#[path = "realnet_common.rs"] mod realnet_common;` in each
// binary rather than published as its own example (it has no `main`).

use std::collections::HashMap;
use std::time::Duration;

use xrds_net::WebRTCClient;

/// The crate's bundled sample video, embedded directly into the binary at
/// compile time — not just a path to it. These binaries are meant to be
/// built once and then copied to (or run on) a *different* machine for the
/// real-network test; a path baked in via `concat!(env!("CARGO_MANIFEST_DIR"), ...)`
/// would only resolve on the machine that built it, and silently break (or
/// worse, silently point at some unrelated file) anywhere else. Embedding
/// the actual bytes means `--file` is the only way this ever touches the
/// filesystem for the source video, and only if you pass it.
pub static DEFAULT_SAMPLE_VIDEO_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/crates/xrds-net/samples/sample_video.h264"
));

pub const DEFAULT_OUTPUT_DIR: &str = "test_output";

/// Initializes logging with a quieter-than-default filter, unless the
/// operator already set `RUST_LOG` themselves (never overridden — this
/// used to unconditionally stomp any existing `RUST_LOG`).
///
/// Plain `RUST_LOG=info` also enables `warn` (info is *less* severe, not a
/// stricter filter), and `webrtc`'s own dependencies (`webrtc_ice`,
/// `webrtc_sctp`, `webrtc_mdns`) log a lot of `warn!`-level noise that's
/// almost entirely non-actionable in this context: per-interface bind
/// failures while probing every local network adapter (including inactive
/// virtual ones), STUN hosts that don't resolve over IPv6, and
/// post-teardown "connection already closed" messages. None of that
/// affects whether the connection actually worked — `log_ice_summary`
/// above already reports the outcome that matters. Silencing just those
/// three crates keeps `xrds_net`'s own logs (and `webrtc`'s higher-level
/// state-change logs) visible.
pub fn init_logger() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            "info,webrtc_ice=error,webrtc_sctp=error,webrtc_mdns=error",
        );
    }
    let _ = env_logger::try_init();
}

/// Minimal `--flag value` argument parser — deliberately not a dependency
/// (`clap` etc.): the flag surface here is small (3-4 flags per binary),
/// and every other example in this crate is dependency-free too. Positional
/// args and `--flag` (no value) aren't supported; not needed here.
pub struct Args {
    values: HashMap<String, String>,
}

impl Args {
    pub fn parse() -> Self {
        let mut values = HashMap::new();
        let raw: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < raw.len() {
            if let Some(flag) = raw[i].strip_prefix("--") {
                if let Some(value) = raw.get(i + 1) {
                    values.insert(flag.to_string(), value.clone());
                    i += 2;
                    continue;
                }
            }
            i += 1;
        }
        Self { values }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or(default).to_string()
    }

    /// Panics with a clear usage message if `key` wasn't passed — fine for
    /// these operator-run binaries (not library code).
    pub fn required(&self, key: &str) -> String {
        self.get(key)
            .unwrap_or_else(|| panic!("missing required --{key} <value> argument"))
            .to_string()
    }

    pub fn get_u32_or(&self, key: &str, default: u32) -> u32 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    pub fn get_u64_or(&self, key: &str, default: u64) -> u64 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
}

/// Optional `--turn-username`/`--turn-password` flags, applied as env var
/// overrides (`XRDS_TURN_USERNAME`/`XRDS_TURN_PASSWORD`) before any
/// `WebRTCClient` is created — that's the only place `build_ice_servers()`
/// actually reads credentials from (see
/// docs/done/xrds-net-release-readiness.md Phase 1), so this is purely a
/// convenience over exporting the env vars yourself; both do the same
/// thing. Call once, early in `main()`, before creating any `WebRTCClient`
/// — env vars are read at `publish()`/`join_session()` time, not at
/// process startup, but setting them as early as possible avoids surprises
/// if that ever changes. Providing only one of the two is harmless: the
/// TURN entry still requires both to be present, so a partial override
/// just falls back to STUN-only, same as neither being set.
pub fn apply_turn_credentials_from_args(args: &Args) {
    if let Some(username) = args.get("turn-username") {
        std::env::set_var("XRDS_TURN_USERNAME", username);
    }
    if let Some(password) = args.get("turn-password") {
        std::env::set_var("XRDS_TURN_PASSWORD", password);
    }
}

/// Waits for ICE to connect (bounded), then prints the outcome — including
/// the winning candidate pair type via `active_candidate_pair_summary()`,
/// which is the actual evidence of whether a TURN relay was used versus a
/// direct/STUN-assisted path. This is the main thing Phase 3 of
/// docs/done/xrds-net-release-readiness.md needs a human to read off the
/// terminal.
///
/// Returns whether ICE actually connected — **check this** before doing
/// anything that assumes a working connection (sending on the data
/// channel, waiting for a stream). ICE failing here is a real, expected
/// outcome on a flaky/restrictive network, not a bug — but blundering
/// ahead into calls that then panic with a confusing backtrace turns an
/// already-diagnosed, already-printed failure into a crash.
pub async fn log_ice_summary(role: &str, client: &WebRTCClient, max_wait: Duration) -> bool {
    match client.wait_for_ice_connected(max_wait).await {
        Ok(()) => {
            println!("[{role}] ICE connected: {:?}", client.ice_connection_state());
            match client.active_candidate_pair_summary().await {
                Some(summary) => println!("[{role}] Active candidate pair: {summary}"),
                None => println!(
                    "[{role}] Active candidate pair: not available yet (stats may need a moment to catch up)"
                ),
            }
            true
        }
        Err(e) => {
            println!("[{role}] ICE did not connect: {e}");
            false
        }
    }
}

/// Prints a session id in a way that's hard to miss/mis-copy in a scrolling
/// terminal — this is the one piece of information that must be manually
/// carried from the publisher's machine to the subscriber's.
pub fn print_session_id_banner(session_id: &str) {
    let border = "=".repeat(session_id.len() + 4);
    println!("{border}");
    println!("= {session_id} =");
    println!("{border}");
    println!("^ copy this session id to the subscriber: --session-id {session_id}");
}
