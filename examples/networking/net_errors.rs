// Focused example: the structured `NetError` model.
//
// Every xrds-net call returns `Result<_, NetError>`. Rather than a stringly
// error, the variants let you tell "this protocol can't do that" from "you
// forgot an input" from "the network failed" — programmatically. Most cases
// below need NO network: they fail before any socket is opened.
//
// See MANUAL.md §6 for the full reference.
use xrds_net::{NetError, RequestOptions, TransferOp, XrdsNet};

/// Turn any `NetError` into a human category + guidance — the shape a real app
/// would use to decide whether to retry, prompt the user, or give up.
fn describe(err: &NetError) -> String {
    match err {
        NetError::UnrecognizedScheme(scheme) => {
            format!("unrecognized scheme '{scheme}' — no protocol maps to it (fix the URL)")
        }
        NetError::Capability { protocol, verb, detail } => {
            format!("{protocol:?} does not support '{verb}': {detail} (use a different verb/protocol)")
        }
        NetError::MissingInput { protocol, field, hint } => {
            format!("{protocol:?} needs '{field}' first — {hint}")
        }
        NetError::Network(msg) => format!("transport/I-O failure: {msg} (maybe retry)"),
        NetError::Protocol(msg) => format!("server rejected the request: {msg}"),
    }
}

fn show(label: &str, result: Result<impl std::fmt::Debug, NetError>) {
    match result {
        Ok(value) => println!("[{label}] ok: {value:?}"),
        Err(e) => println!("[{label}] {}", describe(&e)),
    }
}

pub fn main() {
    println!("xrds-net: NetError variants\n");

    // UnrecognizedScheme — the URL scheme maps to no protocol. No network.
    show(
        "unrecognized-scheme",
        XrdsNet::request("gopher://example.com/x", RequestOptions::get()),
    );

    // Capability — the protocol fundamentally can't do this verb. No network:
    // the capability is checked before any connection is attempted.
    show(
        "http-cannot-dispatch",
        XrdsNet::dispatch("https://example.com/x", b"nope".to_vec()),
    );
    show(
        "ws-cannot-request",
        XrdsNet::request("ws://example.com/x", RequestOptions::get()),
    );

    // MissingInput — a required input is absent, and the hint says what to add.
    // FTP needs credentials (as URL userinfo). No network: validate() fails first.
    show(
        "ftp-missing-credentials",
        XrdsNet::transfer("ftp://example.com/file.txt", TransferOp::Download),
    );

    // Network — transport/DNS/TLS failure. This one *does* attempt a
    // connection, to a host that doesn't resolve.
    show(
        "bad-host",
        XrdsNet::request("http://nonexistent.invalid./", RequestOptions::get()),
    );

    // Protocol — a server-side rejection *after* a successful transport (auth
    // denied, broker refused, ...). Not forced here; it surfaces the same way
    // — `match NetError::Protocol(_)` — when a reachable server says no.
    println!("\n(NetError::Protocol appears when a reachable server rejects the request.)");
}
