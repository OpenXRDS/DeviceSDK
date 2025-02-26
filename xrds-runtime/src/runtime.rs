use crate::application::{RuntimeApplication, RuntimeEvent};

use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::{Duration, SystemTime},
};

use crate::error::RuntimeError;
use log::debug;
use tokio::{runtime::Handle, task::JoinHandle};
use winit::event_loop;

pub trait RuntimeHandler {
    fn on_construct(&mut self);
    fn on_begin(&mut self);
    fn on_resumed(&mut self);
    fn on_suspended(&mut self);
    fn on_end(&mut self);
    fn on_update(&mut self);
    fn on_deconstruct(&mut self);
}

pub struct Runtime {
    main_thread: tokio::runtime::Runtime,
}

pub struct RuntimeParameters {
    pub app_name: String,
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
        let render_runtime_handle = Handle::current();
        let main_runtime_handle = self.main_thread.handle().clone();

        let app = Arc::new(RwLock::new(app));
        let mut runtime_app = RuntimeApplication::default();

        let event_loop = event_loop::EventLoop::<RuntimeEvent>::with_user_event().build()?;
        let event_proxy = event_loop.create_proxy();

        let main_app = app.clone();
        let main_event_proxy = event_proxy.clone();
        let main_result: JoinHandle<anyhow::Result<()>> = main_runtime_handle.spawn(async move {
            let event_proxy = main_event_proxy;
            let app = main_app;
            let render_handle = render_runtime_handle;
            {
                let mut lock = app.write().map_err(|_| RuntimeError::SyncError)?;

                lock.on_construct();
                lock.on_begin();
            }

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

        let _res = main_result.await??;

        Ok(())
    }
}
