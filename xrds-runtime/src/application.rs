use std::{sync::Arc, time::Duration};

use log::debug;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event_loop,
    window::{Window, WindowAttributes},
};
use xrds_graphics::{GraphicsApi, Renderer, Surface};
use xrds_openxr::{FormFactor, OpenXrContext, OpenXrOnPreRenderResult};

use crate::{Context, RuntimeError, RuntimeHandler};

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
    renderer: Option<Renderer>,
    xrds_context: Option<Context>,
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
            renderer: None,
        })
    }

    fn on_resumed(&mut self, event_loop: &event_loop::ActiveEventLoop) -> anyhow::Result<()> {
        debug!("Application on resumed");
        // Select graphics api
        // Currently vulkan is the only supported api
        let graphics_api = GraphicsApi::Vulkan;

        // Initialize xr
        let openxr_context = xrds_openxr::OpenXrContextBuilder::default()
            .with_application_name("XRDS runtime application")
            .with_form_factor(FormFactor::HeadMountedDisplay)
            .with_graphics_api(graphics_api)
            .build()?;

        let graphics_instance = openxr_context.graphics_instance().clone();
        let renderer = xrds_graphics::Renderer::new(
            openxr_context.graphics_instance().clone(),
            openxr_context.swapchain_format()?,
            openxr_context.swapchain_extent()?,
            1,
        )?;
        let xrds_context = Context::new(graphics_instance.clone())?;

        self.openxr_context = Some(openxr_context);
        self.renderer = Some(renderer);
        self.xrds_context = Some(xrds_context.clone());

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
        }

        self.app.on_begin(xrds_context.clone())?;
        self.app.on_resumed(xrds_context.clone())?;

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
            let mut world = xrds_context.get_current_world();
            world.update(diff)?;
        }
        Ok(())
    }

    fn on_redraw(&mut self, event_loop: &event_loop::ActiveEventLoop) -> anyhow::Result<()> {
        let openxr_context = self
            .openxr_context
            .as_mut()
            .ok_or(RuntimeError::OpenXrNotInitialized)?;

        let renderer = self
            .renderer
            .as_mut()
            .ok_or(RuntimeError::RendererNotInitialized)?;

        let xr_swapchain_texture = match openxr_context.on_pre_render()? {
            OpenXrOnPreRenderResult::DoRender(t) => t,
            OpenXrOnPreRenderResult::SkipRender => return Ok(()),
            OpenXrOnPreRenderResult::Exit => {
                event_loop.exit();
                return Ok(());
            }
        };

        let xrds_context = self.xrds_context.as_ref().unwrap();
        let world = xrds_context.get_current_world();
        world.update_instances()?;
        let cam_binding = openxr_context.get_cam_binding();

        let mut command_encoder = renderer.create_command_encoder()?;
        // Encode to g-buffers
        {
            let mut gbuffer_pass = renderer.create_gbuffer_pass(&mut command_encoder)?;
            cam_binding.encode(&mut gbuffer_pass);
            world.encode(&mut gbuffer_pass)?;
        }
        {
            let mut lighting_pass = renderer.create_lighting_pass(&mut command_encoder)?;
            cam_binding.encode(&mut lighting_pass);
            renderer.do_deferred_lighting(&mut lighting_pass)?;
        }

        renderer.copy_render_result(&mut command_encoder, &xr_swapchain_texture)?;
        renderer.summit(command_encoder)?;

        renderer.on_post_render()?;
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
