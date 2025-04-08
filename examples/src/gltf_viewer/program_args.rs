#[derive(clap::Args)]
pub struct GltfViewerOptions {
    #[command(subcommand)]
    pub mode: RenderMode,
    #[command(flatten)]
    pub xr: XrOptions,
    #[command(flatten)]
    pub window: WindowOptions,
}

#[derive(clap::Args)]
pub struct XrOptions {
    /// Enable openxr
    #[arg(long, default_value_t = false)]
    pub enable_xr: bool,
}

#[derive(clap::Args)]
pub struct WindowOptions {
    /// Enable viewer window
    #[arg(long, default_value_t = false)]
    pub enable_window: bool,
    /// Set width of viewer window
    #[arg(long, default_value_t = 1280, requires = "enable_window")]
    pub width: u32,
    /// Set height of viewer window
    #[arg(long, default_value_t = 720, requires = "enable_window")]
    pub height: u32,
    /// Make viewer window resizable
    #[arg(long, default_value_t = false, requires = "enable_window")]
    pub resizable: bool,
}

#[derive(clap::Subcommand)]
pub enum RenderMode {
    Single(SingleRenderOptions),
    Multi(MultiRenderOptions),
}

#[derive(clap::Args)]
pub struct SingleRenderOptions {
    /// Path of .gltf or .glb file
    pub path: String,
    #[arg(long, default_value_t = 1.0)]
    pub scale: f32,
    #[command(flatten)]
    pub position: PositionOption,
    #[arg(long, default_value_t = 0.0)]
    /// rotation speed in degrees/min
    pub rotation_speed: f32,
}

#[derive(clap::Args)]
pub struct MultiRenderOptions {
    /// Path of .gltf or .glb files. Comma separated
    pub path: String,
    pub dist_min: f32,
    pub dist_max: f32,
    pub scale_min: f32,
    pub scale_max: f32,
}

#[derive(clap::Args)]
#[group(required = false, multiple = false)]
pub struct PositionOption {
    /// gltf origin in worldspace
    #[arg(long, default_value = "(0.0, 0.0, 0.0)")]
    position: Option<String>,
    #[arg(long)]
    pos_x: Option<f32>,
    #[arg(long)]
    pos_y: Option<f32>,
    #[arg(long)]
    pos_z: Option<f32>,
}
