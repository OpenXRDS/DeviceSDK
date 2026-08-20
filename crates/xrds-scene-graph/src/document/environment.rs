use super::*;

impl XrdsSceneDocument {
    pub fn environment(&self) -> Option<&XrdsSceneEnvironment> {
        self.metadata.environment.as_ref()
    }

    pub fn ibl_environment(&self) -> Option<&XrdsSceneIblEnvironment> {
        self.environment()?.ibl.as_ref()
    }

    pub fn ibl_environment_asset_ids(&self) -> Option<(&str, &str)> {
        let ibl = self.ibl_environment()?;
        Some((&ibl.diffuse_asset_id, &ibl.specular_asset_id))
    }

    pub fn skybox_environment(&self) -> Option<&XrdsSceneSkyboxEnvironment> {
        self.environment()?.skybox.as_ref()
    }

    pub fn skybox_environment_asset_id(&self) -> Option<&str> {
        Some(&self.skybox_environment()?.texture_asset_id)
    }

    pub fn exposure_environment(&self) -> Option<&XrdsSceneExposureEnvironment> {
        self.environment()?.exposure.as_ref()
    }

    pub fn fog_environment(&self) -> Option<&XrdsSceneFogEnvironment> {
        self.environment()?.fog.as_ref()
    }

    pub fn set_ibl_environment(
        &mut self,
        diffuse_asset_id: impl Into<String>,
        specular_asset_id: impl Into<String>,
        intensity: f32,
    ) -> Result<(), XrdsSceneEnvironmentWorkflowError> {
        if !intensity.is_finite() || intensity < 0.0 {
            return Err(XrdsSceneEnvironmentWorkflowError::InvalidIblIntensity);
        }

        let diffuse_asset_id = normalize_asset_id(diffuse_asset_id.into())
            .map_err(XrdsSceneEnvironmentWorkflowError::Asset)?;
        let specular_asset_id = normalize_asset_id(specular_asset_id.into())
            .map_err(XrdsSceneEnvironmentWorkflowError::Asset)?;

        self.metadata
            .environment
            .get_or_insert_with(XrdsSceneEnvironment::default)
            .ibl = Some(XrdsSceneIblEnvironment {
            diffuse_asset_id,
            specular_asset_id,
            intensity,
        });

        self.validate()
            .map_err(XrdsSceneEnvironmentWorkflowError::Validation)
    }

    pub fn clear_ibl_environment(&mut self) {
        let Some(environment) = self.metadata.environment.as_mut() else {
            return;
        };

        environment.ibl = None;
        if environment.is_empty() {
            self.metadata.environment = None;
        }
    }

    /// Set the scene skybox.
    ///
    /// `yaw_deg` turns the sky about the vertical axis — the adjustment that places
    /// the sun where the author wants it. Any finite value is accepted and wrapped
    /// on use; rejecting 370° would be pedantry, since it is the same sky as 10°.
    pub fn set_skybox_environment(
        &mut self,
        texture_asset_id: impl Into<String>,
        brightness: f32,
        yaw_deg: f32,
    ) -> Result<(), XrdsSceneEnvironmentWorkflowError> {
        if !brightness.is_finite() || brightness < 0.0 {
            return Err(XrdsSceneEnvironmentWorkflowError::InvalidSkyboxBrightness);
        }
        if !yaw_deg.is_finite() {
            return Err(XrdsSceneEnvironmentWorkflowError::InvalidSkyboxYaw);
        }

        let texture_asset_id = normalize_asset_id(texture_asset_id.into())
            .map_err(XrdsSceneEnvironmentWorkflowError::Asset)?;

        self.metadata
            .environment
            .get_or_insert_with(XrdsSceneEnvironment::default)
            .skybox = Some(XrdsSceneSkyboxEnvironment {
            texture_asset_id,
            brightness,
            yaw_deg,
        });

        self.validate()
            .map_err(XrdsSceneEnvironmentWorkflowError::Validation)
    }

    pub fn clear_skybox_environment(&mut self) {
        let Some(environment) = self.metadata.environment.as_mut() else {
            return;
        };

        environment.skybox = None;
        if environment.is_empty() {
            self.metadata.environment = None;
        }
    }

    pub fn set_exposure_environment(
        &mut self,
        ev100: f32,
    ) -> Result<(), XrdsSceneEnvironmentWorkflowError> {
        if !ev100.is_finite() {
            return Err(XrdsSceneEnvironmentWorkflowError::InvalidExposureEv100);
        }

        self.metadata
            .environment
            .get_or_insert_with(XrdsSceneEnvironment::default)
            .exposure = Some(XrdsSceneExposureEnvironment { ev100 });

        self.validate()
            .map_err(XrdsSceneEnvironmentWorkflowError::Validation)
    }

    pub fn clear_exposure_environment(&mut self) {
        let Some(environment) = self.metadata.environment.as_mut() else {
            return;
        };

        environment.exposure = None;
        if environment.is_empty() {
            self.metadata.environment = None;
        }
    }

    pub fn set_fog_environment(
        &mut self,
        color: [f32; 4],
        falloff: XrdsSceneFogFalloff,
    ) -> Result<(), XrdsSceneEnvironmentWorkflowError> {
        if color.iter().any(|channel| !channel.is_finite()) {
            return Err(XrdsSceneEnvironmentWorkflowError::InvalidFogColor);
        }
        validate_fog_falloff(&falloff)
            .map_err(|_| XrdsSceneEnvironmentWorkflowError::InvalidFogRange)?;

        self.metadata
            .environment
            .get_or_insert_with(XrdsSceneEnvironment::default)
            .fog = Some(XrdsSceneFogEnvironment { color, falloff });

        self.validate()
            .map_err(XrdsSceneEnvironmentWorkflowError::Validation)
    }

    pub fn clear_fog_environment(&mut self) {
        let Some(environment) = self.metadata.environment.as_mut() else {
            return;
        };

        environment.fog = None;
        if environment.is_empty() {
            self.metadata.environment = None;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum XrdsSceneEnvironmentWorkflowError {
    Asset(XrdsSceneAssetWorkflowError),
    InvalidIblIntensity,
    InvalidSkyboxBrightness,
    /// A non-finite skybox yaw. Rejected rather than clamped because it would reach
    /// `Quat::from_rotation_y` and produce a NaN rotation, which renders as nothing
    /// with no error anywhere.
    InvalidSkyboxYaw,
    InvalidExposureEv100,
    InvalidFogColor,
    InvalidFogRange,
    Validation(XrdsSceneValidationError),
}