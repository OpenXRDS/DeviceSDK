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

//! Shared support code for the protocol-agnostic assessment binaries
//! (`examples/assessment/protocol-agnostic/`). No `main` — each sender/receiver
//! pulls this in with `#[path = "assess_common.rs"] mod common;`, the same way
//! `examples/webrtc/realnet_common.rs` is shared.
//!
//! The assessment is a **content × protocol matrix**: every supported XR
//! content type, over every protocol. One run of a sender/receiver pair walks
//! the whole content suite over one protocol, so the matrix is covered by
//! running each pair once rather than by 5 × N separate invocations.
//!
//! What lives here is everything that must be *identical* across protocols, so
//! that a PASS on MQTT means the same thing as a PASS on FTP:
//!
//! - the content-type vocabulary and the default suite,
//! - the manifest (what was sent: type, name, size, SHA-256, position in the
//!   suite),
//! - the on-the-wire frame for the push transports (WS, MQTT),
//! - integrity verification, the per-item lines, and the suite report,
//!   including the machine-readable result file.
//!
//! Nothing here is protocol-specific, and nothing in the transfer path
//! branches on content type — which is the claim being assessed.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The XR content types XRDS supports. Fixed vocabulary — a sender that names
/// anything else is rejected rather than quietly accepted, so the matrix in the
/// report is exactly the matrix that was agreed.
pub const CONTENT_TYPES: &[(&str, &str)] = &[
    ("gltf-model", "glTF / GLB 3D model"),
    ("texture", "image texture, lightmap or environment map"),
    ("audio", "audio clip (spatial or ambient)"),
    ("video", "video clip (2D or 360)"),
    ("scene-document", "XRDS scene document (.xrds)"),
];

/// One file per supported content type, used when no `--file`/`--files` is
/// given. Kept small on purpose: every protocol has to get through the whole
/// suite in a sitting, and MQTT publishes one broker connection per chunk.
/// Swap in bigger assets with `--files` when throughput is what's being
/// measured.
/// Bare file names, resolved by [`resolve_sample`]. They are copies of assets
/// from `assets/`, not references into it: the kit has to be handed to someone
/// running the assessment on their own machine, and a suite that only works
/// inside a full checkout is not portable.
pub const DEFAULT_SUITE: &[(&str, &str)] = &[
    ("gltf-model", "StainedGlassLamp.gltf"),
    ("texture", "arrow_box.png"),
    ("audio", "spatial_test_ping.wav"),
    ("video", "tiny_test.mp4"),
    ("scene-document", "assessment_scene.xrds"),
];

/// Where `samples/` is looked for, in order.
///
/// The portable kit is a flat folder — five executables and `samples/` beside
/// them — so the first candidate is the directory holding the running binary.
/// That is what makes the same build work both from a checkout and from a kit
/// copied onto an evaluation machine with no toolchain: nothing is baked in
/// that only one of the two layouts can satisfy.
///
/// Only the default suite goes through this. A path given with `--file` /
/// `--files` is used exactly as typed.
pub fn resolve_sample(name: &str) -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. next to the executable: the kit layout.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("samples").join(name));
        }
    }
    // 2. run from inside examples/assessment/protocol-agnostic/.
    candidates.push(PathBuf::from("samples").join(name));
    // 3. run from the repo root, which is what the README tells you to do.
    let from_root = PathBuf::from("examples/assessment/protocol-agnostic/samples").join(name);
    candidates.push(from_root.clone());

    candidates
        .into_iter()
        .find(|p| p.exists())
        // Nothing found: hand back the documented location so the "no such
        // file" names somewhere a reader can act on, rather than a temp dir.
        .unwrap_or(from_root)
}

/// Content type from a file extension. `None` when the extension doesn't map
/// to a supported type — the caller then requires an explicit `--content-type`
/// rather than guessing, since the vocabulary is fixed.
pub fn infer_content_type(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "glb" | "gltf" => Some("gltf-model"),
        "png" | "jpg" | "jpeg" | "ktx2" | "hdr" | "exr" | "basis" => Some("texture"),
        "wav" | "mp3" | "ogg" | "opus" | "flac" => Some("audio"),
        "mp4" | "mkv" | "webm" | "mov" => Some("video"),
        "xrds" => Some("scene-document"),
        _ => None,
    }
}

