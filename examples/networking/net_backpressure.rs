// Focused example: listen() buffering — `ListenOptions` + `Overflow`.
//
// A `listen` stream reads events on a background thread into a BOUNDED buffer,
// so a fast producer (e.g. a video feed) can't grow memory without bound when
// the consumer falls behind. You choose what a full buffer does.
//
// See MANUAL.md §7.
use std::time::Duration;

use xrds_net::{ListenOptions, Overflow, XrdsNet};

pub fn main() {
    println!("xrds-net: listen backpressure (ListenOptions / Overflow)\n");

    // Lossless (the default): the producer waits for space. Over a TCP-backed
    // transport that becomes TCP flow control and the sender throttles —
    // nothing is dropped. Right for telemetry / VOD.
    let lossless = ListenOptions::default();
    println!("lossless : buffer {}, {:?}", lossless.buffer, lossless.overflow);

    // Live (drop-oldest): a shallow buffer that always keeps the freshest few
    // messages and never stalls the network thread. Bounds memory AND latency
    // at the cost of losing stale data. Right for real-time video, where a
    // late frame is worthless.
    let live = ListenOptions {
        buffer: 4,
        overflow: Overflow::DropOldest,
    };
    println!("live     : buffer {}, {:?}\n", live.buffer, live.overflow);

    // Run a short live round-trip with the drop-oldest config to show
    // `listen_with` in use. (test.mosquitto.org is a public MQTT broker.)
    let topic = "mqtt://test.mosquitto.org:1883/xrds-net/examples/backpressure";
    let stream = match XrdsNet::listen_with(topic, live) {
        Ok(s) => s,
        Err(e) => {
            println!("listen failed: {e}");
            return;
        }
    };

    // Publish several messages quickly.
    for i in 0..5u8 {
        if let Err(e) = XrdsNet::dispatch(topic, vec![i]) {
            println!("dispatch {i} failed: {e}");
            return;
        }
    }

    // Drain non-blockingly-ish via recv_timeout. With buffer:4 + drop-oldest,
    // had the producer badly outrun us the oldest would have been dropped —
    // we keep the freshest. (Illustrative; exact counts depend on timing.)
    let mut count = 0;
    while let Ok(event) = stream.recv_timeout(Duration::from_secs(2)) {
        println!("received: {:?}", event.payload);
        count += 1;
    }
    println!("\ndrained {count} message(s); stream closing");
    stream.close();
}
