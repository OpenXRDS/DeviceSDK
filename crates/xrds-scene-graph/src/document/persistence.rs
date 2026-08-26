use super::*;

impl XrdsSceneDocument {
    pub fn validate(&self) -> Result<(), XrdsSceneValidationError> {
        let mut seen_asset_ids = HashSet::new();

        for asset in &self.assets {
            let asset_id = asset.id.trim();
            if asset_id.is_empty() {
                return Err(XrdsSceneValidationError::EmptyAssetId);
            }

            if !seen_asset_ids.insert(asset_id.to_string()) {
                return Err(XrdsSceneValidationError::DuplicateAssetId(
                    asset_id.to_string(),
                ));
            }

            if asset.uri.trim().is_empty() {
                return Err(XrdsSceneValidationError::EmptyAssetUri(asset.id.clone()));
            }

            if matches!(
                asset.kind,
                XrdsSceneAssetKind::Texture
                    | XrdsSceneAssetKind::EnvironmentMap
                    | XrdsSceneAssetKind::Audio
                    | XrdsSceneAssetKind::Video
            ) && !has_supported_binary_asset_extension(&asset.uri, asset.kind)
            {
                return Err(XrdsSceneValidationError::InvalidAssetExtension {
                    asset_id: asset.id.clone(),
                    kind: asset.kind,
                });
            }
        }

        validate_scene_environment(self)?;

        let mut seen = HashSet::new();
        let ids: HashMap<_, _> = self.nodes.iter().map(|node| (node.id, node)).collect();

        let mut video_owner: HashMap<String, XrdsSceneNodeId> = HashMap::new();

        for node in &self.nodes {
            if !seen.insert(node.id) {
                return Err(XrdsSceneValidationError::DuplicateNodeId(node.id));
            }

            if let Some(parent_id) = node.parent_id {
                if parent_id == node.id {
                    return Err(XrdsSceneValidationError::SelfParent(node.id));
                }

                if !ids.contains_key(&parent_id) {
                    return Err(XrdsSceneValidationError::MissingParent {
                        node_id: node.id,
                        parent_id,
                    });
                }
            }

            if let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload {
                if asset
                    .asset_id
                    .as_ref()
                    .is_some_and(|asset_id| asset_id.trim().is_empty())
                {
                    return Err(XrdsSceneValidationError::EmptyGltfAssetId(node.id));
                }

                let has_catalog_reference = asset
                    .asset_id
                    .as_ref()
                    .is_some_and(|asset_id| !asset_id.trim().is_empty());

                if !has_catalog_reference && asset.asset_uri.trim().is_empty() {
                    return Err(XrdsSceneValidationError::MissingGltfAssetUri(node.id));
                }
            }

            if let Some(material) = node_material_ref(node) {
                validate_material_texture_slots(self, node.id, &material.textures)?;
                for asset_id in video_asset_ids_in_slots(self, &material.textures) {
                    // One video, one surface.
                    //
                    // Two meshes showing one clip would have to share a decoder, so
                    // they could only ever play in lockstep — and an author who
                    // binds different clips to different meshes reasonably expects
                    // independent playback. Rather than have the model mean one
                    // thing for two copies of a clip and another for one, a video
                    // belongs to a single surface. Reusing it means copying the file
                    // and importing it again, which also makes the second decoder
                    // visible as a second asset rather than hidden.
                    //
                    // Unlike a texture, which is cheap to share: a decoder is a
                    // thread on a desktop and a hardware codec session on a headset.
                    if let Some(previous) = video_owner.insert(asset_id.clone(), node.id) {
                        if previous != node.id {
                            return Err(XrdsSceneValidationError::VideoAssetBoundTwice {
                                asset_id,
                                first_node_id: previous,
                                second_node_id: node.id,
                            });
                        }
                    }
                }
            }

            if let XrdsSceneNodePayload::AudioClip(clip) = &node.payload {
                validate_audio_clip_asset(self, node.id, &clip.asset_id)?;
            }
        }

        validate_gltf_authoring_entries(self)?;

        for node in &self.nodes {
            let mut current = node.parent_id;
            let mut steps = 0usize;

            while let Some(parent_id) = current {
                if parent_id == node.id {
                    return Err(XrdsSceneValidationError::CycleDetected(node.id));
                }

                current = ids.get(&parent_id).and_then(|parent| parent.parent_id);
                steps += 1;

                if steps > self.nodes.len() {
                    return Err(XrdsSceneValidationError::CycleDetected(node.id));
                }
            }
        }

        Ok(())
    }

