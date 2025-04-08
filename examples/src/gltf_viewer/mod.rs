use std::{f32::consts::PI, path::PathBuf, time::Duration};

use rand::Rng;
use xrds::{
    core::{core::Transform, graphics::ObjectInstance},
    RuntimeHandler,
};

mod program_args;
pub use program_args::*;

#[derive(Default)]
struct App {
    gltf_path: String,
    spawned: Option<ObjectInstance>,
}

impl RuntimeHandler for App {
    fn on_construct(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_begin(&mut self, context: &mut xrds::Context) -> anyhow::Result<()> {
        let objects = context.load_objects_from_gltf(self.gltf_path.as_str())?;
        let world = context.get_current_world_mut();

        let transform = Transform::default()
            .with_translation(glam::vec3(0.0, 0.0, 0.0))
            .with_scale(glam::vec3(1.0, 1.0, 1.0));
        self.spawned = Some(world.spawn(&objects[0], &transform)?);

        // let rng = rand::rng;
        // let uniform = rand::distr::Uniform::new(0.0f32, 1.0f32)?;
        // for _ in 0..100 {
        //     let distance = rng().sample(uniform) * 10.0;
        //     let angle = rng().sample(uniform) * PI * 2.0;
        //     let tx = distance * angle.cos();
        //     let tz = distance * angle.sin();
        //     let s = rng().sample(uniform) + 0.5;
        //     let ry = rng().sample(uniform) * PI * 2.0;

        //     let transform = Transform::default()
        //         .with_translation(glam::vec3(tx, 0.0, tz))
        //         .with_scale(glam::vec3(s, s, s))
        //         .with_rotation(glam::Quat::from_rotation_y(ry));
        //     world.spawn(&objects[0], &transform)?;
        // }
        Ok(())
    }

    fn on_resumed(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_update(&mut self, context: &mut xrds::Context, diff: Duration) -> anyhow::Result<()> {
        let world = context.get_current_world();
        if let Some(spawned) = &mut self.spawned {
            // spawned.transform_mut().rotate(Quat::from_rotation_y(
            //     60.0f32.to_radians() * diff.as_secs_f32(),
            // ));
            // world.update_object(spawned);
        }
        Ok(())
    }

    fn on_suspended(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_end(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_deconstruct(&mut self, _context: &mut xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn run(options: GltfViewerOptions) -> anyhow::Result<()> {
    let target = if options.xr.enable_xr {
        if options.window.enable_window {
            xrds::RuntimeTarget::XrWithPreview
        } else {
            xrds::RuntimeTarget::Xr
        }
    } else if options.window.enable_window {
        xrds::RuntimeTarget::Window
    } else {
        anyhow::bail!("Both xr and window disabled. At least one must be enabled")
    };
    let mut runtime_builder = xrds::Runtime::builder()
        .with_application_name("gltf_viewer")
        .with_target(target);

    match target {
        xrds::RuntimeTarget::Window | xrds::RuntimeTarget::XrWithPreview => {
            runtime_builder = runtime_builder.with_window_options(xrds::RuntimeWindowOptions {
                width: options.window.width,
                height: options.window.height,
                resizable: options.window.resizable,
                title: "gitf_viewer".to_owned(),
                ..Default::default()
            })
        }
        _ => {}
    }
    let runtime = runtime_builder.build()?;

    let path = match options.mode {
        RenderMode::Single(options) => PathBuf::from(options.path),
        RenderMode::Multi(options) => PathBuf::from(options.path),
    };

    let gltf_file = if path.is_file() {
        if let Some(extension) = path.extension() {
            let extension_str = extension.to_string_lossy();
            if extension_str != "gltf" && extension_str != "glb" {
                anyhow::bail!("Invalid file extension: {}", extension_str);
            }
        } else {
            anyhow::bail!("Invalid file extension");
        }
        path
    } else {
        let gltf_files: Vec<_> = path
            .read_dir()?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let path = e.path();
                if let Some(extension) = path.extension() {
                    let extension_str = extension.to_string_lossy();
                    if extension_str == "gltf" || extension_str == "glb" {
                        Some(path)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        if gltf_files.is_empty() {
            anyhow::bail!("No gltf file found");
        }

        // viewer support only 1 gltf or glb file so we use first gltf file
        gltf_files[0].clone()
    };

    let app = App {
        gltf_path: gltf_file.to_string_lossy().to_string(),
        ..Default::default()
    };

    runtime.run(app)?;

    Ok(())
}
