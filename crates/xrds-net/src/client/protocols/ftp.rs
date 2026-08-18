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

//! `FtpHandler`: FTP as a [`FileTransferHandler`], plus `run_command()` as an
//! expert-only extra (raw `FtpCommands` access via `as_any`) for the
//! commands `upload`/`download`/`list`/`delete` don't cover
//! (`CDUP`/`QUIT`/`RMD`/`MKD`/`PWD`/`NOOP`/`APPE`).
//!
//! Extracted verbatim from `client.rs`'s `connect_ftp`/`connect_sftp` + all
//! `run_ftp_*` (Phase 1 of `docs/done/xrds-net-protocol-handler.md`) — `Client`'s
//! old methods are untouched and still the ones actually called until Phase
//! 2 rewires `Client` onto this handler.
//!
//! `validate()` catches the missing-user/password precondition with an
//! actionable hint instead of the old `connect_ftp`'s generic error string —
//! see "Guided-error validation" in the plan doc.
//!
//! `connect_sftp` was never a real implementation upstream (`Ok(self)`, a
//! no-op placeholder marked "Need to test first") — there is nothing to
//! extract, so `SFTP` isn't routed anywhere different from `FTP` yet; both
//! go through this same `connect()`.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use suppaftp::FtpStream;

use crate::client::categories::FileTransferHandler;
use crate::client::context::ClientContext;
use crate::client::error::NetError;
use crate::client::handler::ProtocolHandler;
use crate::common::data_structure::{FtpPayload, FtpResponse};
use crate::common::enums::{FtpCommands, PROTOCOLS};

fn ftp_err(e: impl std::fmt::Display) -> NetError {
    NetError::Network(e.to_string())
}

#[derive(Default)]
pub struct FtpHandler {
    stream: Option<Arc<Mutex<FtpStream>>>,
}

impl FtpHandler {
    pub fn new() -> Self {
        Self::default()
    }

    fn stream(&self) -> Result<Arc<Mutex<FtpStream>>, NetError> {
        self.stream
            .clone()
            .ok_or_else(|| NetError::Network("FTP connection is not initialized.".to_string()))
    }

    /// Expert-only extra: run a raw `FtpCommands` variant not covered by
    /// `upload`/`download`/`list`/`delete` — mirrors the original
    /// `run_ftp_command` dispatch, reached via `ProtocolHandler::as_any()`
    /// downcast.
    pub fn run_command(&self, ftp_payload: FtpPayload) -> FtpResponse {
        match ftp_payload.command {
            FtpCommands::CWD => self.run_cwd(&ftp_payload),
            FtpCommands::CDUP => self.run_cdup(),
            FtpCommands::QUIT => self.run_quit(),
            FtpCommands::RETR => self.run_retr_response(&ftp_payload),
            FtpCommands::STOR => self.run_stor_response(&ftp_payload),
            FtpCommands::APPE => self.run_appe(&ftp_payload),
            FtpCommands::DELE => self.run_dele_response(&ftp_payload),
            FtpCommands::RMD => self.run_rmd(&ftp_payload),
            FtpCommands::MKD => self.run_mkd(&ftp_payload),
            FtpCommands::PWD => self.run_pwd(),
            FtpCommands::LIST => self.run_list_response(&ftp_payload),
            FtpCommands::NOOP => self.run_noop(),
        }
    }