    fn validate_for_persistence(&self) -> Result<(), XrdsSceneDocumentPersistenceError> {
        if self.version != XRDS_SCENE_DOCUMENT_VERSION {
            return Err(XrdsSceneDocumentPersistenceError::UnsupportedVersion {
                found: self.version,
                expected: XRDS_SCENE_DOCUMENT_VERSION,
            });
        }

        self.validate()
            .map_err(XrdsSceneDocumentPersistenceError::Validation)
    }

    pub fn to_json_string(&self) -> Result<String, XrdsSceneDocumentPersistenceError> {
        self.validate_for_persistence()?;
        serde_json::to_string(self).map_err(XrdsSceneDocumentPersistenceError::Json)
    }

    pub fn to_json_string_pretty(&self) -> Result<String, XrdsSceneDocumentPersistenceError> {
        self.validate_for_persistence()?;
        serde_json::to_string_pretty(self).map_err(XrdsSceneDocumentPersistenceError::Json)
    }

    pub fn from_json_str(json: &str) -> Result<Self, XrdsSceneDocumentPersistenceError> {
        let document: Self =
            serde_json::from_str(json).map_err(XrdsSceneDocumentPersistenceError::Json)?;
        document.validate_for_persistence()?;
        Ok(document)
    }

    pub fn save_json(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), XrdsSceneDocumentPersistenceError> {
        let json = self.to_json_string_pretty()?;
        fs::write(path, json).map_err(XrdsSceneDocumentPersistenceError::Io)
    }

    pub fn load_json(path: impl AsRef<Path>) -> Result<Self, XrdsSceneDocumentPersistenceError> {
        let json = fs::read_to_string(path).map_err(XrdsSceneDocumentPersistenceError::Io)?;
        Self::from_json_str(&json)
    }

