use super::*;
use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use xrds_components::{
    XrdsMaterialTextureFilterMode, XrdsMaterialTextureSamplerParams, XrdsMaterialTextureSlotKind,
    XrdsMaterialTextureSlots, XrdsMaterialTextureUvParams, XrdsMaterialTextureWrapMode,
};

const XRDS_RUNTIME_MATERIAL_SHADER_PATH: &str =
    "embedded://xrds_runtime/xrds_api/shaders/xrds_runtime_material_extension.wgsl";
const XRDS_RUNTIME_MATERIAL_PREPASS_SHADER_PATH: &str =
    "embedded://xrds_runtime/xrds_api/shaders/xrds_runtime_material_prepass.wgsl";
const XRDS_TEXTURE_FLAG_BASE_COLOR: u32 = 1 << 0;
const XRDS_TEXTURE_FLAG_METALLIC_ROUGHNESS: u32 = 1 << 1;
const XRDS_TEXTURE_FLAG_NORMAL: u32 = 1 << 2;
const XRDS_TEXTURE_FLAG_OCCLUSION: u32 = 1 << 3;
const XRDS_TEXTURE_FLAG_EMISSIVE: u32 = 1 << 4;

pub(super) type XrdsRuntimeMaterial =
    ExtendedMaterial<StandardMaterial, XrdsRuntimeMaterialExtension>;

