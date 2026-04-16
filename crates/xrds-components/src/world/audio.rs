use crate::{default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject};

#[derive(Debug, Clone)]
pub struct XrdsAudioClip {
    pub name: String,
    pub transform: TransformParams,
    pub visible: bool,
    /// Catalog asset id referencing an `XrdsSceneAssetKind::Audio` asset.
    pub audio_asset_id: String,
    /// Playback volume in the range 0.0–1.0.
    pub volume: f32,
    pub looped: bool,
    /// When `true` the clip is attenuated by 3-D distance from the listener.
    /// When `false` the clip plays at the same level everywhere in the scene.
    pub spatial: bool,
    pub autoplay: bool,
}

impl XrdsAudioClip {
    pub fn new(audio_asset_id: impl Into<String>) -> Self {
        Self {
            name: default_component_name::<Self>(),
            transform: TransformParams::default(),
            visible: true,
            audio_asset_id: audio_asset_id.into(),
            volume: 1.0,
            looped: false,
            spatial: true,
            autoplay: false,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl XrdsObject for XrdsAudioClip {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsAudioClip {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsAudioClip {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
