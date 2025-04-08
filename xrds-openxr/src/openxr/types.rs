#[derive(Debug, Clone, Copy)]
pub enum FormFactor {
    HeadMountedDisplay,
    HandheldDisplay,
}

#[derive(Debug, Clone, Copy)]
pub enum ViewType {
    Mono,
    Stereo,
    StereoFoveated,
}

#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    Opaque,
    Additive,
    AlphaBlend,
}

#[derive(Debug, Clone, Copy)]
pub struct View {
    pub max_image_size: wgpu::Extent3d,
    pub max_swapchain_sample_count: u32,
    pub recommended_image_size: wgpu::Extent3d,
    pub recommended_swapchain_sample_count: u32,
}

#[derive(Debug, Clone)]
pub struct ViewConfiguration {
    pub ty: ViewType,
    pub views: Vec<View>,
    pub blend_modes: Vec<BlendMode>,
    pub fov_mutable: bool,
}

impl From<FormFactor> for openxr::FormFactor {
    fn from(value: FormFactor) -> Self {
        openxr::FormFactor::from_raw(match value {
            FormFactor::HeadMountedDisplay => 1i32,
            FormFactor::HandheldDisplay => 2i32,
        })
    }
}

impl From<&FormFactor> for openxr::FormFactor {
    fn from(value: &FormFactor) -> Self {
        openxr::FormFactor::from_raw(match value {
            FormFactor::HeadMountedDisplay => 1i32,
            FormFactor::HandheldDisplay => 2i32,
        })
    }
}

impl From<openxr::FormFactor> for FormFactor {
    fn from(value: openxr::FormFactor) -> Self {
        let raw = value.into_raw();
        match raw {
            1 => FormFactor::HeadMountedDisplay,
            2 => FormFactor::HandheldDisplay,
            _ => panic!("Unknown formfactor type {}", raw),
        }
    }
}

impl From<&openxr::FormFactor> for FormFactor {
    fn from(value: &openxr::FormFactor) -> Self {
        let raw = value.into_raw();
        match raw {
            1 => FormFactor::HeadMountedDisplay,
            2 => FormFactor::HandheldDisplay,
            _ => panic!("Unknown formfactor type {}", raw),
        }
    }
}

impl From<openxr::EnvironmentBlendMode> for BlendMode {
    fn from(value: openxr::EnvironmentBlendMode) -> Self {
        match value {
            openxr::EnvironmentBlendMode::OPAQUE => BlendMode::Opaque,
            openxr::EnvironmentBlendMode::ADDITIVE => BlendMode::Additive,
            openxr::EnvironmentBlendMode::ALPHA_BLEND => BlendMode::AlphaBlend,
            _ => panic!("Unknown blend mode type"),
        }
    }
}

impl From<BlendMode> for openxr::EnvironmentBlendMode {
    fn from(value: BlendMode) -> Self {
        match value {
            BlendMode::Opaque => openxr::EnvironmentBlendMode::OPAQUE,
            BlendMode::Additive => openxr::EnvironmentBlendMode::ADDITIVE,
            BlendMode::AlphaBlend => openxr::EnvironmentBlendMode::ALPHA_BLEND,
        }
    }
}

impl From<openxr::ViewConfigurationType> for ViewType {
    fn from(value: openxr::ViewConfigurationType) -> Self {
        match value {
            openxr::ViewConfigurationType::PRIMARY_MONO => ViewType::Mono,
            openxr::ViewConfigurationType::PRIMARY_STEREO => ViewType::Stereo,
            openxr::ViewConfigurationType::PRIMARY_STEREO_WITH_FOVEATED_INSET => {
                ViewType::StereoFoveated
            }
            _ => panic!("Unknown view type"),
        }
    }
}

impl From<ViewType> for openxr::ViewConfigurationType {
    fn from(value: ViewType) -> Self {
        match value {
            ViewType::Mono => openxr::ViewConfigurationType::PRIMARY_MONO,
            ViewType::Stereo => openxr::ViewConfigurationType::PRIMARY_STEREO,
            ViewType::StereoFoveated => {
                openxr::ViewConfigurationType::PRIMARY_STEREO_WITH_FOVEATED_INSET
            }
        }
    }
}