#[derive(Asset, AsBindGroup, Debug, Clone, Default, TypePath)]
pub(super) struct XrdsRuntimeMaterialExtension {
    #[uniform(100)]
    pub(super) material_uniform: XrdsRuntimeMaterialExtensionUniform,
    // All five textures share binding 102 as their sampler to stay within
    // Metal's hard per-stage limit of 16 samplers (view=6, StandardMaterial=6, here=1 → 13 total).
    #[texture(101)]
    #[sampler(102)]
    #[dependency]
    pub(super) base_color_texture: Option<bevy::asset::Handle<Image>>,
    #[texture(103)]
    #[dependency]
    pub(super) metallic_roughness_texture: Option<bevy::asset::Handle<Image>>,
    #[texture(104)]
    #[dependency]
    pub(super) normal_texture: Option<bevy::asset::Handle<Image>>,
    #[texture(105)]
    #[dependency]
    pub(super) occlusion_texture: Option<bevy::asset::Handle<Image>>,
    #[texture(106)]
    #[dependency]
    pub(super) emissive_texture: Option<bevy::asset::Handle<Image>>,
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub(super) struct XrdsRuntimeMaterialExtensionUniform {
    pub(super) flags: u32,
    pub(super) _padding: UVec3,
    pub(super) base_color: XrdsRuntimeTextureSlotUniform,
    pub(super) metallic_roughness: XrdsRuntimeTextureSlotUniform,
    pub(super) normal: XrdsRuntimeTextureSlotUniform,
    pub(super) occlusion: XrdsRuntimeTextureSlotUniform,
    pub(super) emissive: XrdsRuntimeTextureSlotUniform,
}

impl Default for XrdsRuntimeMaterialExtensionUniform {
    fn default() -> Self {
        Self {
            flags: 0,
            _padding: UVec3::ZERO,
            base_color: XrdsRuntimeTextureSlotUniform::default(),
            metallic_roughness: XrdsRuntimeTextureSlotUniform::default(),
            normal: XrdsRuntimeTextureSlotUniform::default(),
            occlusion: XrdsRuntimeTextureSlotUniform::default(),
            emissive: XrdsRuntimeTextureSlotUniform::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub(super) struct XrdsRuntimeTextureSlotUniform {
    pub(super) uv_transform: Mat3,
    pub(super) uv_set: u32,
    pub(super) _padding: UVec3,
}

impl Default for XrdsRuntimeTextureSlotUniform {
    fn default() -> Self {
        Self {
            uv_transform: Mat3::IDENTITY,
            uv_set: 0,
            _padding: UVec3::ZERO,
        }
    }
}

impl MaterialExtension for XrdsRuntimeMaterialExtension {
    fn fragment_shader() -> ShaderRef {
        XRDS_RUNTIME_MATERIAL_SHADER_PATH.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        XRDS_RUNTIME_MATERIAL_PREPASS_SHADER_PATH.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        XRDS_RUNTIME_MATERIAL_SHADER_PATH.into()
    }
}

pub(super) fn runtime_material_from_authored_in_world(
    world: Option<&World>,
    params: XrdsMaterialParams,
) -> XrdsRuntimeMaterial {
    let mut base_color = params.base_color;
    base_color.rgba[3] *= params.opacity.clamp(0.0, 1.0);
    let alpha = base_color.rgba[3];
    let alpha_mode = match params.pbr.alpha_mode {
        XrdsMaterialAlphaMode::Auto => {
            if alpha < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            }
        }
        XrdsMaterialAlphaMode::Opaque => AlphaMode::Opaque,
        XrdsMaterialAlphaMode::Mask => AlphaMode::Mask(params.pbr.alpha_cutoff.clamp(0.0, 1.0)),
        XrdsMaterialAlphaMode::Blend => AlphaMode::Blend,
    };

    let textures = params.textures.clone();

    XrdsRuntimeMaterial {
        base: StandardMaterial {
            base_color: base_color.into(),
            emissive: params.emissive.into(),
            perceptual_roughness: params.pbr.roughness.clamp(0.0, 1.0),
            metallic: params.pbr.metallic.clamp(0.0, 1.0),
            reflectance: params.pbr.reflectance.clamp(0.0, 1.0),
            double_sided: params.pbr.double_sided,
            alpha_mode,
            unlit: params.unlit,
            ..default()
        },
        extension: runtime_material_extension_from_authored(world, &textures),
    }
}

fn runtime_material_extension_from_authored(
    world: Option<&World>,
    textures: &XrdsMaterialTextureSlots,
) -> XrdsRuntimeMaterialExtension {
    let base_color_texture = resolved_texture_handle_for_material_slot(
        world,
        XrdsMaterialTextureSlotKind::BaseColor,
        textures.base_color.as_ref(),
    );
    let metallic_roughness_texture = resolved_texture_handle_for_material_slot(
        world,
        XrdsMaterialTextureSlotKind::MetallicRoughness,
        textures.metallic_roughness.as_ref(),
    );
    let normal_texture = resolved_texture_handle_for_material_slot(
        world,
        XrdsMaterialTextureSlotKind::Normal,
        textures.normal.as_ref(),
    );
    let occlusion_texture = resolved_texture_handle_for_material_slot(
        world,
        XrdsMaterialTextureSlotKind::Occlusion,
        textures.occlusion.as_ref(),
    );
    let emissive_texture = resolved_texture_handle_for_material_slot(
        world,
        XrdsMaterialTextureSlotKind::Emissive,
        textures.emissive.as_ref(),
    );

    let mut flags = 0;
    if base_color_texture.is_some() {
        flags |= XRDS_TEXTURE_FLAG_BASE_COLOR;
    }
    if metallic_roughness_texture.is_some() {
        flags |= XRDS_TEXTURE_FLAG_METALLIC_ROUGHNESS;
    }
    if normal_texture.is_some() {
        flags |= XRDS_TEXTURE_FLAG_NORMAL;
    }
    if occlusion_texture.is_some() {
        flags |= XRDS_TEXTURE_FLAG_OCCLUSION;
    }
    if emissive_texture.is_some() {
        flags |= XRDS_TEXTURE_FLAG_EMISSIVE;
    }

    XrdsRuntimeMaterialExtension {
        material_uniform: XrdsRuntimeMaterialExtensionUniform {
            flags,
            _padding: UVec3::ZERO,
            base_color: runtime_texture_slot_uniform(textures.base_color.as_ref()),
            metallic_roughness: runtime_texture_slot_uniform(textures.metallic_roughness.as_ref()),
            normal: runtime_texture_slot_uniform(textures.normal.as_ref()),
            occlusion: runtime_texture_slot_uniform(textures.occlusion.as_ref()),
            emissive: runtime_texture_slot_uniform(textures.emissive.as_ref()),
        },
        base_color_texture,
        metallic_roughness_texture,
        normal_texture,
        occlusion_texture,
        emissive_texture,
    }
}

fn runtime_texture_slot_uniform(
    texture: Option<&XrdsMaterialTextureRef>,
) -> XrdsRuntimeTextureSlotUniform {
    let Some(texture) = texture else {
        return XrdsRuntimeTextureSlotUniform::default();
    };

    XrdsRuntimeTextureSlotUniform {
        uv_transform: runtime_texture_uv_transform(texture.uv),
        uv_set: texture.uv.set.min(1),
        _padding: UVec3::ZERO,
    }
}

pub(super) fn runtime_texture_uv_transform(uv: XrdsMaterialTextureUvParams) -> Mat3 {
    let rotation_rad = uv.rotation_deg.to_radians();
    let (sin_theta, cos_theta) = rotation_rad.sin_cos();
    let m00 = cos_theta * uv.scale[0];
    let m10 = sin_theta * uv.scale[0];
    let m01 = -sin_theta * uv.scale[1];
    let m11 = cos_theta * uv.scale[1];
    let [mut tx, mut ty] = uv.offset;

    if matches!(
        uv.transform_mode,
        xrds_components::XrdsMaterialTextureUvTransformMode::Centered
    ) {
        let center_x = 0.5;
        let center_y = 0.5;
        tx += center_x - (m00 * center_x + m01 * center_y);
        ty += center_y - (m10 * center_x + m11 * center_y);
    }

    Mat3::from_cols_array(&[m00, m10, 0.0, m01, m11, 0.0, tx, ty, 1.0])
}

fn resolved_texture_handle_for_material_slot(
    world: Option<&World>,
    slot: XrdsMaterialTextureSlotKind,
    texture: Option<&XrdsMaterialTextureRef>,
) -> Option<bevy::asset::Handle<Image>> {
    let texture = texture?;
    let world = world?;
    let asset_uri = resolve_texture_asset_uri_in_world(world, &texture.texture_asset_id)?;
    let server = world.get_resource::<AssetServer>()?;

    let uses_srgb = texture_slot_uses_srgb(slot);
    if uses_srgb && texture.sampler == XrdsMaterialTextureSamplerParams::default() {
        return Some(server.load::<Image>(asset_uri));
    }

    let sampler = runtime_image_sampler_descriptor(texture.sampler);
    Some(
        server.load_with_settings::<Image, ImageLoaderSettings>(asset_uri, move |settings| {
            settings.is_srgb = uses_srgb;
            settings.sampler = ImageSampler::Descriptor(sampler.clone());
        }),
    )
}

pub(super) fn texture_slot_uses_srgb(slot: XrdsMaterialTextureSlotKind) -> bool {
    matches!(
        slot,
        XrdsMaterialTextureSlotKind::BaseColor | XrdsMaterialTextureSlotKind::Emissive
    )
}

pub(super) fn runtime_image_sampler_descriptor(
    sampler: XrdsMaterialTextureSamplerParams,
) -> ImageSamplerDescriptor {
    ImageSamplerDescriptor {
        address_mode_u: runtime_image_address_mode(sampler.wrap_u),
        address_mode_v: runtime_image_address_mode(sampler.wrap_v),
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: runtime_image_filter_mode(sampler.mag_filter),
        min_filter: runtime_image_filter_mode(sampler.min_filter),
        mipmap_filter: runtime_image_filter_mode(sampler.mipmap_filter),
        ..default()
    }
}

fn runtime_image_address_mode(mode: XrdsMaterialTextureWrapMode) -> ImageAddressMode {
    match mode {
        XrdsMaterialTextureWrapMode::Repeat => ImageAddressMode::Repeat,
        XrdsMaterialTextureWrapMode::MirroredRepeat => ImageAddressMode::MirrorRepeat,
        XrdsMaterialTextureWrapMode::ClampToEdge => ImageAddressMode::ClampToEdge,
    }
}

fn runtime_image_filter_mode(mode: XrdsMaterialTextureFilterMode) -> ImageFilterMode {
    match mode {
        XrdsMaterialTextureFilterMode::Linear => ImageFilterMode::Linear,
        XrdsMaterialTextureFilterMode::Nearest => ImageFilterMode::Nearest,
    }
}

fn resolve_texture_asset_uri_in_world(world: &World, asset_id: &str) -> Option<String> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return None;
    }

