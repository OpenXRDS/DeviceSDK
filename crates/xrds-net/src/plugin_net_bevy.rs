use std::collections::VecDeque;

use bevy::prelude::*;

use crate::{
    client::{Client, ClientBuilder},
    common::{data_structure::NetResponse, enums::PROTOCOLS},
};

#[derive(Resource, Clone, Debug)]
pub struct NetPluginConfig {
    pub log_errors: bool,
}

impl Default for NetPluginConfig {
    fn default() -> Self {
        Self { log_errors: true }
    }
}

#[derive(Debug, Clone)]
pub enum NetCommand {
    Connect {
        protocol: PROTOCOLS,
        url: String,
    },
    Send {
        payload: Vec<u8>,
        topic: Option<String>,
    },
    Receive,
    Request {
        protocol: PROTOCOLS,
        url: String,
        method: Option<String>,
        body: Option<String>,
    },
    Close,
}

#[derive(Debug, Clone)]
pub enum NetOutput {
    Connected { client_id: String },
    Sent,
    Received { payload: Vec<u8> },
    Response(NetResponse),
    Closed,
    Error(String),
}

#[derive(Resource, Default)]
pub struct NetClientState {
    client: Option<Client>,
    commands: VecDeque<NetCommand>,
    outputs: VecDeque<NetOutput>,
}

impl NetClientState {
    pub fn push_command(&mut self, command: NetCommand) {
        self.commands.push_back(command);
    }

    pub fn pop_output(&mut self) -> Option<NetOutput> {
        self.outputs.pop_front()
    }

    pub fn has_connection(&self) -> bool {
        self.client.is_some()
    }
}

pub struct NetPlugin {
    pub config: NetPluginConfig,
}

impl Default for NetPlugin {
    fn default() -> Self {
        Self {
            config: NetPluginConfig::default(),
        }
    }
}

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .init_resource::<NetClientState>()
            .add_systems(Update, process_net_commands);
    }
}

pub fn process_net_commands(mut state: ResMut<NetClientState>, config: Res<NetPluginConfig>) {
    while let Some(command) = state.commands.pop_front() {
        match command {
            NetCommand::Connect { protocol, url } => {
                let client = ClientBuilder::new()
                    .set_protocol(protocol)
                    .build()
                    .set_url(&url);
                let result = client.connect();

                match result {
                    Ok(client) => {
                        let client_id = client.id.clone();
                        state.client = Some(client);
                        state.outputs.push_back(NetOutput::Connected { client_id });
                    }
                    Err(error) => push_error(&mut state, &config, error),
                }
            }
            NetCommand::Send { payload, topic } => {
                let Some(client) = state.client.take() else {
                    push_error(
                        &mut state,
                        &config,
                        "No active connection to send data".to_owned(),
                    );
                    continue;
                };

                match client.send(payload, topic.as_deref()) {
                    Ok(client) => {
                        state.client = Some(client);
                        state.outputs.push_back(NetOutput::Sent);
                    }
                    Err(error) => {
                        push_error(&mut state, &config, error);
                    }
                }
            }
            NetCommand::Receive => {
                let Some(client) = state.client.as_ref() else {
                    push_error(
                        &mut state,
                        &config,
                        "No active connection to receive data".to_owned(),
                    );
                    continue;
                };

                match client.rcv() {
                    Ok(payload) => state.outputs.push_back(NetOutput::Received { payload }),
                    Err(error) => push_error(&mut state, &config, error),
                }
            }
            NetCommand::Request {
                protocol,
                url,
                method,
                body,
            } => {
                let mut client = ClientBuilder::new()
                    .set_protocol(protocol)
                    .build()
                    .set_url(&url);
                if let Some(method) = method {
                    client = client.set_method(&method);
                }
                if let Some(body) = body {
                    client = client.set_req_body(&body);
                }

                let response = client.request();
                state.outputs.push_back(NetOutput::Response(response));
            }
            NetCommand::Close => {
                let Some(client) = state.client.as_ref() else {
                    state.outputs.push_back(NetOutput::Closed);
                    continue;
                };

                match client.close() {
                    Ok(()) => {
                        state.client = None;
                        state.outputs.push_back(NetOutput::Closed);
                    }
                    Err(error) => push_error(&mut state, &config, error),
                }
            }
        }
    }
}

fn push_error(state: &mut NetClientState, config: &NetPluginConfig, error: String) {
    if config.log_errors {
        error!("[xrds-net] {error}");
    }
    state.outputs.push_back(NetOutput::Error(error));
}