    pub fn to_runtime_nodes(&self) -> Result<Vec<XrdsSceneRuntimeNode>, XrdsSceneValidationError> {
        self.validate()?;
        Ok(self
            .nodes
            .iter()
            .map(|node| match &node.payload {
                XrdsSceneNodePayload::GltfAsset(asset) => {
                    let resolved = self.resolve_gltf_asset(asset);
                    let mut runtime_node =
                        node.to_runtime_node_with_gltf_asset_uri(Some(&resolved.asset_uri));
                    runtime_node.gltf_node_authoring =
                        self.gltf_node_authoring_entry(node.id).cloned();
                    runtime_node
                }
                _ => node.to_runtime_node(),
            })
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneValidationError {
    DuplicateAssetId(String),
    EmptyAssetId,
    EmptyAssetUri(String),
    InvalidAssetExtension {
        asset_id: String,
        kind: XrdsSceneAssetKind,
    },
    DuplicateNodeId(XrdsSceneNodeId),
    EmptyGltfAssetId(XrdsSceneNodeId),
    MissingGltfAssetUri(XrdsSceneNodeId),
    EmptyMaterialTextureAssetId {
        node_id: XrdsSceneNodeId,
        slot: XrdsSceneMaterialTextureSlotKind,
    },
    MissingMaterialTextureAsset {
        node_id: XrdsSceneNodeId,
        slot: XrdsSceneMaterialTextureSlotKind,
        asset_id: String,
    },
    /// A video asset bound to more than one mesh. See the check for why.
    VideoAssetBoundTwice {
        asset_id: String,
        first_node_id: XrdsSceneNodeId,
        second_node_id: XrdsSceneNodeId,
    },
    MaterialTextureAssetKindMismatch {
        node_id: XrdsSceneNodeId,
        slot: XrdsSceneMaterialTextureSlotKind,
        asset_id: String,
        found: XrdsSceneAssetKind,
    },
    EmptySceneIblAssetId {
        slot: XrdsSceneIblAssetSlot,
    },
    MissingSceneIblAsset {
        slot: XrdsSceneIblAssetSlot,
        asset_id: String,
    },
    SceneIblAssetKindMismatch {
        slot: XrdsSceneIblAssetSlot,
        asset_id: String,
        found: XrdsSceneAssetKind,
    },
    InvalidSceneIblIntensity,
    EmptySceneSkyboxAssetId {
        slot: XrdsSceneSkyboxAssetSlot,
    },
    MissingSceneSkyboxAsset {
        slot: XrdsSceneSkyboxAssetSlot,
        asset_id: String,
    },
    SceneSkyboxAssetKindMismatch {
        slot: XrdsSceneSkyboxAssetSlot,
        asset_id: String,
        found: XrdsSceneAssetKind,
    },
    InvalidSceneSkyboxBrightness,
    InvalidSceneExposureEv100,
    InvalidSceneFogColor,
    InvalidSceneFogRange,
    EmptyAudioClipAssetId(XrdsSceneNodeId),
    MissingAudioClipAsset {
        node_id: XrdsSceneNodeId,
        asset_id: String,
    },
    AudioClipAssetKindMismatch {
        node_id: XrdsSceneNodeId,
        asset_id: String,
        found: XrdsSceneAssetKind,
    },
    MissingGltfAuthoringNode(XrdsSceneNodeId),
    GltfAuthoringTargetIsNotGltf(XrdsSceneNodeId),
    InvalidGltfAuthoring {
        node_id: XrdsSceneNodeId,
        error: XrdsSceneGltfWorkflowError,
    },
    MissingParent {
        node_id: XrdsSceneNodeId,
        parent_id: XrdsSceneNodeId,
    },
    SelfParent(XrdsSceneNodeId),
    CycleDetected(XrdsSceneNodeId),
}

#[derive(Debug)]
pub enum XrdsSceneDocumentPersistenceError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Validation(XrdsSceneValidationError),
    UnsupportedVersion { found: u32, expected: u32 },
}

fn validate_material_texture_slots(
    document: &XrdsSceneDocument,
    node_id: XrdsSceneNodeId,
    textures: &XrdsSceneMaterialTextureSlots,
) -> Result<(), XrdsSceneValidationError> {
    for slot in [
        XrdsSceneMaterialTextureSlotKind::BaseColor,
        XrdsSceneMaterialTextureSlotKind::MetallicRoughness,
        XrdsSceneMaterialTextureSlotKind::Normal,
        XrdsSceneMaterialTextureSlotKind::Occlusion,
        XrdsSceneMaterialTextureSlotKind::Emissive,
    ] {
        let Some(texture) = textures.get(slot) else {
            continue;
        };

        let asset_id = texture.texture_asset_id.trim();
        if asset_id.is_empty() {
            return Err(XrdsSceneValidationError::EmptyMaterialTextureAssetId { node_id, slot });
        }

        let Some(asset) = document.asset(asset_id) else {
            return Err(XrdsSceneValidationError::MissingMaterialTextureAsset {
                node_id,
                slot,
                asset_id: asset_id.to_string(),
            });
        };

        // Video counts as a texture here, because to a material that is exactly what
        // it is: it fills the same slot, named by the same asset id, and only its
        // contents change. Refusing it made an imported clip bindable in the
        // Inspector and then rejected on commit — the picker offered something the
        // document would not accept.
        //
        // This is the third gate a video has to pass, after the runtime's slot
        // resolver and the editor's picker, and it is the authoritative one: the
        // other two can be right while this refuses the edit.
        if !matches!(
            asset.kind,
            XrdsSceneAssetKind::Texture | XrdsSceneAssetKind::Video
        ) {
            return Err(XrdsSceneValidationError::MaterialTextureAssetKindMismatch {
                node_id,
                slot,
                asset_id: asset.id.clone(),
                found: asset.kind,
            });
        }
    }

    Ok(())
}

fn validate_audio_clip_asset(
    document: &XrdsSceneDocument,
    node_id: XrdsSceneNodeId,
    asset_id: &str,
) -> Result<(), XrdsSceneValidationError> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return Err(XrdsSceneValidationError::EmptyAudioClipAssetId(node_id));
    }

