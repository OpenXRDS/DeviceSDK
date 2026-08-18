// Standalone counterpart to examples/networking/net_app.rs — the "server"
// side of a Bevy-integration smoke test. Loops XrdsNet::dispatch() to the
// same MQTT topic net_app.rs subscribes to via listen_feed(), once a
// second, so a person watching the net_app window can see live server
// pushes actually reach a running XrdsApp's update() loop without stalling
// the frame.
//
// Run with net_app.rs in a second terminal:
//   cargo run --example net_app_publisher
//   cargo run --example net_app
use std::time::Duration;

use xrds_net::XrdsNet;

fn main() {
    let topic = "mqtt://test.mosquitto.org:1883/xrds-net/examples/telemetry";
    let mut tick: u32 = 0;
    loop {
        tick += 1;
        match XrdsNet::dispatch(topic, format!("tick {tick}").into_bytes()) {
            Ok(()) => println!("published tick {tick}"),
            Err(e) => eprintln!("publish failed: {e}"),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
