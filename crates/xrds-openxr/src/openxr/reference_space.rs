use bevy::prelude::*;

use crate::openxr::{
    resources::OpenXrPrimaryReferenceSpace,
    schedule::{OpenXrRuntimeSystems, OpenXrSchedules},
    session::OpenXrSession,
};

pub struct OpenXrReferenceSpacePlugin;

impl Plugin for OpenXrReferenceSpacePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OpenXrSchedules::SessionCreate,
            create_reference_space.in_set(OpenXrRuntimeSystems::PostSessionCreate),
        );
    }
}

fn create_reference_space(world: &mut World) {
    debug_span!("OpenXrReferenceSpacePlugin");
    log::info!("XR: create_reference_space start");

    let mut primary_space_type = openxr::ReferenceSpaceType::STAGE;

    let session = world.resource::<OpenXrSession>();
    let reference_space_types = match session.enumerate_reference_space_types() {
        Ok(v) => v,
        Err(e) => {
            log::error!("XR: enumerate_reference_space_types failed: {e:?}");
            return;
        }
    };

    if reference_space_types.contains(&primary_space_type) {
        log::info!("XR: reference space STAGE supported");
    } else {
        log::info!("XR: reference space STAGE not supported, using LOCAL_FLOOR");
        primary_space_type = openxr::ReferenceSpaceType::LOCAL_FLOOR;
    }

    let primary_space = match session
        .create_reference_space(primary_space_type, openxr::Posef::IDENTITY)
    {
        Ok(s) => s,
        Err(e) => {
            log::error!("XR: create_reference_space failed: {e:?}");
            return;
        }
    };

    world.insert_resource(OpenXrPrimaryReferenceSpace(primary_space));
    log::info!("XR: primary reference space({:?}) created", primary_space_type);
}
