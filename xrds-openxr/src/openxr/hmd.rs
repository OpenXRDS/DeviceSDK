use uuid::Uuid;
use xrds_graphics::{
    AssetServer, CameraComponent, CameraInfo, Entity, Fov, PostRenderAction, ProjectionType,
    RenderTargetType,
};

pub struct HmdEntity {}

/// Set of entities for represent HMD camera set
impl HmdEntity {
    pub fn build(asset_server: &mut AssetServer) -> anyhow::Result<Uuid> {
        let root_id = Uuid::new_v4();
        let root = Entity::default()
            .with_id(&root_id)
            .with_name("HMD")
            .with_camera_component(
                CameraComponent::default()
                    .with_camera(&[
                        CameraInfo::new(Fov::default(), ProjectionType::Perspective, 10000.0, 0.05),
                        CameraInfo::new(Fov::default(), ProjectionType::Perspective, 10000.0, 0.05),
                    ])
                    .with_render_target_type(RenderTargetType::Texture2DArray)
                    .with_post_render_action(PostRenderAction::CopyFinal),
            );

        log::info!("HmdEntity: {:?}", root_id);
        asset_server.register_entities(&[root]);

        Ok(root_id)
    }
}