pub fn is_known_content_type(name: &str) -> bool {
    CONTENT_TYPES.iter().any(|(id, _)| *id == name)
}

fn content_type_list() -> String {
    CONTENT_TYPES
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>()
        .join(", ")
}

/* -------------------------------------------------------------------------- */
/*  Arguments                                                                   */
/* -------------------------------------------------------------------------- */

/// Dirt-simple `--key value` / `--flag` parsing. Deliberately dependency-free:
/// these binaries are run by hand during an evaluation, and a hand-run tool
/// should not need a CLI framework to explain itself.
pub struct Args {
    values: HashMap<String, String>,
}

impl Args {
    pub fn parse() -> Self {
        let mut values = HashMap::new();
        let argv: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < argv.len() {
            let arg = &argv[i];
            if let Some(key) = arg.strip_prefix("--") {
                let next_is_value = argv.get(i + 1).is_some_and(|v| !v.starts_with("--"));
                if next_is_value {
                    values.insert(key.to_string(), argv[i + 1].clone());
                    i += 2;
                } else {
                    values.insert(key.to_string(), "true".to_string());
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        Self { values }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.get(key) == Some("true")
    }

    /// Exits with a usage message rather than panicking — an evaluator running
    /// this by hand should get a sentence, not a backtrace.
    pub fn require(&self, key: &str, usage: &str) -> String {
        match self.get(key) {
            Some(v) => v.to_string(),
            None => {
                eprintln!("error: --{key} is required\n\nusage:\n  {usage}");
                std::process::exit(2);
            }
        }
    }
}

/* -------------------------------------------------------------------------- */
/*  The content suite                                                           */
/* -------------------------------------------------------------------------- */

/// One piece of content to transfer.
pub struct Item {
    pub content_type: String,
    pub file_name: String,
    pub data: Vec<u8>,
}

/// What every sender transfers, resolved from the arguments:
///
/// - nothing given → the default suite, one file per supported content type;
/// - `--file <path> [--content-type <type>]` → that one file;
/// - `--files a,b,c [--types t1,t2,t3]` → those files, types inferred from the
///   extension where `--types` is absent.
///
/// Exits with a clear message on an unreadable file or an unsupported type —
/// the matrix should never be silently wrong.
pub fn resolve_items(args: &Args) -> Vec<Item> {
    let specs: Vec<(Option<String>, PathBuf)> = if let Some(file) = args.get("file") {
        vec![(
            args.get("content-type").map(str::to_string),
            PathBuf::from(file),
        )]
    } else if let Some(files) = args.get("files") {
        let types: Vec<&str> = args
            .get("types")
            .map(|t| t.split(',').map(str::trim).collect())
            .unwrap_or_default();
        files
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .enumerate()
            .map(|(i, path)| {
                (
                    types.get(i).map(|t| t.to_string()),
                    PathBuf::from(path),
                )
            })
            .collect()
    } else {
        DEFAULT_SUITE
            .iter()
            .map(|(content_type, name)| (Some(content_type.to_string()), resolve_sample(name)))
            .collect()
    };

    let mut items = Vec::with_capacity(specs.len());
    for (declared, path) in specs {
        let content_type = match declared.or_else(|| infer_content_type(&path).map(str::to_string)) {
            Some(content_type) => content_type,
            None => {
                eprintln!(
                    "error: cannot infer a content type for {} — pass --content-type (one of: {})",
                    path.display(),
                    content_type_list()
                );
                std::process::exit(2);
            }
        };
        if !is_known_content_type(&content_type) {
            eprintln!(
                "error: '{content_type}' is not a supported XR content type (one of: {})",
                content_type_list()
            );
            std::process::exit(2);
        }
        items.push(Item {
            content_type,
            file_name: file_name_of(&path),
            data: read_payload(&path),
        });
    }

    if items.is_empty() {
        eprintln!("error: no content to transfer");
        std::process::exit(2);
    }
    items
}

/// Read the file to transfer, with a clear message if it isn't there.
pub fn read_payload(path: &Path) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", path.display());
            std::process::exit(2);
        }
    }
}

pub fn file_name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("payload.bin")
        .to_string()
}

/* -------------------------------------------------------------------------- */
/*  Manifest                                                                    */
/* -------------------------------------------------------------------------- */

