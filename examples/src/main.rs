use clap::Parser;
use gltf_viewer::GltfViewerOptions;

mod gltf_viewer;
mod simple_triangle;

#[derive(clap::Subcommand)]
enum ProgramTarget {
    GltfViewer(GltfViewerOptions),
    SimpleTriangle,
}

#[derive(clap::Parser)]
struct ProgramArgs {
    #[command(subcommand)]
    command: ProgramTarget,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter(Some("xrds_"), log::LevelFilter::Debug)
        // .filter_level(log::LevelFilter::Trace)
        .init();
    let args = ProgramArgs::try_parse()?;

    match args.command {
        ProgramTarget::GltfViewer(options) => gltf_viewer::run(options)?,
        ProgramTarget::SimpleTriangle => simple_triangle::run()?,
    }

    Ok(())
}
