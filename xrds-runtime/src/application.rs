use std::{sync::Arc, time::Duration};

use log::debug;
use uuid::Uuid;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event_loop,
    window::{Window, WindowAttributes},
};
use xrds_graphics::{GraphicsApi, Surface};
use xrds_openxr::{FormFactor, HmdEntity, OpenXrContext, OpenXrOnPreRenderResult};

use crate::{Context, RuntimeError, RuntimeHandler, WorldEvent, WorldOnCameraUpdated};

#[derive(Debug, Clone, Copy)]
pub struct PreviewWindowAttributes {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RuntimeEvent {
    RedrawRequested,
    Tick(Duration),
    Closed,
}

pub(crate) struct RuntimeApplication<'window, A>
where
    A: RuntimeHandler + Send + Sync + 'static,
{
    preview_window_attr: Option<PreviewWindowAttributes>,
    preview_window: Option<PreviewWindow<'window>>,
    openxr_context: Option<OpenXrContext>,
    xrds_context: Option<Context>,
    primary_camera_id: Option<Uuid>,
    app: A,
}

#[derive(Debug)]
pub(crate) struct PreviewWindow<'window> {
    window: Arc<Window>,
    surface: Surface<'window>,
}

impl<'w, A> RuntimeApplication<'w, A>
where
    A: RuntimeHandler + Send + Sync + 'static,
{
    pub fn new(mut app: A) -> anyhow::Result<Self> {
        app.on_construct()?;
        Ok(Self {
            app,
            preview_window_attr: None,
            preview_window: None,
            openxr_context: None,
            xrds_context: None,
            primary_camera_id: None,
        })
    }

    fn on_resumed(&mut self, event_loop: &event_loop::ActiveEventLoop) -> anyhow::Result<()> {
        debug!("Application on resumed");
        // Select graphics api
        // Currently vulkan is the only supported api
        let graphics_api = GraphicsApi::Vulkan;

        // Initialize xr. Make it optional
        let openxr_context = xrds_openxr::OpenXrContextBuilder::default()
            .with_application_name("XRDS runtime application")
            .with_form_factor(FormFactor::HeadMountedDisplay)
            .with_graphics_api(graphics_api)
            .build()?;

        let graphics_instance = openxr_context.graphics_instance().clone();

        let mut xrds_context = Context::new(graphics_instance.clone())?;
        // Initialize OpenXr Camera
        let primary_camera_id = {
            let mut asset_server = xrds_context.get_asset_server().write().unwrap();
            HmdEntity::build(&mut asset_server)?
        };
        {
            let extent = openxr_context.swapchain_extent()?;
            let format = openxr_context.swapchain_format()?;
            let world = xrds_context.get_current_world_mut();
            let camera_ids = world.spawn_camera(&primary_camera_id, Some(extent), Some(format))?;
            self.primary_camera_id = Some(camera_ids[0]); // we spawned one camera for HMD
        }

        // Initialize preview/debug window
        if let Some(pwa) = self.preview_window_attr {
            let window = Arc::new(
                event_loop.create_window(
                    WindowAttributes::default()
                        .with_inner_size(LogicalSize::new(pwa.width, pwa.height)),
                )?,
            );

            let surface = graphics_instance
                .instance()
                .create_surface(window.clone())?;

            self.preview_window = Some(PreviewWindow {
                window,
                surface: Surface::new(surface),
            });

            // make camera and spawn to world
        }

        self.app.on_begin(&mut xrds_context)?;
        self.app.on_resumed(&mut xrds_context)?;

        self.openxr_context = Some(openxr_context);
        self.xrds_context = Some(xrds_context);

        Ok(())
    }

    fn on_window_event(
        &mut self,
        _event_loop: &event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) -> anyhow::Result<()> {
        match _event {
            winit::event::WindowEvent::Resized(_s) => {}
            winit::event::WindowEvent::CloseRequested => {}
            winit::event::WindowEvent::RedrawRequested => {}
            _ => {}
        }
        Ok(())
    }

    fn on_tick(&mut self, diff: Duration) -> anyhow::Result<()> {
        if let Some(xrds_context) = &mut self.xrds_context {
            let world = xrds_context.get_current_world_mut();
            world.on_update(diff)?;

            self.app.on_update(xrds_context, diff)?;
        }
        Ok(())
    }

    fn on_redraw(&mut self, event_loop: &event_loop::ActiveEventLoop) -> anyhow::Result<()> {
        let openxr_context = self
            .openxr_context
            .as_mut()
            .ok_or(RuntimeError::OpenXrNotInitialized)?;

        let xr_render_params = match openxr_context.on_pre_render()? {
            OpenXrOnPreRenderResult::DoRender(t) => t,
            OpenXrOnPreRenderResult::SkipRender => return Ok(()),
            OpenXrOnPreRenderResult::Exit => {
                event_loop.exit();
                return Ok(());
            }
        };

        let xrds_context = self.xrds_context.as_mut().unwrap();
        let world = xrds_context.get_current_world_mut();

        if let Some(primary_camera_id) = &self.primary_camera_id {
            world.emit_event(WorldEvent::OnCameraUpdated(WorldOnCameraUpdated {
                camera_id: primary_camera_id,
                params: &xr_render_params,
            }))?;

            world.on_pre_render()?;
            world.on_render()?;
            world.on_post_render()?;
        }

        openxr_context.on_post_render()?;

        Ok(())
    }
}

impl<'window, A> ApplicationHandler<RuntimeEvent> for RuntimeApplication<'window, A>
where
    A: RuntimeHandler + Send + Sync + 'static,
    RuntimeEvent: 'static,
{
    fn resumed(&mut self, event_loop: &event_loop::ActiveEventLoop) {
        self.on_resumed(event_loop)
            .expect("Could not resume application");
    }

    fn window_event(
        &mut self,
        event_loop: &event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        self.on_window_event(event_loop, window_id, event)
            .expect("Could not handle window event");
    }

    fn user_event(&mut self, event_loop: &event_loop::ActiveEventLoop, event: RuntimeEvent) {
        match event {
            RuntimeEvent::RedrawRequested => {
                self.on_redraw(event_loop)
                    .expect("Something went wrong during redraw");
            }
            RuntimeEvent::Tick(d) => {
                self.on_tick(d).expect("Something went wrong during tick");
            }
            RuntimeEvent::Closed => {
                event_loop.exit();
                debug!("Closed");
            }
        }
    }
}
