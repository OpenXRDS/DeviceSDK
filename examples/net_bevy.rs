//! Example demonstrating how to integrate xrds-net into a Bevy application.
//!
//! This example shows:
//! 1. How to initialize the net plugin resources
//! 2. How to queue network commands
//! 3. How to process responses in game systems
//! 4. Best practices for demand-oriented network operations

use xrds::*;
use xrds_net::{
    common::enums::PROTOCOLS,
    plugin_net_bevy::{
        process_net_commands, NetClientState, NetCommand, NetOutput, NetPluginConfig,
    },
};
struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "net".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.add_systems(setup);
    }

    fn on_update(&mut self, mut on_update: OnUpdate) {
        // Register the background command processor (runs every frame)
        on_update.add_systems(process_net_commands);

        // Add your game systems that interact with the network
        on_update.add_systems((
            example_send_requests,
            example_handle_responses,
            example_ws_send_requests,
        ));
    }
}

fn setup(mut commands: Commands) {
    // Initialize network plugin resources
    commands.insert_resource(NetPluginConfig::default());
    commands.init_resource::<NetClientState>();

    info!("Network example initialized");
    info!("This example demonstrates:");
    info!("  - Sending HTTP requests on-demand");
    info!("  - Handling responses asynchronously");
    info!("  - Integrating network operations with game logic");
}

fn example_ws_send_requests(
    mut net_state: ResMut<NetClientState>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::KeyW) {
        info!("Connecting to WebSocket server (W pressed)...");

        net_state.push_command(NetCommand::Connect {
            protocol: PROTOCOLS::WSS,
            url: "wss://echo.websocket.org".to_string(),
        });

        net_state.push_command(NetCommand::Send {
            payload: "hello websocket bevy".as_bytes().to_vec(),
            topic: None,
        });

        net_state.push_command(NetCommand::Receive {});

        net_state.push_command(NetCommand::Close);
    }
}

// Example system: Send network requests based on game events/conditions
fn example_send_requests(
    mut net_state: ResMut<NetClientState>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // Example: Press SPACE to trigger an HTTP GET request
    if keyboard.just_pressed(KeyCode::Space) {
        info!("Sending HTTP GET request (Space pressed)...");

        net_state.push_command(NetCommand::Request {
            protocol: PROTOCOLS::HTTP,
            url: "http://example.com".to_string(),
            method: Some("GET".to_string()),
            body: None,
        });
    }

    // Example: Press P to send a POST request
    if keyboard.just_pressed(KeyCode::KeyP) {
        info!("Sending HTTP POST request (P pressed)...");

        // hardcoded JSON body for demonstration
        let json_body = "{\"name\": \"OpenXRDS\", \"type\": \"example\"}";

        net_state.push_command(NetCommand::Request {
            protocol: PROTOCOLS::HTTP,
            url: "https://httpbin.org/post".to_string(),
            method: Some("POST".to_string()),
            body: Some(json_body.to_string()),
        });
    }

    // Developers can add their own conditions:
    // - Time-based triggers
    // - Entity state changes
    // - User interactions
    // - Game events
}

// Example system: Process network responses
fn example_handle_responses(mut net_state: ResMut<NetClientState>) {
    // Poll for completed network operations
    while let Some(output) = net_state.pop_output() {
        match output {
            NetOutput::Connected { client_id } => {
                info!(
                    "Successfully connected to server (Client ID: {})",
                    client_id
                );
                // Developers can:
                // - Update game state to reflect connection status
                // - Trigger events for other systems
                // - Log telemetry for successful connections
            }
            NetOutput::Response(response) => {
                info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                info!("Network Response Received:");
                info!("  Protocol: {:?}", response.protocol);
                info!("  Status: {}", response.status_code);
                info!("  Headers: {} total", response.headers.len());

                // Show body preview
                if !response.body.is_empty() {
                    let body_str = String::from_utf8_lossy(&response.body);
                    let preview_len = body_str.len().min(200);
                    info!("  Body preview: {}...", &body_str[..preview_len]);
                }

                if let Some(error) = response.error {
                    error!("  Error: {}", error);
                }

                // Developers can:
                // - Update game state based on response
                // - Spawn entities with downloaded data
                // - Trigger events for other systems
                // - Cache responses in resources

                info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
            NetOutput::Sent => {
                info!("Data sent successfully");
                // Developers can:
                // - Update UI to show send status
                // - Log telemetry for sent data
                // - Prepare to receive response
            }
            NetOutput::Received { payload } => {
                info!("Data received: {} bytes", payload.len());
                info!(
                    "Payload preview: {}",
                    String::from_utf8_lossy(&payload[..payload.len().min(100)])
                );
                // Developers can:
                // - Process incoming data
                // - Update game state or UI
                // - Log telemetry for received data
            }
            NetOutput::Closed => {
                info!("Connection closed");
                // Developers can:
                // - Update game state to reflect disconnection
                // - Trigger events for other systems
                // - Log telemetry for connection lifecycle
            }
            NetOutput::Error(err) => {
                error!("Network operation failed: {}", err);

                // Developers can:
                // - Retry failed requests
                // - Show error UI to user
                // - Fall back to cached data
                // - Log telemetry
            }
        }
    }
}
