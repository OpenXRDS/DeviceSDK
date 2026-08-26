use xrds_scene_graph::{
    XrdsSceneFogEnvironment, XrdsSceneExposureEnvironment,
    XrdsSceneIblEnvironment, XrdsSceneSkyboxEnvironment, XrdsXrBlendMode,
};
use crate::bridge::{EditorCommand, EnvironmentDto};
use crate::editor_state::{EditorSession, EditorState};

/// Handle scene environment commands. Returns true if the change needs to be
/// applied to the Bevy runtime (always, since environment is applied every frame).
pub fn apply_environment_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    _state: &mut EditorState,
) -> bool {
    match cmd {
        EditorCommand::SetFog { color, falloff } => {
            // Sanitized rather than refused: this fires on every slider tick, and a
            // drag that momentarily inverts the ramp should keep previewing.
            let falloff: xrds_scene_graph::XrdsSceneFogFalloff = (*falloff).into();
            let fog = XrdsSceneFogEnvironment { color: *color, falloff: falloff.sanitized() };
            let _ = session.0.edit(|doc| {
                doc.metadata.environment.get_or_insert_with(Default::default).fog = Some(fog);
            });
            true
        }
        EditorCommand::ClearFog => {
            let _ = session.0.edit(|doc| {
                if let Some(e) = &mut doc.metadata.environment { e.fog = None; }
                if doc.metadata.environment.as_ref().map(|e| e.is_empty()).unwrap_or(false) {
                    doc.metadata.environment = None;
                }
            });
            true
        }
        EditorCommand::SetXrPassthrough { enabled } => {
            let mode = if *enabled {
                XrdsXrBlendMode::AlphaBlend
            } else {
                XrdsXrBlendMode::Opaque
            };
            let _ = session.0.edit(|doc| doc.metadata.xr_blend_mode = mode);
            true
        }
        EditorCommand::SetExposure { ev100 } => {
            let exp = XrdsSceneExposureEnvironment { ev100: *ev100 };
            let _ = session.0.edit(|doc| {
                doc.metadata.environment.get_or_insert_with(Default::default).exposure = Some(exp);
            });
            true
        }
        EditorCommand::ClearExposure => {
            let _ = session.0.edit(|doc| {
                if let Some(e) = &mut doc.metadata.environment { e.exposure = None; }
                if doc.metadata.environment.as_ref().map(|e| e.is_empty()).unwrap_or(false) {
                    doc.metadata.environment = None;
                }
            });
            true
        }
        EditorCommand::SetColorGrading { temperature, tint, hue_deg, saturation } => {
            let grading = xrds_scene_graph::XrdsSceneColorGradingEnvironment {
                temperature: *temperature,
                tint: *tint,
                hue_deg: *hue_deg,
                saturation: *saturation,
            };
            let _ = session.0.edit(|doc| {
                doc.metadata.environment.get_or_insert_with(Default::default).color_grading =
                    Some(grading);
            });
            true
        }
        EditorCommand::ClearColorGrading => {
            let _ = session.0.edit(|doc| {
                if let Some(e) = &mut doc.metadata.environment { e.color_grading = None; }
                if doc.metadata.environment.as_ref().map(|e| e.is_empty()).unwrap_or(false) {
                    doc.metadata.environment = None;
                }
            });
            true
        }
        EditorCommand::SetAntiAliasing { msaa } => {
            use xrds_scene_graph::XrdsSceneMsaa;
            let samples = match msaa.as_str() {
                "Off" => XrdsSceneMsaa::Off,
                "2x" => XrdsSceneMsaa::Sample2,
                "8x" => XrdsSceneMsaa::Sample8,
                // Anything unrecognised lands on the default rather than panicking
                // or silently disabling AA — a stale frontend must not degrade the
                // image.
                _ => XrdsSceneMsaa::Sample4,
            };
            let aa = xrds_scene_graph::XrdsSceneAntiAliasingEnvironment { msaa: samples };
            let _ = session.0.edit(|doc| {
                doc.metadata.environment.get_or_insert_with(Default::default).anti_aliasing =
                    Some(aa);
            });
            true
        }
        EditorCommand::ClearAntiAliasing => {
            let _ = session.0.edit(|doc| {
                if let Some(e) = &mut doc.metadata.environment { e.anti_aliasing = None; }
                if doc.metadata.environment.as_ref().map(|e| e.is_empty()).unwrap_or(false) {
                    doc.metadata.environment = None;
                }
            });
            true
        }
        EditorCommand::SetIbl { diffuse_asset_id, specular_asset_id, intensity } => {
            let ibl = XrdsSceneIblEnvironment {
                diffuse_asset_id:  diffuse_asset_id.clone(),
                specular_asset_id: specular_asset_id.clone(),
                intensity: *intensity,
            };
            let _ = session.0.edit(|doc| {
                doc.metadata.environment.get_or_insert_with(Default::default).ibl = Some(ibl);
            });
            true
        }
        EditorCommand::ClearIbl => {
            let _ = session.0.edit(|doc| {
                if let Some(e) = &mut doc.metadata.environment { e.ibl = None; }
            });
            true
        }
        EditorCommand::SetSkybox { texture_asset_id, brightness, yaw_deg } => {
            let sky = XrdsSceneSkyboxEnvironment {
                texture_asset_id: texture_asset_id.clone(),
                brightness: *brightness,
                yaw_deg: *yaw_deg,
            };
            let _ = session.0.edit(|doc| {
                doc.metadata.environment.get_or_insert_with(Default::default).skybox = Some(sky);
            });
            true
        }
        EditorCommand::SetAtmosphere { enabled } => {
            let on = *enabled;
            let _ = session.0.edit(|doc| {
                let env = doc.metadata.environment.get_or_insert_with(Default::default);
                env.atmosphere = on.then(xrds_scene_graph::XrdsSceneAtmosphereEnvironment::default);
                if env.is_empty() { doc.metadata.environment = None; }
            });
            true
        }
        EditorCommand::ClearSkybox => {
            let _ = session.0.edit(|doc| {
                if let Some(e) = &mut doc.metadata.environment { e.skybox = None; }
            });
            true
        }
        _ => false,
    }
}