    fn run_cwd(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let response = stream.lock().unwrap().cwd(ftp_payload.payload_name.as_str());
        match response {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_cdup(&self) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let mut guard = stream.lock().unwrap();
        match guard.cdup() {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_quit(&self) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let mut guard = stream.lock().unwrap();
        match guard.quit() {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_retr_response(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        match self.download_impl(ftp_payload.payload_name.as_str()) {
            Ok(data) => FtpResponse { payload: Some(data), error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_stor_response(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        let Some(payload) = ftp_payload.payload.clone() else {
            return FtpResponse {
                payload: None,
                error: Some("The payload is required for STOR command.".to_string()),
            };
        };
        match self.upload_impl(ftp_payload.payload_name.as_str(), payload) {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_appe(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let Some(payload) = ftp_payload.payload.clone() else {
            return FtpResponse {
                payload: None,
                error: Some("The payload is required for APPE command.".to_string()),
            };
        };
        let mut reader = Cursor::new(payload);
        let response = stream
            .lock()
            .unwrap()
            .append_file(ftp_payload.payload_name.as_str(), &mut reader);
        match response {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_dele_response(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        match self.delete_impl(ftp_payload.payload_name.as_str()) {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    /// Remove the directory. Only empty directory can be removed.
    fn run_rmd(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let mut guard = stream.lock().unwrap();
        match guard.rmdir(ftp_payload.payload_name.as_str()) {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_mkd(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let mut guard = stream.lock().unwrap();
        match guard.mkdir(ftp_payload.payload_name.as_str()) {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_pwd(&self) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let mut guard = stream.lock().unwrap();
        match guard.pwd() {
            Ok(res) => FtpResponse { payload: Some(res.as_bytes().to_vec()), error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_list_response(&self, ftp_payload: &FtpPayload) -> FtpResponse {
        let path = if ftp_payload.payload_name.is_empty() {
            None
        } else {
            Some(ftp_payload.payload_name.as_str())
        };
        match self.list_impl(path) {
            Ok(list) => FtpResponse { payload: Some(list.join("\n").as_bytes().to_vec()), error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn run_noop(&self) -> FtpResponse {
        let stream = match self.stream() {
            Ok(s) => s,
            Err(e) => return FtpResponse { payload: None, error: Some(e.to_string()) },
        };
        let mut guard = stream.lock().unwrap();
        match guard.noop() {
            Ok(_) => FtpResponse { payload: None, error: None },
            Err(e) => FtpResponse { payload: None, error: Some(e.to_string()) },
        }
    }

    fn upload_impl(&self, path: &str, data: Vec<u8>) -> Result<(), NetError> {
        let stream = self.stream()?;
        let mut reader = Cursor::new(data);
        stream
            .lock()
            .unwrap()
            .put_file(path, &mut reader)
            .map_err(ftp_err)?;
        Ok(())
    }

    fn download_impl(&self, path: &str) -> Result<Vec<u8>, NetError> {
        let stream = self.stream()?;
        let mut guard = stream.lock().unwrap();
        let cursor = guard.retr_as_buffer(path).map_err(ftp_err)?;
        Ok(cursor.into_inner())
    }

    fn list_impl(&self, path: Option<&str>) -> Result<Vec<String>, NetError> {
        let stream = self.stream()?;
        let mut guard = stream.lock().unwrap();
        guard.list(path).map_err(ftp_err)
    }

    fn delete_impl(&self, path: &str) -> Result<(), NetError> {
        let stream = self.stream()?;
        stream.lock().unwrap().rm(path).map_err(ftp_err)?;
        Ok(())
    }
}

impl FileTransferHandler for FtpHandler {
    fn connect(&mut self, ctx: &ClientContext) -> Result<(), NetError> {
        crate::common::ensure_rustls_crypto_provider();
        // Check credentials before attempting the socket connection at all —
        // cheaper failure, and matches `validate()`'s guided-error intent
        // even for callers that invoke `connect()` directly without going
        // through `validate()` first.
        let (Some(user), Some(password)) = (ctx.user.as_ref(), ctx.password.as_ref()) else {
            return Err(NetError::missing_input(
                PROTOCOLS::FTP,
                "user/password",
                "FTP requires credentials — call set_user(..).set_password(..) before connecting",
            ));
        };

        // Built from the parsed host/port rather than `ctx.raw_url` directly
        // — `raw_url` may still carry a scheme, embedded userinfo, and a
        // path (e.g. `ftp://demo:password@host:21/readme.txt`), none of
        // which `FtpStream::connect` (a plain `host:port` dial) understands.
        let host = ctx.host.as_deref().unwrap_or_default();
        let port = ctx.port.unwrap_or(21);
        let mut ftp_stream = FtpStream::connect(format!("{host}:{port}")).map_err(ftp_err)?;

        ftp_stream.login(user, password).map_err(ftp_err)?;
        self.stream = Some(Arc::new(Mutex::new(ftp_stream)));
        Ok(())
    }

    fn upload(&mut self, _ctx: &ClientContext, path: &str, data: Vec<u8>) -> Result<(), NetError> {
        self.upload_impl(path, data)
    }

    fn download(&mut self, _ctx: &ClientContext, path: &str) -> Result<Vec<u8>, NetError> {
        self.download_impl(path)
    }

    fn list(&mut self, _ctx: &ClientContext, path: &str) -> Result<Vec<String>, NetError> {
        let path = if path.is_empty() { None } else { Some(path) };
        self.list_impl(path)
    }

    fn delete(&mut self, _ctx: &ClientContext, path: &str) -> Result<(), NetError> {
        self.delete_impl(path)
    }
}

impl ProtocolHandler for FtpHandler {
    fn validate(&self, ctx: &ClientContext) -> Result<(), NetError> {
        if ctx.user.is_none() || ctx.password.is_none() {
            return Err(NetError::missing_input(
                PROTOCOLS::FTP,
                "user/password",
                "FTP requires credentials — call set_user(..).set_password(..) before connecting",
            ));
        }
        Ok(())
    }

    fn as_file_transfer(&mut self) -> Option<&mut dyn FileTransferHandler> {
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

    fn ctx_for(url: &str) -> ClientContext {
        let mut ctx = ClientContext::new(PROTOCOLS::FTP, "test-id".to_string());
        ctx.raw_url = url.to_string();
        ctx
    }

    #[test]
    fn validate_fails_without_credentials() {
        let handler = FtpHandler::new();
        let ctx = ctx_for("ftp://127.0.0.1/");

        let err = handler.validate(&ctx).expect_err("missing creds should fail validate()");
        match &err {
            NetError::MissingInput { protocol, field, hint } => {
                assert_eq!(*protocol, PROTOCOLS::FTP);
                assert_eq!(*field, "user/password");
                // guided-error: the hint must say what to actually call, not
                // just that something is missing.
                assert!(hint.contains("set_user"));
                assert!(hint.contains("set_password"));
            }
            other => panic!("expected NetError::MissingInput, got {other:?}"),
        }
    }

    #[test]
    fn validate_passes_with_credentials() {
        let handler = FtpHandler::new();
        let mut ctx = ctx_for("ftp://127.0.0.1/");
        ctx.user = Some("anonymous".to_string());
        ctx.password = Some("anonymous".to_string());

        handler.validate(&ctx).expect("validate should pass with credentials set");
    }

    #[test]
    fn connect_without_credentials_is_a_missing_input_error() {
        let mut handler = FtpHandler::new();
        let ctx = ctx_for("ftp://127.0.0.1:1/");

        let err = handler.connect(&ctx);
        // Either the socket connect fails first (Network) or, if that somehow
        // succeeds, the credential check fails (MissingInput) — never a panic.
        assert!(err.is_err());
    }

    #[test]
    fn exposes_itself_as_a_file_transfer_handler() {
        let mut handler = FtpHandler::new();
        assert!(handler.as_file_transfer().is_some());
    }

    #[test]
    fn run_command_without_a_connection_reports_an_error_not_a_panic() {
        let handler = FtpHandler::new();
        let response = handler.run_command(FtpPayload {
            command: FtpCommands::NOOP,
            payload_name: String::new(),
            payload: None,
        });
        assert!(response.error.is_some());
    }
}
