use clap::Parser;
use gltf_viewer::GltfOptions;

mod gltf_viewer;
mod simple_triangle;

#[derive(Clone, clap::Subcommand)]
enum ProgramTarget {
    GltfViewer(GltfOptions),
    SimpleTriangle,
}

#[derive(clap::Parser)]
struct ProgramArgs {
    #[command(subcommand)]
    command: ProgramTarget,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();
    let args = ProgramArgs::try_parse()?;

    match args.command {
        ProgramTarget::GltfViewer(options) => gltf_viewer::run(options)?,
        ProgramTarget::SimpleTriangle => simple_triangle::run()?,
    }

    Ok(())
}