    let Some(asset) = document.asset(asset_id) else {
        return Err(XrdsSceneValidationError::MissingAudioClipAsset {
            node_id,
            asset_id: asset_id.to_string(),
        });
    };

    if asset.kind != XrdsSceneAssetKind::Audio {
        return Err(XrdsSceneValidationError::AudioClipAssetKindMismatch {
            node_id,
            asset_id: asset.id.clone(),
            found: asset.kind,
        });
    }

    Ok(())
}

fn validate_scene_environment(
    document: &XrdsSceneDocument,
) -> Result<(), XrdsSceneValidationError> {
    let Some(environment) = document.metadata.environment.as_ref() else {
        return Ok(());
    };

    if let Some(ibl) = environment.ibl.as_ref() {
        validate_scene_ibl_asset(
            document,
            XrdsSceneIblAssetSlot::Diffuse,
            &ibl.diffuse_asset_id,
        )?;
        validate_scene_ibl_asset(
            document,
            XrdsSceneIblAssetSlot::Specular,
            &ibl.specular_asset_id,
        )?;

        if !ibl.intensity.is_finite() || ibl.intensity < 0.0 {
            return Err(XrdsSceneValidationError::InvalidSceneIblIntensity);
        }
    }

    if let Some(skybox) = environment.skybox.as_ref() {
        validate_scene_skybox_asset(
            document,
            XrdsSceneSkyboxAssetSlot::Texture,
            &skybox.texture_asset_id,
        )?;

        if !skybox.brightness.is_finite() || skybox.brightness < 0.0 {
            return Err(XrdsSceneValidationError::InvalidSceneSkyboxBrightness);
        }
    }

    if let Some(exposure) = environment.exposure.as_ref() {
        if !exposure.ev100.is_finite() {
            return Err(XrdsSceneValidationError::InvalidSceneExposureEv100);
        }
    }

    if let Some(fog) = environment.fog.as_ref() {
        if fog.color.iter().any(|channel| !channel.is_finite()) {
            return Err(XrdsSceneValidationError::InvalidSceneFogColor);
        }
        validate_fog_falloff(&fog.falloff)?;
    }

    Ok(())
}

/// Reject fog parameters that render as artefacts rather than as fog.
///
/// An inverted linear ramp (`end <= start`) and a non-positive visibility both
/// produce garbage rather than an error at draw time — the second divides by zero
/// inside Koschmieder's equation — so they are refused here where an author can
/// still be told.
pub(crate) fn validate_fog_falloff(
    falloff: &XrdsSceneFogFalloff,
) -> Result<(), XrdsSceneValidationError> {
    match *falloff {
        XrdsSceneFogFalloff::Linear { start, end } => {
            if !start.is_finite() || !end.is_finite() || start < 0.0 || end < start {
                return Err(XrdsSceneValidationError::InvalidSceneFogRange);
            }
        }
        XrdsSceneFogFalloff::Exponential { visibility }
        | XrdsSceneFogFalloff::ExponentialSquared { visibility } => {
            if !visibility.is_finite() || visibility <= 0.0 {
                return Err(XrdsSceneValidationError::InvalidSceneFogRange);
            }
        }
    }
    Ok(())
}

