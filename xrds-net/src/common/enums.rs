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

 #[derive(Debug, Clone, Copy, PartialEq)]
 pub enum PROTOCOLS {
    HTTP,
    HTTPS,
    FILE,
    COAP,
    // COAPS,
    MQTT,
    FTP,
    SFTP,
    WS,
    WSS,
    WEBRTC,
    HTTP3,
    QUIC
}

/**
 * https://en.wikipedia.org/wiki/List_of_FTP_commands
 */
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FtpCommands {
    // USER,
    // PASS,
    // ACCT,
    CWD,
    CDUP,
    // SMNT,
    QUIT,
    // REIN,
    // PORT,
    // PASV,
    // TYPE,
    // STRU,
    // MODE,
    RETR,
    STOR,
    // STOU,
    APPE,
    // ALLO,
    // REST,
    // RNFR,
    // RNTO,
    // ABOR,
    DELE,
    RMD,
    MKD,
    PWD,
    LIST,
    // NLST,
    // SITE,
    // SYST,
    // STAT,
    // HELP,
    NOOP
}