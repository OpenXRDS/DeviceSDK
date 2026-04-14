use super::*;

impl XrdsSceneDocument {
    pub fn node_material(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<&XrdsSceneMaterial, XrdsSceneMaterialWorkflowError> {
        let node = self
            .node(node_id)
            .ok_or(XrdsSceneMaterialWorkflowError::NodeNotFound(node_id))?;
        node_material_ref(node).ok_or(XrdsSceneMaterialWorkflowError::NodeHasNoMaterial(node_id))
    }

    pub fn node_material_pbr(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<&XrdsSceneMaterialPbrParams, XrdsSceneMaterialWorkflowError> {
        Ok(&self.node_material(node_id)?.pbr)
    }

    pub fn node_material_textures(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<&XrdsSceneMaterialTextureSlots, XrdsSceneMaterialWorkflowError> {
        Ok(&self.node_material(node_id)?.textures)
    }

    pub fn set_node_material(
        &mut self,
        node_id: XrdsSceneNodeId,
        material: XrdsSceneMaterial,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        let material = normalize_scene_material(material)?;
        self.update_node_material(node_id, move |existing| *existing = material)
    }

    pub fn set_node_material_base_color(
        &mut self,
        node_id: XrdsSceneNodeId,
        color: XrdsColor,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| material.base_color = color.rgba)
    }

    pub fn set_node_material_emissive(
        &mut self,
        node_id: XrdsSceneNodeId,
        emissive: XrdsLinearRgba,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| material.emissive = emissive.rgba)
    }

    pub fn set_node_material_opacity(
        &mut self,
        node_id: XrdsSceneNodeId,
        opacity: f32,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| {
            material.opacity = opacity.clamp(0.0, 1.0)
        })
    }

    pub fn set_node_material_unlit(
        &mut self,
        node_id: XrdsSceneNodeId,
        unlit: bool,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| material.unlit = unlit)
    }

    pub fn set_node_material_pbr(
        &mut self,
        node_id: XrdsSceneNodeId,
        pbr: XrdsSceneMaterialPbrParams,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        let pbr = normalize_scene_material_pbr(pbr);
        self.update_node_material(node_id, move |material| material.pbr = pbr)
    }

    pub fn set_node_material_textures(
        &mut self,
        node_id: XrdsSceneNodeId,
        textures: XrdsSceneMaterialTextureSlots,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        let textures = normalize_scene_material_textures(textures)?;
        self.update_node_material(node_id, move |material| material.textures = textures)
    }

    pub fn set_node_material_texture(
        &mut self,
        node_id: XrdsSceneNodeId,
        slot: XrdsSceneMaterialTextureSlotKind,
        texture: Option<XrdsSceneTextureRef>,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        let texture = texture
            .map(|texture| normalize_scene_texture_ref(texture, slot))
            .transpose()?;
        self.update_node_material(node_id, move |material| {
            material.textures.set(slot, texture)
        })
    }

    pub fn set_node_material_metallic(
        &mut self,
        node_id: XrdsSceneNodeId,
        metallic: f32,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| {
            material.pbr.metallic = metallic.clamp(0.0, 1.0)
        })
    }

    pub fn set_node_material_perceptual_roughness(
        &mut self,
        node_id: XrdsSceneNodeId,
        perceptual_roughness: f32,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| {
            material.pbr.perceptual_roughness = perceptual_roughness.clamp(0.0, 1.0)
        })
    }

    pub fn set_node_material_reflectance(
        &mut self,
        node_id: XrdsSceneNodeId,
        reflectance: f32,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| {
            material.pbr.reflectance = reflectance.clamp(0.0, 1.0)
        })
    }

    pub fn set_node_material_double_sided(
        &mut self,
        node_id: XrdsSceneNodeId,
        double_sided: bool,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| material.pbr.double_sided = double_sided)
    }

    pub fn set_node_material_alpha_mode(
        &mut self,
        node_id: XrdsSceneNodeId,
        alpha_mode: XrdsSceneMaterialAlphaMode,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| material.pbr.alpha_mode = alpha_mode)
    }

    pub fn set_node_material_alpha_cutoff(
        &mut self,
        node_id: XrdsSceneNodeId,
        alpha_cutoff: f32,
    ) -> Result<(), XrdsSceneMaterialWorkflowError> {
        self.update_node_material(node_id, |material| {
            material.pbr.alpha_cutoff = alpha_cutoff.clamp(0.0, 1.0)
        })
    }

    fn update_node_material<F>(
        &mut self,
        node_id: XrdsSceneNodeId,
        update: F,
    ) -> Result<(), XrdsSceneMaterialWorkflowError>
    where
        F: FnOnce(&mut XrdsSceneMaterial),
    {
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMaterialWorkflowError::NodeNotFound(node_id))?;
        let material = node_material_mut(node)
            .ok_or(XrdsSceneMaterialWorkflowError::NodeHasNoMaterial(node_id))?;
        update(material);
        *material = normalize_scene_material(material.clone())?;
        Ok(())
    }
}