/// Suffix for the sidecar manifest used by FTP, which has no framing of its own
/// to carry it in.
pub const MANIFEST_SUFFIX: &str = ".xrds-manifest.json";

/// What the sender claims it is sending. The receiver verifies the bytes it
/// actually got against this — that comparison *is* the assessment.
///
/// It also carries who sent it and where to: the receiving machine cannot know
/// either on its own for every protocol, and without them the evidence on disk
/// would not show that the bytes crossed a network at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub run_id: String,
    pub protocol: String,
    pub content_type: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub chunk_size: u32,
    pub chunk_count: u32,
    /// Position of this item in the content suite, and how many items the suite
    /// holds — this is how a receiver knows when the run is complete without
    /// being told separately.
    pub suite_index: u32,
    pub suite_size: u32,
    pub sent_at_unix_ms: u128,
    /// Host name and IP of the machine that sent this, and the URL it sent to
    /// (for a pull protocol, the URL it served on). The IP is this sender's own
    /// address on the route to that target — a listening receiver cannot learn
    /// it any other way through the xrds-net API.
    pub sender_host: String,
    pub sender_ip: String,
    pub target: String,
}

impl Manifest {
    #[allow(clippy::too_many_arguments)]
    pub fn for_item(
        run_id: &str,
        protocol: &str,
        target: &str,
        item: &Item,
        chunk_size: u32,
        suite_index: u32,
        suite_size: u32,
    ) -> Self {
        let chunk_count = if chunk_size == 0 {
            1
        } else {
            item.data.len().div_ceil(chunk_size as usize).max(1) as u32
        };
        Self {
            run_id: run_id.to_string(),
            protocol: protocol.to_string(),
            content_type: item.content_type.clone(),
            file_name: item.file_name.clone(),
            size: item.data.len() as u64,
            sha256: sha256_hex(&item.data),
            chunk_size,
            chunk_count,
            suite_index,
            suite_size,
            sent_at_unix_ms: now_ms(),
            sender_host: host_name(),
            sender_ip: local_ip_towards(target),
            target: target.to_string(),
        }
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("manifest serializes")
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("bad manifest json: {e}"))
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Host name of this machine, from the environment (no extra dependency for
/// something the evidence only needs as a label). `unknown-host` if neither
/// variable is set.
pub fn host_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string())
}

/// This machine's address on its default route — what the other machine should
/// dial. Printed by whichever side listens, so nobody has to look up an IP by
/// hand at the evaluation site.
pub fn local_ip() -> String {
    local_ip_towards("")
}

/* -------------------------------------------------------------------------- */
/*  Binding a port                                                              */
/* -------------------------------------------------------------------------- */

/// How many ports past the requested one to try before giving up.
const PORT_FALLBACK_TRIES: u16 = 10;

/// Claim a usable port, falling back to the next free one.
///
/// This exists because a refused bind is otherwise **silent** on two of these
/// binaries: `XRNetServer` binds inside a spawned task and swallows the error,
/// so the receiver goes on printing "waiting ..." forever while the sender gets
/// "connection refused" — the failure surfaces on the wrong machine, minutes
/// later. Ports get taken for dull reasons at an evaluation site (a previous
/// run still winding down, an unrelated process holding an ephemeral port,
/// a policy reserving a range), so the useful behaviour is to move on and say
/// so, loudly, before anything else is printed.
///
/// Returns the port actually claimed; the socket is closed again, so the real
/// server binds it a moment later. `--strict-port` turns the fallback off for a
/// run that must use one specific port.
pub fn choose_port(kind: PortKind, bind: &str, preferred: u16, strict: bool) -> u16 {
    let tries = if strict { 1 } else { PORT_FALLBACK_TRIES };

    for offset in 0..tries {
        let port = preferred.saturating_add(offset);
        match kind.can_bind(bind, port) {
            Ok(()) => {
                if port != preferred {
                    println!(
                        "  note: port {preferred} is unavailable — using {port} instead \
                         (pass --strict-port to fail instead of moving on)"
                    );
                }
                return port;
            }
            Err(e) if offset + 1 == tries => {
                eprintln!("error: cannot bind {bind}:{port} ({kind}) — {e}");
                if strict {
                    eprintln!("       --strict-port was given, so no other port was tried");
                } else {
                    eprintln!(
                        "       tried {preferred}..={port}; all refused. Check what holds them \
                         (netstat -an | findstr :{preferred}) or pass a free --port"
                    );
                }
                std::process::exit(2);
            }
            Err(_) => continue,
        }
    }
    unreachable!("the loop returns or exits")
}

