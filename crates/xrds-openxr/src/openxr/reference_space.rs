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

    let mut primary_space_type = openxr::ReferenceSpaceType::STAGE;

    let session = world.resource::<OpenXrSession>();
    let reference_space_types = session
        .enumerate_reference_space_types()
        .expect("Could not enumerate reference space types");

    if reference_space_types.contains(&primary_space_type) {
        info!("Reference space type 'stage' supported");
    } else {
        info!("Reference space type 'stage' not supported. Use floor instead");
        primary_space_type = openxr::ReferenceSpaceType::LOCAL_FLOOR;
    }

    let primary_space = session
        .create_reference_space(primary_space_type, openxr::Posef::IDENTITY)
        .expect("Could not create primary reference space");

    world.insert_resource(OpenXrPrimaryReferenceSpace(primary_space));
    info!(
        "OpenXR primary reference space({:?}) and bounds rect created",
        primary_space_type
    );

    // Enumerate actions sets and get space and bound rect for each action set
    // let action_sets = world.get_resource::<ActionSets>();
    // action_sets.sets.iter()~~~

    info!("OpenXR reference space and bounds rect created");
}
