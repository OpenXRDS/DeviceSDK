use crate::*;

use std::sync::{Arc, Mutex};

use error::RuntimeError;
use xrds_graphics::Renderer;

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
    main_context: tokio::runtime::Runtime,
    render_context: tokio::runtime::Runtime,
    renderer: Arc<Mutex<Renderer>>,
}

pub struct RuntimeParameters {
    pub app_name: String,
}

impl Runtime {
    pub fn new(params: RuntimeParameters) -> Self {
        let main_context = tokio::runtime::Builder::new_current_thread()
            .thread_name(format!("{}-main", params.app_name))
            .build()
            .expect("Could not create main runtime");
        let render_context = tokio::runtime::Builder::new_multi_thread()
            .thread_name(format!("{}-render", params.app_name))
            .build()
            .expect("Could not create render runtime");
        Self {
            main_context,
            render_context,
            renderer: Arc::new(Mutex::new(Renderer {})),
        }
    }

    pub fn run<A>(self, mut app: A) -> Result<(), RuntimeError>
    where
        A: RuntimeHandler + Send + Sync,
    {
        // let main_context = self.main_context;
        // let render_context = self.render_context;
        // let renderer = self.renderer.clone();
        app.on_begin();
        // let _main_context_future: JoinHandle<anyhow::Result<()>> = main_context.spawn(async move {
        //     {
        //         let mut lock = renderer.lock().unwrap();
        //         let scene = lock.load_scene()?;
        //         // graphics.load_scene();
        //     }
        //     // // Initialize OpenXR

        //     // // Initialize window (Optional)

        //     // // Initialize graphics

        //     // // Initialize user application
        //     app.on_construct()?;

        //     // // Begin OpenXR session

        //     // // Begin user application
        //     // app.on_begin()?;

        //     // // Call on_resumed() on first iteration
        //     // app.on_resumed()?;

        //     // If system support winit. Use event_loop instead loop{}
        //     loop {
        //         app.on_update().unwrap();
        //         break;
        //     }

        //     // // Suspend app first
        //     // self.app.on_suspended()?;

        //     // self.app.on_end()?;

        //     // self.app.on_deconstruct()?;
        //     Ok(())
        // });

        // let renderer = self.renderer.clone();
        // let _render_context_future = render_context.spawn(async move {
        //     let lock = renderer.lock();
        // });

        // // Start render thread

        Ok(())
    }
}