/// Which kind of socket a port has to serve.
#[derive(Clone, Copy)]
pub enum PortKind {
    Tcp,
    Udp,
}

impl PortKind {
    fn can_bind(self, bind: &str, port: u16) -> std::io::Result<()> {
        match self {
            // Bound and immediately dropped: this is a check, not the real
            // listener. A racing process could still take it in between, which
            // the caller's own bind would then report.
            PortKind::Tcp => std::net::TcpListener::bind((bind, port)).map(|_| ()),
            PortKind::Udp => std::net::UdpSocket::bind((bind, port)).map(|_| ()),
        }
    }
}

impl std::fmt::Display for PortKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PortKind::Tcp => "TCP",
            PortKind::Udp => "UDP",
        })
    }
}

/// Tell the operator exactly what to run on the other machine. Whichever side
/// listens knows the address the other side needs, and `0.0.0.0` in a bind
/// string does not help anyone type it.
pub fn print_peer_command(what: &str, command: &str) {
    println!();
    println!("  on the {what} machine, run:");
    println!("      {command}");
    println!();
}

/// This machine's own IP address on the route towards `target`, so the
/// evidence records which address the bytes actually left from or arrived on.
///
/// A listening receiver never learns the peer's address through the xrds-net
/// API, and `0.0.0.0` in a bind string says nothing — so each side records its
/// own address and the manifest carries the sender's across. Found by
/// `connect`ing a UDP socket, which only fixes the local route: no packet is
/// sent and nothing has to be listening.
///
/// `target` may be a URL (`ws://host:9301/`, userinfo and path included) or a
/// bare `host:port`. Falls back to the default-route address when the target
/// can't be parsed or is a wildcard, and to `unknown-ip` if even that fails.
pub fn local_ip_towards(target: &str) -> String {
    fn probe(dest: &str) -> Option<String> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect(dest).ok()?;
        Some(socket.local_addr().ok()?.ip().to_string())
    }

    // Strip scheme, userinfo and path down to `host[:port]`.
    let authority = target
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(target);
    let authority = authority.split('/').next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);

    let host = authority.rsplit_once(':').map(|(h, _)| h).unwrap_or(authority);
    if !host.is_empty() && host != "0.0.0.0" && host != "*" {
        // Any port works: connect only selects a route.
        if let Some(ip) = probe(&format!("{host}:9")) {
            return ip;
        }
    }

    // Wildcard bind or unparseable target: report the default-route address.
    probe("8.8.8.8:9").unwrap_or_else(|| "unknown-ip".to_string())
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Short, human-typeable run id — it ends up in MQTT topics and file names.
pub fn new_run_id() -> String {
    format!("{:x}", now_ms() & 0xffff_ffff)
}

/* -------------------------------------------------------------------------- */
/*  Frames (push transports: WS, MQTT)                                          */
/* -------------------------------------------------------------------------- */

pub const KIND_MANIFEST: u8 = 0;
pub const KIND_CHUNK: u8 = 1;
pub const KIND_END: u8 = 2;
pub const KIND_ACK: u8 = 3;
pub const KIND_NACK: u8 = 4;

const MAGIC: &[u8; 4] = b"XRDS";
const VERSION: u8 = 1;
/// magic(4) + version(1) + kind(1) + seq(4) + len(4)
pub const HEADER_LEN: usize = 14;

#[derive(Debug, Clone)]
pub struct Frame {
    pub kind: u8,
    pub seq: u32,
    pub payload: Vec<u8>,
}

pub fn encode_frame(kind: u8, seq: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(kind);
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn decode_frame(bytes: &[u8]) -> Result<Frame, String> {
    if bytes.len() < HEADER_LEN {
        return Err(format!("frame too short: {} bytes", bytes.len()));
    }
    if &bytes[0..4] != MAGIC {
        return Err("not an XRDS frame".to_string());
    }
    if bytes[4] != VERSION {
        return Err(format!("unsupported frame version {}", bytes[4]));
    }
    let kind = bytes[5];
    let seq = u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    let len = u32::from_be_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
    if bytes.len() < HEADER_LEN + len {
        return Err(format!(
            "truncated frame: declared {len} bytes, got {}",
            bytes.len() - HEADER_LEN
        ));
    }
    Ok(Frame {
        kind,
        seq,
        payload: bytes[HEADER_LEN..HEADER_LEN + len].to_vec(),
    })
}

pub fn kind_name(kind: u8) -> &'static str {
    match kind {
        KIND_MANIFEST => "manifest",
        KIND_CHUNK => "chunk",
        KIND_END => "end",
        KIND_ACK => "ack",
        KIND_NACK => "nack",
        _ => "unknown",
    }
}

