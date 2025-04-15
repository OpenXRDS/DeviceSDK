use std::{f32::consts::PI, option, path::PathBuf, time::Duration};

use glam::{vec3, Quat, Vec3};
use rand::Rng;
use xrds::{
    core::{
        core::{Transform, ViewDirection},
        graphics::{
            LightColor, LightDescription, LightType, ObjectInstance, PointLightDescription,
        },
    },
    RuntimeHandler,
};

mod program_args;
pub use program_args::*;

#[derive(Default)]
struct App {
    gltf_path: String,
    spawned: Option<ObjectInstance>,
    directional_light: Option<ObjectInstance>,
    point_lights: Vec<ObjectInstance>,
    options: GltfViewerOptions,
}

impl RuntimeHandler for App {
    fn on_construct(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_begin(&mut self, context: &mut xrds::Context) -> anyhow::Result<()> {
        let rng = rand::rng;

        // Object creation
        let objects = context.load_objects_from_gltf(self.gltf_path.as_str())?;
        let directional_light_entity_id = context.create_light(
            &LightDescription {
                color: LightColor::DIRECT_SUNLIGHT,
                intensity: 10.0,
                ty: LightType::Directional,
                cast_shadow: true,
            },
            Some("directional light"),
        )?;
        let colors = [
            vec3(1.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, 1.0),
        ];
        let point_lights: Vec<_> = colors
            .iter()
            .map(|c| {
                context
                    .create_light(
                        &LightDescription {
                            color: *c,
                            intensity: 2.0,
                            ty: LightType::Point(PointLightDescription { range: 10.0 }),
                            cast_shadow: false,
                        },
                        None,
                    )
                    .unwrap()
            })
            .collect();

        let world = context.get_current_world_mut();

        self.directional_light = Some(world.spawn_light(
            &directional_light_entity_id,
            &ViewDirection::default().with_direction(vec3(0.0, -1.0, 0.5).normalize()),
        )?);
        let uniform = rand::distr::Uniform::new(0.0f32, 1.0f32)?;
        // self.point_lights = point_lights
        //     .iter()
        //     .map(|point_light_id| {
        //         let distance = rng().sample(uniform) * 5.0;
        //         let angle = rng().sample(uniform) * PI * 2.0;
        //         let tx = distance * angle.cos();
        //         let tz = distance * angle.sin();
        //         let ty = rng().sample(uniform) * 4.0;
        //         let transform = Transform::default().with_translation(glam::vec3(tx, ty, tz));
        //         world.spawn_light(point_light_id, &transform).unwrap()
        //     })
        //     .collect();
        match &self.options.mode {
            RenderMode::Multi(options) => {
                for _ in 0..100 {
                    let distance = (rng().sample(uniform) * options.dist_max - options.dist_min)
                        .max(0.0)
                        + options.dist_min;
                    let angle = rng().sample(uniform) * PI * 2.0;
                    let tx = distance * angle.cos();
                    let tz = distance * angle.sin();
                    let s = (rng().sample(uniform) + options.scale_max - options.scale_min)
                        .max(0.0)
                        + options.scale_min;
                    let ry = rng().sample(uniform) * PI * 2.0;

                    let transform = Transform::default()
                        .with_translation(glam::vec3(tx, 0.0, tz))
                        .with_scale(glam::vec3(s, s, s))
                        .with_rotation(glam::Quat::from_rotation_y(ry));
                    world.spawn(&objects[0], &transform)?;
                }
            }
            RenderMode::Single(options) => {
                let transform = Transform::default()
                    .with_translation(glam::vec3(
                        options.position.pos_x,
                        options.position.pos_y,
                        options.position.pos_z,
                    ))
                    .with_rotation(Quat::from_rotation_y(-90.0f32.to_radians()))
                    .with_scale(glam::vec3(options.scale, options.scale, options.scale));
                self.spawned = Some(world.spawn(&objects[0], &transform)?);
            }
        }
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

    let path = match &options.mode {
        RenderMode::Single(options) => PathBuf::from(&options.path),
        RenderMode::Multi(options) => PathBuf::from(&options.path),
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
    let app = App {
        gltf_path: gltf_file.to_string_lossy().to_string(),
        options,
        ..Default::default()
    };

    runtime.run(app)?;

    Ok(())
}
