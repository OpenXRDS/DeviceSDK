use crate::{
    application::{RuntimeApplication, RuntimeEvent},
    Context,
};

use std::{
    thread::sleep,
    time::{Duration, SystemTime},
};

use log::debug;
use tokio::task::JoinHandle;
use winit::event_loop;

///
/// TODO!: Remove anyhow
pub trait RuntimeHandler {
    fn on_construct(&mut self) -> anyhow::Result<()>;
    fn on_begin(&mut self, context: Context) -> anyhow::Result<()>;
    fn on_resumed(&mut self, context: Context) -> anyhow::Result<()>;
    fn on_suspended(&mut self, context: Context) -> anyhow::Result<()>;
    fn on_end(&mut self, context: Context) -> anyhow::Result<()>;
    fn on_update(&mut self, context: Context) -> anyhow::Result<()>;
    fn on_deconstruct(&mut self, context: Context) -> anyhow::Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub enum RuntimeTarget {
    #[default]
    Xr,
    Window,
    XrWithPreview,
}

#[derive(Debug, Default, Clone)]
pub struct RuntimeWindowOptions {
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub resizable: bool,
    pub fullscreen: bool,
    pub decorated: bool,
}

#[derive(Debug)]
pub struct Runtime {
    main_thread: tokio::runtime::Runtime,
}

#[derive(Debug, Clone)]
pub struct RuntimeParameters {
    pub app_name: String,
    pub target: RuntimeTarget,
    pub window_options: Option<RuntimeWindowOptions>,
}

impl Runtime {
    pub fn new(params: RuntimeParameters) -> Self {
        let main_context = tokio::runtime::Builder::new_multi_thread()
            .thread_name(format!("{}-main", params.app_name))
            .build()
            .expect("Could not create main runtime");
        Self {
            main_thread: main_context,
        }
    }

    pub fn run_block<A>(self, app: A) -> anyhow::Result<()>
    where
        A: RuntimeHandler + Send + Sync + 'static,
    {
        let render_runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("Could not create render runtime");

        render_runtime.block_on(self.run(app))
    }

    pub async fn run<A>(self, app: A) -> anyhow::Result<()>
    where
        A: RuntimeHandler + Send + Sync + 'static,
    {
        // We are running in render thread
        let main_runtime_handle = self.main_thread.handle().clone();

        let mut runtime_app = RuntimeApplication::new(app)?;

        let event_loop = event_loop::EventLoop::<RuntimeEvent>::with_user_event().build()?;
        let event_proxy = event_loop.create_proxy();

        let main_event_proxy = event_proxy.clone();
        let main_result: JoinHandle<anyhow::Result<()>> = main_runtime_handle.spawn(async move {
            let event_proxy = main_event_proxy;

            let tick_rate = Duration::from_secs_f32(1.0 / 120.0);
            let mut before = SystemTime::now();

            loop {
                let diff = SystemTime::now().duration_since(before)?;
                if diff >= tick_rate {
                    match event_proxy.send_event(RuntimeEvent::Tick(diff)) {
                        Ok(_) => {}
                        Err(_) => break,
                    }
                    before = SystemTime::now();
                }
                match event_proxy.send_event(RuntimeEvent::RedrawRequested) {
                    Ok(_) => {}
                    Err(_) => break,
                }
                sleep(Duration::from_millis(1));
            }
            Ok(())
        });

        event_loop.run_app(&mut runtime_app)?;
        debug!("Event loop closed");

        main_result.await??;

        Ok(())
    }
}