/* -------------------------------------------------------------------------- */
/*  Reassembly (push transports)                                                */
/* -------------------------------------------------------------------------- */

/// Collects the chunks of **one item**. Tolerates duplicates and out-of-order
/// arrival (MQTT retained messages and QoS-1 redelivery both produce these).
/// Reset between items with `start_item`.
#[derive(Default)]
pub struct Reassembly {
    pub manifest: Option<Manifest>,
    chunks: HashMap<u32, Vec<u8>>,
    pub started: Option<Instant>,
}

impl Reassembly {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a new item, discarding anything half-collected from the previous
    /// one.
    pub fn start_item(&mut self, manifest: Manifest) {
        self.chunks.clear();
        self.started = Some(Instant::now());
        self.manifest = Some(manifest);
    }

    pub fn accept_chunk(&mut self, seq: u32, payload: Vec<u8>) {
        self.chunks.insert(seq, payload);
    }

    pub fn received_chunks(&self) -> usize {
        self.chunks.len()
    }

    pub fn elapsed(&self) -> Option<Duration> {
        self.started.map(|t| t.elapsed())
    }

    /// `Some(bytes)` once every chunk the manifest promised has arrived.
    pub fn assembled(&self) -> Option<Vec<u8>> {
        let manifest = self.manifest.as_ref()?;
        if self.chunks.len() != manifest.chunk_count as usize {
            return None;
        }
        let mut out = Vec::with_capacity(manifest.size as usize);
        for seq in 0..manifest.chunk_count {
            out.extend_from_slice(self.chunks.get(&seq)?);
        }
        Some(out)
    }
}

/* -------------------------------------------------------------------------- */
/*  Verification and reporting                                                  */
/* -------------------------------------------------------------------------- */

pub struct Verdict {
    pub pass: bool,
    pub detail: String,
}

pub fn verify(manifest: &Manifest, data: &[u8]) -> Verdict {
    if data.len() as u64 != manifest.size {
        return Verdict {
            pass: false,
            detail: format!("size mismatch: expected {}, got {}", manifest.size, data.len()),
        };
    }
    let actual = sha256_hex(data);
    if actual != manifest.sha256 {
        return Verdict {
            pass: false,
            detail: format!("sha256 mismatch: expected {}, got {}", manifest.sha256, actual),
        };
    }
    Verdict {
        pass: true,
        detail: "byte-identical (size + sha256 match)".to_string(),
    }
}

/// Where received files and their reports go: one folder per run underneath,
/// created on demand.
///
/// In a checkout this is the repo-relative path, so evidence stays with the
/// source it came from. In the portable kit it is `received/` beside the
/// executables — a conductor collecting evidence should find it next to the
/// thing they ran, not three directories down a tree that only makes sense
/// inside this repository. `--out-dir` overrides either.
/// A binary built by cargo sits under `target/`; one in a kit does not. That is
/// the only reliable difference between the two situations at run time, since
/// the working directory is the same in both (the folder you launched from).
fn out_dir_root() -> PathBuf {
    let in_cargo_target = std::env::current_exe()
        .ok()
        .map(|exe| exe.components().any(|c| c.as_os_str() == "target"))
        .unwrap_or(false);

    if in_cargo_target {
        return PathBuf::from("examples/assessment/protocol-agnostic/received");
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("received")))
        .unwrap_or_else(|| PathBuf::from("received"))
}

pub fn default_out_dir() -> PathBuf {
    let dir = out_dir_root();
    let _ = std::fs::create_dir_all(&dir);
    dir
}

struct Row {
    manifest: Manifest,
    received_size: usize,
    received_sha256: String,
    elapsed: Option<Duration>,
    saved_to: PathBuf,
    verdict: Verdict,
}