/// Build an `EnvironmentDto` from the scene document's metadata.
/// Whether the scene is authored for passthrough.
///
/// Separate from [`build_environment_dto`] because `xr_blend_mode` sits on
/// `metadata` directly rather than inside `metadata.environment`, and a scene can
/// want passthrough without having any environment at all.
pub fn build_xr_passthrough(session: &EditorSession) -> bool {
    matches!(
        session.0.document().metadata.xr_blend_mode,
        XrdsXrBlendMode::AlphaBlend
    )
}

pub fn build_environment_dto(session: &EditorSession) -> Option<EnvironmentDto> {
    let env = session.0.document().metadata.environment.as_ref()?;
    let fog = env.fog.as_ref();
    let exp = env.exposure.as_ref();
    let ibl = env.ibl.as_ref();
    let sky = env.skybox.as_ref();
    let grade = env.color_grading.as_ref();
    let aa = env.anti_aliasing.as_ref();
    Some(EnvironmentDto {
        fog_enabled:  fog.is_some(),
        fog_color:    fog.map(|f| f.color).unwrap_or([0.5, 0.6, 0.7, 1.0]),
        fog_falloff:  fog.map(|f| f.falloff.into()).unwrap_or_default(),
        exposure_enabled: exp.is_some(),
        ev100:        exp.map(|e| e.ev100).unwrap_or(0.0),
        color_grading_enabled: grade.is_some(),
        grading_temperature: grade.map(|g| g.temperature).unwrap_or(0.0),
        grading_tint:        grade.map(|g| g.tint).unwrap_or(0.0),
        grading_hue_deg:     grade.map(|g| g.hue_deg).unwrap_or(0.0),
        // 1.0, not 0.0: saturation is a multiplier, and defaulting a disabled
        // control to zero would show the author a greyscale slider position for a
        // scene that is not greyscale.
        grading_saturation:  grade.map(|g| g.saturation).unwrap_or(1.0),
        anti_aliasing_enabled: aa.is_some(),
        msaa: match aa.map(|a| a.msaa).unwrap_or_default() {
            xrds_scene_graph::XrdsSceneMsaa::Off => "Off",
            xrds_scene_graph::XrdsSceneMsaa::Sample2 => "2x",
            xrds_scene_graph::XrdsSceneMsaa::Sample4 => "4x",
            xrds_scene_graph::XrdsSceneMsaa::Sample8 => "8x",
        }.to_string(),
        ibl_enabled:  ibl.is_some(),
        ibl_diffuse:  ibl.map(|i| i.diffuse_asset_id.clone()).unwrap_or_default(),
        ibl_specular: ibl.map(|i| i.specular_asset_id.clone()).unwrap_or_default(),
        // cd/m2, like skybox brightness — not a 0..1 factor. Defaulting to 1.0 would
        // light the scene with essentially nothing, the same trap the skybox had.
        ibl_intensity:ibl.map(|i| i.intensity).unwrap_or(1000.0),
        skybox_enabled:    sky.is_some(),
        skybox_asset:      sky.map(|s| s.texture_asset_id.clone()).unwrap_or_default(),
        // 1000 cd/m2, not 1.0: brightness is an absolute luminance measured against
        // the camera exposure (default ev100 9.7, outdoor daylight), so a value of 1
        // renders as black. This is the value the UI offers when enabling a skybox
        // for the first time, so it decides whether the feature appears to work.
        skybox_brightness: sky.map(|s| s.brightness).unwrap_or(1000.0),
        skybox_yaw_deg:    sky.map(|s| s.yaw_deg).unwrap_or(0.0),
        atmosphere_enabled: env.atmosphere.is_some(),
    })
}
