use bevy::{ecs::schedule::ScheduleLabel, prelude::*, render::extract_resource::ExtractResource};

use crate::openxr::resources::OpenXrFrameState;

/// Dedicated schedules for OpenXR management
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenXrSchedules {
    SessionCreate,
    Update,
    Cleanup,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum OpenXrRuntimeSystems {
    PreSessionCreate,
    SessionCreate,
    PostSessionCreate,
    HandleEvents,
    UpdateSessionStates,
    PreFrameLoop,
    WaitFrame,
    FrameLoop,
    PostFrameLoop,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenXrRenderSystems {
    BeginFrame,
    PreRender,
    PostRender,
}

#[derive(Resource, ExtractResource, Clone, Copy, Default, Eq, PartialEq, Hash, Debug)]
pub enum OpenXrSystemState {
    #[default]
    Unavailable,
    Available,
    SessionCreated,
}

#[derive(Resource, ExtractResource, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum OpenXrSessionState {
    #[default]
    Unknown,
    Idle,
    Ready,
    Running,
    Stopping,
    LossPending,
    Exiting,
}

#[derive(Resource, ExtractResource, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum OpenXrDeviceState {
    #[default]
    Unknown,
    Synchronized,
    Visible,
    Focused,
}

#[derive(Message, Clone, Copy, Default, Debug)]
pub struct OpenXrMessageCreateSession;

#[allow(unused)]
pub fn openxr_in_state_synchronized(state: Res<OpenXrDeviceState>) -> bool {
    matches!(
        *state,
        OpenXrDeviceState::Synchronized | OpenXrDeviceState::Visible | OpenXrDeviceState::Focused
    )
}

#[allow(unused)]
pub fn openxr_in_state_visible(state: Res<OpenXrDeviceState>) -> bool {
    matches!(
        *state,
        OpenXrDeviceState::Visible | OpenXrDeviceState::Focused
    )
}

#[allow(unused)]
pub fn openxr_in_state_focused(state: Res<OpenXrDeviceState>) -> bool {
    matches!(*state, OpenXrDeviceState::Focused)
}

#[allow(unused)]
pub fn openxr_should_render(frame_state: Option<Res<OpenXrFrameState>>) -> bool {
    if let Some(frame_state) = frame_state {
        return frame_state.0.should_render;
    }
    false
}