/// Collects one row per content item, writes the received files, and prints the
/// suite report at the end. Every receiver drives this, so all five protocols
/// report in the same shape and the rows can be concatenated into the
/// assessment matrix.
///
/// **The files it writes are the evidence of the transfer**, so each run gets
/// its own folder — `<out-dir>/<protocol>/<run-id>/` — holding the received
/// payloads, a human-readable `report.txt` and a machine-readable
/// `result.json`. Runs never overwrite each other, and the record names both
/// machines: the sender's host and target come from the manifest, the
/// receiver's from here.
pub struct SuiteReport {
    protocol: &'static str,
    api: &'static str,
    /// What this node connected to, or listened on — the other half of the
    /// endpoint pair recorded in the evidence.
    source: String,
    out_dir: PathBuf,
    rows: Vec<Row>,
    run_id: Option<String>,
    started_at_unix_ms: u128,
}

impl SuiteReport {
    pub fn new(
        protocol: &'static str,
        api: &'static str,
        source: impl Into<String>,
        out_dir: PathBuf,
    ) -> Self {
        Self {
            protocol,
            api,
            source: source.into(),
            out_dir,
            rows: Vec::new(),
            run_id: None,
            started_at_unix_ms: now_ms(),
        }
    }

    pub fn completed(&self) -> usize {
        self.rows.len()
    }