fn validate_scene_ibl_asset(
    document: &XrdsSceneDocument,
    slot: XrdsSceneIblAssetSlot,
    asset_id: &str,
) -> Result<(), XrdsSceneValidationError> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return Err(XrdsSceneValidationError::EmptySceneIblAssetId { slot });
    }

    let Some(asset) = document.asset(asset_id) else {
        return Err(XrdsSceneValidationError::MissingSceneIblAsset {
            slot,
            asset_id: asset_id.to_string(),
        });
    };

    if asset.kind != XrdsSceneAssetKind::EnvironmentMap {
        return Err(XrdsSceneValidationError::SceneIblAssetKindMismatch {
            slot,
            asset_id: asset.id.clone(),
            found: asset.kind,
        });
    }

    Ok(())
}

fn validate_scene_skybox_asset(
    document: &XrdsSceneDocument,
    slot: XrdsSceneSkyboxAssetSlot,
    asset_id: &str,
) -> Result<(), XrdsSceneValidationError> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return Err(XrdsSceneValidationError::EmptySceneSkyboxAssetId { slot });
    }

    let Some(asset) = document.asset(asset_id) else {
        return Err(XrdsSceneValidationError::MissingSceneSkyboxAsset {
            slot,
            asset_id: asset_id.to_string(),
        });
    };

    if asset.kind != XrdsSceneAssetKind::EnvironmentMap {
        return Err(XrdsSceneValidationError::SceneSkyboxAssetKindMismatch {
            slot,
            asset_id: asset.id.clone(),
            found: asset.kind,
        });
    }

    Ok(())
}

/// The video assets a material's slots name, deduplicated.
///
/// Deduplicated because one surface may legitimately show a clip in two slots —
/// base colour and emissive, say — and that is still one surface.
fn video_asset_ids_in_slots(
    document: &XrdsSceneDocument,
    textures: &XrdsSceneMaterialTextureSlots,
) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    // Listed rather than iterated, matching `validate_material_texture_slots` just
    // above: the struct has one field per slot and no iterator.
    let slots = [
        textures.base_color.as_ref(),
        textures.metallic_roughness.as_ref(),
        textures.normal.as_ref(),
        textures.occlusion.as_ref(),
        textures.emissive.as_ref(),
    ];
    for texture in slots.into_iter().flatten() {
        let asset_id = texture.texture_asset_id.trim();
        let is_video = document
            .asset(asset_id)
            .is_some_and(|asset| asset.kind == XrdsSceneAssetKind::Video);
        if is_video && !ids.iter().any(|id| id == asset_id) {
            ids.push(asset_id.to_string());
        }
    }
    ids
}

fn has_supported_binary_asset_extension(uri: &str, kind: XrdsSceneAssetKind) -> bool {
    let Some(extension) = Path::new(uri).extension().and_then(|ext| ext.to_str()) else {
        return false;
    };

    let extension = extension.to_ascii_lowercase();
    match kind {
        XrdsSceneAssetKind::Texture => matches!(
            extension.as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "bmp"
                | "tga"
                | "gif"
                | "hdr"
                | "exr"
                | "ktx2"
                | "basis"
                | "dds"
        ),
        XrdsSceneAssetKind::EnvironmentMap => {
            matches!(extension.as_str(), "hdr" | "exr" | "ktx2" | "dds")
        }
        XrdsSceneAssetKind::Audio => {
            matches!(extension.as_str(), "mp3" | "ogg" | "wav" | "flac")
        }
        // Container only — the codec inside cannot be inferred from the extension.
        XrdsSceneAssetKind::Video => matches!(extension.as_str(), "mp4"),
        XrdsSceneAssetKind::Gltf => matches!(extension.as_str(), "gltf" | "glb"),
    }
}
