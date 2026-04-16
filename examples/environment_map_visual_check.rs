use xrds::scene_graph::{
    XrdsSceneAsset, XrdsSceneAssetKind, XrdsSceneEnvironment, XrdsSceneExposureEnvironment,
    XrdsSceneIblEnvironment, XrdsSceneSkyboxEnvironment,
};
use xrds::sdk::{
    primitives::{XrdsPlane3D, XrdsSphere},
    world::{lights::XrdsPointLight, XrdsCamera},
    XrdsClearColorConfig, XrdsColor, XrdsLinearRgba, XrdsMaterialAlphaMode, XrdsMaterialParams,
    XrdsMaterialPbrParams,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const IBL_DIFFUSE_ASSET_ID: &str = "asset:envmap-diffuse";
const IBL_SPECULAR_ASSET_ID: &str = "asset:envmap-specular";
const SKYBOX_ASSET_ID: &str = "asset:envmap-skybox";

#[derive(Default)]
struct EnvironmentMapVisualCheckApp {
    mirror_sphere: Option<Handle<XrdsSphere>>,
    satin_sphere: Option<Handle<XrdsSphere>>,
    matte_sphere: Option<Handle<XrdsSphere>>,
}

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "EnvironmentMapVisualCheck".to_owned(),
        ..Default::default()
    })
    .run_xrds(EnvironmentMapVisualCheckApp::default())
    .expect("failed to run environment_map_visual_check example");
}

impl XrdsApp for EnvironmentMapVisualCheckApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        api.merge_scene_assets(&[
            XrdsSceneAsset {
                id: IBL_DIFFUSE_ASSET_ID.to_string(),
                uri: "environment_maps/diffuse.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: IBL_SPECULAR_ASSET_ID.to_string(),
                uri: "environment_maps/specular.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: SKYBOX_ASSET_ID.to_string(),
                uri: "environment_maps/specular.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
        ]);

        api.set_scene_environment(XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: IBL_DIFFUSE_ASSET_ID.to_string(),
                specular_asset_id: IBL_SPECULAR_ASSET_ID.to_string(),
                intensity: 950.0,
            }),
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: SKYBOX_ASSET_ID.to_string(),
                brightness: 950.0,
            }),
            exposure: Some(XrdsSceneExposureEnvironment { ev100: 6.0 }),
            ..Default::default()
        });

        let camera = api.spawn(
            &XrdsCamera::new()
                .with_name("EnvironmentMapCheckCamera")
                .with_clear_color(XrdsClearColorConfig::Custom(XrdsColor::srgb(
                    0.01, 0.015, 0.02,
                )))
                .at([0.0, 2.4, 8.0])
                .looking_at([0.0, 1.1, 0.0]),
        );
        api.set_camera_perspective(
            &camera,
            xrds::sdk::PerspectiveCameraParams {
                fov_deg: 36.0,
                near: 0.1,
                far: Some(100.0),
                order: 0,
            },
        );

        let light = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("EnvironmentMapCheckLight");
            light.transform.translation = [0.0, 4.5, 2.8];
            light.intensity = 38_000.0;
            light.range = 22.0;
            light.shadows = true;
            light
        });
        api.set_visible(&light, true);

        let floor = api.spawn(&{
            let mut floor = XrdsPlane3D::new().with_name("EnvironmentMapCheckFloor");
            floor.transform.rotation_quat_xyzw = [-0.70710677, 0.0, 0.0, 0.70710677];
            floor.size = [10.0, 10.0];
            floor
        });
        api.set_material_params(
            &floor,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.1, 0.11, 0.13),
                emissive: XrdsLinearRgba::BLACK,
                opacity: 1.0,
                unlit: false,
                pbr: XrdsMaterialPbrParams {
                    metallic: 0.0,
                    roughness: 0.94,
                    reflectance: 0.18,
                    double_sided: true,
                    alpha_mode: XrdsMaterialAlphaMode::Opaque,
                    alpha_cutoff: 0.5,
                },
                textures: Default::default(),
            },
        );

        let mirror_sphere = spawn_sphere(
            api,
            "MirrorSphere",
            [-2.4, 1.0, 0.0],
            [0.95, 0.97, 1.0],
            1.0,
            0.04,
            0.95,
        );
        let satin_sphere = spawn_sphere(
            api,
            "SatinSphere",
            [0.0, 1.0, 0.0],
            [0.95, 0.82, 0.64],
            1.0,
            0.28,
            0.9,
        );
        let matte_sphere = spawn_sphere(
            api,
            "MatteSphere",
            [2.4, 1.0, 0.0],
            [0.74, 0.82, 0.9],
            0.0,
            0.9,
            0.35,
        );

        self.mirror_sphere = Some(mirror_sphere);
        self.satin_sphere = Some(satin_sphere);
        self.matte_sphere = Some(matte_sphere);

        println!("Environment map visual check is running.");
        println!("Expected result: the left sphere should show the sharpest reflections, the center sphere softer reflections, and the right sphere mostly diffuse lighting.");
        println!("If the skybox is visible and the metal spheres respond differently by roughness, the scene environment map path is behaving correctly.");
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let t = ctx.elapsed_secs();
        let mirror = self
            .mirror_sphere
            .as_ref()
            .expect("mirror sphere handle should be initialized during setup");
        let satin = self
            .satin_sphere
            .as_ref()
            .expect("satin sphere handle should be initialized during setup");
        let matte = self
            .matte_sphere
            .as_ref()
            .expect("matte sphere handle should be initialized during setup");

        let mirror_half_yaw = 0.5 * (t * 0.7);
        let satin_half_yaw = 0.5 * (0.5 + t * 0.45);
        let matte_half_yaw = 0.5 * (-0.35 + t * -0.3);

        ctx.set_rotation(
            mirror,
            [0.0, mirror_half_yaw.sin(), 0.0, mirror_half_yaw.cos()],
        );
        ctx.set_rotation(
            satin,
            [0.0, satin_half_yaw.sin(), 0.0, satin_half_yaw.cos()],
        );
        ctx.set_rotation(
            matte,
            [0.0, matte_half_yaw.sin(), 0.0, matte_half_yaw.cos()],
        );
    }
}

fn spawn_sphere(
    api: &mut XrdsAPI<'_>,
    name: &str,
    translation: [f32; 3],
    color: [f32; 3],
    metallic: f32,
    roughness: f32,
    reflectance: f32,
) -> Handle<XrdsSphere> {
    let sphere = api.spawn(&{
        let mut sphere = XrdsSphere::new().with_name(name);
        sphere.transform.translation = translation;
        sphere.radius = 1.0;
        sphere
    });
    api.set_material_params(
        &sphere,
        XrdsMaterialParams {
            base_color: XrdsColor::srgb(color[0], color[1], color[2]),
            emissive: XrdsLinearRgba::BLACK,
            opacity: 1.0,
            unlit: false,
            pbr: XrdsMaterialPbrParams {
                metallic,
                roughness,
                reflectance,
                double_sided: false,
                alpha_mode: XrdsMaterialAlphaMode::Opaque,
                alpha_cutoff: 0.5,
            },
            textures: Default::default(),
        },
    );
    sphere
}