    /// This run's evidence folder, created on demand once the run id is known.
    pub fn run_dir(&self) -> PathBuf {
        let run_id = self.run_id.clone().unwrap_or_else(|| "unknown-run".to_string());
        let dir = self.out_dir.join(self.protocol).join(run_id);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Write one received item into the run's evidence folder, verify it, print
    /// its line, and keep the row. `elapsed` is `None` where the receiving node
    /// cannot honestly time the transfer — a passive upload target (FTP) only
    /// learns a transfer happened once it is over.
    pub fn record(&mut self, manifest: Manifest, data: &[u8], elapsed: Option<Duration>) -> bool {
        self.run_id.get_or_insert_with(|| manifest.run_id.clone());

        let saved_to = self.run_dir().join(&manifest.file_name);
        if let Err(e) = std::fs::write(&saved_to, data) {
            eprintln!("  ! could not write {}: {e}", saved_to.display());
        }

        let verdict = verify(&manifest, data);

        println!(
            "  [{}/{}] {:<15} {:<30} {:>10} B  {:>9}  {}",
            manifest.suite_index + 1,
            manifest.suite_size,
            manifest.content_type,
            manifest.file_name,
            data.len(),
            elapsed
                .map(|e| format!("{:.3} s", e.as_secs_f64()))
                .unwrap_or_else(|| "-".to_string()),
            if verdict.pass { "PASS" } else { "FAIL" }
        );
        if !verdict.pass {
            println!("          {}", verdict.detail);
        }

        let pass = verdict.pass;
        self.rows.push(Row {
            manifest,
            received_size: data.len(),
            received_sha256: sha256_hex(data),
            elapsed,
            saved_to,
            verdict,
        });
        pass
    }

    /// Print the suite report, write `report.txt` and `result.json` next to the
    /// received files, and return the process exit code (0 = every item
    /// passed).
    pub fn finish(self) -> i32 {
        use std::fmt::Write as _;

        let run_id = self.run_id.clone().unwrap_or_else(|| "none".to_string());
        let passed = self.rows.iter().filter(|r| r.verdict.pass).count();
        let total = self.rows.len();
        let run_dir = self.run_dir();

        // The sender's identity travels in the manifest; every item in a run
        // comes from the same sender, so the first row speaks for all of them.
        let (sender_host, sender_ip, target) = self
            .rows
            .first()
            .map(|r| {
                (
                    r.manifest.sender_host.clone(),
                    r.manifest.sender_ip.clone(),
                    r.manifest.target.clone(),
                )
            })
            .unwrap_or_else(|| {
                ("unknown".to_string(), "unknown".to_string(), "unknown".to_string())
            });
        let receiver_ip = local_ip_towards(&self.source);

        // Built as text first so the console and report.txt cannot disagree.
        let mut out = String::new();
        let _ = writeln!(out, "=== XRDS protocol-agnostic transfer: SUITE REPORT ===");
        let _ = writeln!(out, "  protocol      : {}", self.protocol);
        let _ = writeln!(out, "  xrds-net API  : {}", self.api);
        let _ = writeln!(out, "  run id        : {run_id}");
        let _ = writeln!(
            out,
            "  sent from     : host {sender_host} ({sender_ip}) -> {target}"
        );
        let _ = writeln!(
            out,
            "  received on   : host {} ({receiver_ip}) <- {}",
            host_name(),
            self.source
        );
        let _ = writeln!(
            out,
            "  finished      : {} (unix ms)",
            now_ms()
        );
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  {:<15} {:<30} {:>11} {:>9} {:>9}  {}",
            "content type", "file", "bytes", "chunks", "elapsed", "result"
        );
        for row in &self.rows {
            let _ = writeln!(
                out,
                "  {:<15} {:<30} {:>11} {:>9} {:>9}  {}",
                row.manifest.content_type,
                row.manifest.file_name,
                row.received_size,
                row.manifest.chunk_count,
                row.elapsed
                    .map(|e| format!("{:.3} s", e.as_secs_f64()))
                    .unwrap_or_else(|| "-".to_string()),
                if row.verdict.pass { "PASS" } else { "FAIL" }
            );
            let _ = writeln!(
                out,
                "  {:<15} sha256 {} ({})",
                "", row.received_sha256, row.verdict.detail
            );
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "  RESULT        : {passed}/{total} PASS");

        println!();
        print!("{out}");
        println!();

        let rows: Vec<serde_json::Value> = self
            .rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "content_type": row.manifest.content_type,
                    "file_name": row.manifest.file_name,
                    "expected_size": row.manifest.size,
                    "received_size": row.received_size,
                    "expected_sha256": row.manifest.sha256,
                    "received_sha256": row.received_sha256,
                    "chunk_size": row.manifest.chunk_size,
                    "chunk_count": row.manifest.chunk_count,
                    "elapsed_seconds": row.elapsed.map(|e| e.as_secs_f64()),
                    "sent_at_unix_ms": row.manifest.sent_at_unix_ms,
                    "saved_to": row.saved_to.display().to_string(),
                    "detail": row.verdict.detail,
                    "result": if row.verdict.pass { "PASS" } else { "FAIL" },
                })
            })
            .collect();

        let summary = serde_json::json!({
            "run_id": run_id,
            "protocol": self.protocol,
            "api": self.api,
            "sender_host": sender_host,
            "sender_ip": sender_ip,
            "target": target,
            "receiver_host": host_name(),
            "receiver_ip": receiver_ip,
            "source": self.source,
            "items_passed": passed,
            "items_total": total,
            "result": if passed == total && total > 0 { "PASS" } else { "FAIL" },
            "started_at_unix_ms": self.started_at_unix_ms,
            "finished_at_unix_ms": now_ms(),
            "items": rows,
        });

        let report_path = run_dir.join("report.txt");
        if let Err(e) = std::fs::write(&report_path, out.as_bytes()) {
            eprintln!("  (could not write {}: {e})", report_path.display());
        }
        let result_path = run_dir.join("result.json");
        if let Err(e) = std::fs::write(&result_path, serde_json::to_vec_pretty(&summary).unwrap()) {
            eprintln!("  (could not write {}: {e})", result_path.display());
        }

        println!("  evidence      : {}", run_dir.display());
        println!("                  received files + report.txt + result.json");
        println!();

        i32::from(!(passed == total && total > 0))
    }
}

/// The banner every sender prints before it starts, so a screen recording of
/// the evaluation shows what was claimed as well as what happened.
pub fn print_send_banner(
    protocol: &str,
    api: &str,
    target: &str,
    run_id: &str,
    items: &[Item],
    chunk_size: usize,
) {
    let total: usize = items.iter().map(|i| i.data.len()).sum();
    println!();
    println!("=== XRDS protocol-agnostic transfer: SEND ===");
    println!("  protocol      : {protocol}");
    println!("  xrds-net API  : {api}");
    println!("  target        : {target}");
    println!("  run id        : {run_id}");
    println!("  suite         : {} item(s), {total} bytes total", items.len());
    if chunk_size > 0 {
        println!("  chunk size    : {chunk_size} bytes");
    }
    for (index, item) in items.iter().enumerate() {
        println!(
            "    {}. {:<15} {:<30} {:>10} B",
            index + 1,
            item.content_type,
            item.file_name,
            item.data.len()
        );
    }
    println!();
}
