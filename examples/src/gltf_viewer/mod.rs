use xrds::RuntimeHandler;

#[derive(Clone, clap::Args)]
pub struct GltfOptions {
    /// Enable openxr rendering
    #[arg(long, default_value_t = false)]
    pub enable_xr: bool,
    /// Disable window rendering
    #[arg(long, default_value_t = false)]
    pub disable_window: bool,
    /// Viewer window width
    #[arg(long, default_value_t = 1280)]
    pub width: u32,
    /// Viewer window height
    #[arg(long, default_value_t = 720)]
    pub height: u32,
    /// Make viewer window resizable
    #[arg(long, default_value_t = false)]
    pub resizable: bool,
    /// Path of .gltf or .glb file
    pub path: String,
}

#[derive(Default)]
struct App {
    gltf_path: String,
    objects: Vec<xrds::Object>,
}

impl RuntimeHandler for App {
    fn on_construct(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_begin(&mut self, context: xrds::Context) -> anyhow::Result<()> {
        self.objects = context.load_objects_from_gltf(self.gltf_path.as_str())?;

        let world = context.get_current_world()?;
        world.spawn(&self.objects)?;
        Ok(())
    }

    fn on_resumed(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_update(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_suspended(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_end(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_deconstruct(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn run(options: GltfOptions) -> anyhow::Result<()> {
    let target = if options.enable_xr {
        if options.disable_window {
            xrds::RuntimeTarget::Xr
        } else {
            xrds::RuntimeTarget::XrWithPreview
        }
    } else {
        if options.disable_window {
            anyhow::bail!("Both xr and window disabled. At least one must be enabled")
        } else {
            xrds::RuntimeTarget::Window
        }
    };
    let mut runtime_builder = xrds::Runtime::builder()
        .with_application_name("gltf_viewer")
        .with_target(target);

    match target {
        xrds::RuntimeTarget::Window | xrds::RuntimeTarget::XrWithPreview => {
            runtime_builder = runtime_builder.with_window_options(xrds::RuntimeWindowOptions {
                width: options.width,
                height: options.height,
                resizable: options.resizable,
                title: "gitf_viewer".to_owned(),
                ..Default::default()
            })
        }
        _ => {}
    }
    let runtime = runtime_builder.build()?;

    let app = App {
        gltf_path: options.path,
        ..Default::default()
    };

    runtime.run(app)?;

    Ok(())
}
