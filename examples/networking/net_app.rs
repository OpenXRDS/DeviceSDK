// Networking from inside an `XrdsApp` — the recommended in-app surface.
//
// Both network calls run off the frame thread; `update()` only ever does
// non-blocking polls, so the render loop is never stalled:
//   - one-shot request via `Option<XrdsNetTask>::take_ready()`
//   - ongoing stream via `NetFeed` (`try_recv` / `take_error`)
//
// Compare with:
//   - examples/net_intent.rs — the same verbs called synchronously (standalone,
//     no runtime), fine for scripts/tests but would freeze a frame here.
//   - examples/net.rs — the expert `ClientBuilder`/`Client` session API.
use xrds::net::{
    Event, ListenOptions, NetFeed, NetResponse, NetTaskSlot, Overflow, RequestOptions, XrdsNet,
    XrdsNetTask,
};
use xrds::sdk::{primitives::XrdsCube, world::XrdsCamera};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

#[derive(Default)]
struct NetApp {
    // A one-shot request in flight — polled each frame, self-clears when done.
    manifest_request: Option<XrdsNetTask<NetResponse>>,
    // An ongoing subscription — drained each frame.
    telemetry: Option<NetFeed>,
}

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "NetApp".to_owned(),
        ..Default::default()
    });
    runtime
        .run_xrds(NetApp::default())
        .expect("Could not run application");
}

impl XrdsApp for NetApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        // Minimal scene so this is a real runnable XrdsApp.
        api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("NetCamera")
                .at([0.0, 1.5, 5.0])
                .looking_at([0.0, 0.5, 0.0])
        });
        api.spawn(&{
            let mut cube = XrdsCube::new().with_name("NetCube");
            cube.transform.translation = [0.0, 0.5, 0.0];
            cube
        });

        // One-shot: fetch a manifest off the frame thread. The returned task
        // is just a plain field we poll in `update()`.
        self.manifest_request = Some(XrdsNet::request_async(
            "http://www.rust-lang.org:80/",
            RequestOptions::get(),
        ));

        // Stream: subscribe to a telemetry topic. Live-feed config — a shallow
        // buffer + drop-oldest, so we always hold the freshest few messages and
        // never stall the network thread or bloat memory (the shape a video
        // feed would use; a lossless feed would use `ListenOptions::default()`
        // or plain `listen_feed`). A connect/subscribe failure surfaces via
        // `take_error()` — no `Result` to handle at construction.
        let opts = ListenOptions {
            buffer: 4,
            overflow: Overflow::DropOldest,
        };
        self.telemetry = Some(XrdsNet::listen_feed_with(
            "mqtt://test.mosquitto.org:1883/xrds-net/examples/telemetry",
            opts,
        ));
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {
        // One-shot: non-blocking; `take_ready` hands back the owned result once
        // and clears the slot for us.
        if let Some(result) = self.manifest_request.take_ready() {
            match result {
                Ok(NetResponse {
                    status_code, body, ..
                }) => println!("manifest: status {status_code}, {} bytes", body.len()),
                Err(e) => eprintln!("manifest request failed: {e}"),
            }
        }

        // Stream: drain everything that arrived since last frame — never blocks,
        // and yields nothing until the subscription is live.
        if let Some(feed) = &mut self.telemetry {
            while let Some(Event { payload, .. }) = feed.try_recv() {
                println!("telemetry: {} bytes", payload.len());
                // (a real app hands `payload` to a decoder / applies it to the scene)
            }
            if let Some(e) = feed.take_error() {
                eprintln!("telemetry feed failed: {e}");
                self.telemetry = None; // drop is non-blocking
            }
        }
    }
}