pub(crate) fn node_material_ref(node: &XrdsSceneNode) -> Option<&XrdsSceneMaterial> {
    match &node.payload {
        XrdsSceneNodePayload::Cube(cube) => Some(&cube.material),
        XrdsSceneNodePayload::Cylinder(cylinder) => Some(&cylinder.material),
        XrdsSceneNodePayload::Sphere(sphere) => Some(&sphere.material),
        XrdsSceneNodePayload::Plane3D(plane) => Some(&plane.material),
        XrdsSceneNodePayload::Tetrahedron(tetrahedron) => Some(&tetrahedron.material),
        _ => None,
    }
}

fn node_material_mut(node: &mut XrdsSceneNode) -> Option<&mut XrdsSceneMaterial> {
    match &mut node.payload {
        XrdsSceneNodePayload::Cube(cube) => Some(&mut cube.material),
        XrdsSceneNodePayload::Cylinder(cylinder) => Some(&mut cylinder.material),
        XrdsSceneNodePayload::Sphere(sphere) => Some(&mut sphere.material),
        XrdsSceneNodePayload::Plane3D(plane) => Some(&mut plane.material),
        XrdsSceneNodePayload::Tetrahedron(tetrahedron) => Some(&mut tetrahedron.material),
        _ => None,
    }
}

fn normalize_scene_material(
    mut material: XrdsSceneMaterial,
) -> Result<XrdsSceneMaterial, XrdsSceneMaterialWorkflowError> {
    material.opacity = material.opacity.clamp(0.0, 1.0);
    material.pbr = normalize_scene_material_pbr(material.pbr);
    material.textures = normalize_scene_material_textures(material.textures)?;
    Ok(material)
}

fn normalize_scene_material_textures(
    mut textures: XrdsSceneMaterialTextureSlots,
) -> Result<XrdsSceneMaterialTextureSlots, XrdsSceneMaterialWorkflowError> {
    textures.base_color = textures
        .base_color
        .map(|texture| {
            normalize_scene_texture_ref(texture, XrdsSceneMaterialTextureSlotKind::BaseColor)
        })
        .transpose()?;
    textures.metallic_roughness = textures
        .metallic_roughness
        .map(|texture| {
            normalize_scene_texture_ref(
                texture,
                XrdsSceneMaterialTextureSlotKind::MetallicRoughness,
            )
        })
        .transpose()?;
    textures.normal = textures
        .normal
        .map(|texture| {
            normalize_scene_texture_ref(texture, XrdsSceneMaterialTextureSlotKind::Normal)
        })
        .transpose()?;
    textures.occlusion = textures
        .occlusion
        .map(|texture| {
            normalize_scene_texture_ref(texture, XrdsSceneMaterialTextureSlotKind::Occlusion)
        })
        .transpose()?;
    textures.emissive = textures
        .emissive
        .map(|texture| {
            normalize_scene_texture_ref(texture, XrdsSceneMaterialTextureSlotKind::Emissive)
        })
        .transpose()?;
    Ok(textures)
}

fn normalize_scene_texture_ref(
    mut texture: XrdsSceneTextureRef,
    slot: XrdsSceneMaterialTextureSlotKind,
) -> Result<XrdsSceneTextureRef, XrdsSceneMaterialWorkflowError> {
    let texture_asset_id = texture.texture_asset_id.trim();
    if texture_asset_id.is_empty() {
        return Err(XrdsSceneMaterialWorkflowError::EmptyTextureAssetId(slot));
    }
    texture.texture_asset_id = texture_asset_id.to_string();
    texture.uv = normalize_scene_texture_uv(texture.uv);
    Ok(texture)
}

fn normalize_scene_texture_uv(
    mut uv: XrdsSceneTextureUvParams,
) -> XrdsSceneTextureUvParams {
    if !uv.offset[0].is_finite() {
        uv.offset[0] = 0.0;
    }
    if !uv.offset[1].is_finite() {
        uv.offset[1] = 0.0;
    }
    if !uv.scale[0].is_finite() {
        uv.scale[0] = 1.0;
    }
    if !uv.scale[1].is_finite() {
        uv.scale[1] = 1.0;
    }
    if !uv.rotation_deg.is_finite() {
        uv.rotation_deg = 0.0;
    }
    uv
}

fn normalize_scene_material_pbr(mut pbr: XrdsSceneMaterialPbrParams) -> XrdsSceneMaterialPbrParams {
    pbr.metallic = pbr.metallic.clamp(0.0, 1.0);
    pbr.perceptual_roughness = pbr.perceptual_roughness.clamp(0.0, 1.0);
    pbr.reflectance = pbr.reflectance.clamp(0.0, 1.0);
    pbr.alpha_cutoff = pbr.alpha_cutoff.clamp(0.0, 1.0);
    pbr
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsSceneMaterialWorkflowError {
    NodeNotFound(XrdsSceneNodeId),
    NodeHasNoMaterial(XrdsSceneNodeId),
    EmptyTextureAssetId(XrdsSceneMaterialTextureSlotKind),
}