    world
        .get_resource::<XrdsImportedAssetCatalog>()?
        .assets
        .iter()
        .find(|asset| asset.id == asset_id && asset.kind == XrdsSceneAssetKind::Texture)
        .map(|asset| asset.uri.clone())
}

pub(super) fn apply_authored_material_to_entity(
    world: &mut World,
    entity: Entity,
    params: XrdsMaterialParams,
) {
    let existing_handle = world
        .get::<MeshMaterial3d<XrdsRuntimeMaterial>>(entity)
        .map(|handle| handle.0.clone());

    let material_value = runtime_material_from_authored_in_world(Some(world), params.clone());

    match existing_handle {
        Some(handle) => {
            let mut replacement = None;
            if let Some(mut materials) = world.get_resource_mut::<Assets<XrdsRuntimeMaterial>>() {
                if let Some(material) = materials.get_mut(&handle) {
                    *material = material_value.clone();
                } else {
                    replacement = Some(materials.add(material_value.clone()));
                }
            }

            if let Some(new_handle) = replacement {
                world.entity_mut(entity).insert(MeshMaterial3d(new_handle));
            }
        }
        None => {
            if let Some(mut materials) = world.get_resource_mut::<Assets<XrdsRuntimeMaterial>>() {
                let handle = materials.add(material_value);
                world.entity_mut(entity).insert(MeshMaterial3d(handle));
            }
        }
    }

    world.entity_mut(entity).insert(XrdsStoredMaterial(params));
}

pub(super) fn material_params_for_entity(
    world: &World,
    entity: Entity,
) -> Option<XrdsMaterialParams> {
    world
        .get::<XrdsStoredMaterial>(entity)
        .map(|material| material.0.clone())
}
