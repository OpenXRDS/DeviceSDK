use crate::openxr::{
    frame::OpenXrFrameWaiter,
    graphics::openxr_graphics,
    resources::{OpenXrFrameStream, OpenXrInstance},
    session::{OpenXrSession, OpenXrSessionCreateInfo},
};

impl OpenXrInstance {
    #[inline]
    pub fn create_session(
        &self,
        info: &OpenXrSessionCreateInfo,
    ) -> openxr::Result<(OpenXrSession, OpenXrFrameWaiter, OpenXrFrameStream)> {
        let (session, frame_waiter, frame_stream) = openxr_graphics!(
            &info.0;
            inner => {
                let (session, frame_waiter, frame_stream) = unsafe { self.instance.create_session::<Api>(self.system_id, inner) }?;
                (OpenXrSession::from_inner(session), OpenXrFrameWaiter::from_inner(frame_waiter), OpenXrFrameStream::from_inner(frame_stream))
            }
        );

        Ok((session, frame_waiter, frame_stream))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn create_action_set(
        &self,
        name: &str,
        localized_name: &str,
        priority: u32,
    ) -> openxr::Result<openxr::ActionSet> {
        self.instance
            .create_action_set(name, localized_name, priority)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn create_session_with_guard(
        &self,
        info: &OpenXrSessionCreateInfo,
        drop_guard: Box<dyn std::any::Any + Send + Sync>,
    ) -> openxr::Result<(OpenXrSession, OpenXrFrameWaiter, OpenXrFrameStream)> {
        let (session, frame_waiter, frame_stream) = openxr_graphics!(
            &info.0;
            inner => {
                let (session, frame_waiter, frame_stream) = unsafe { self.instance.create_session_with_guard::<Api>(self.system_id, inner, drop_guard) }?;
                (OpenXrSession::from_inner(session), OpenXrFrameWaiter::from_inner(frame_waiter), OpenXrFrameStream::from_inner(frame_stream))
            }
        );

        Ok((session, frame_waiter, frame_stream))
    }

    #[inline]
    pub fn poll_event<'a>(
        &self,
        storage: &'a mut openxr::EventDataBuffer,
    ) -> openxr::Result<Option<openxr::Event<'a>>> {
        self.instance.poll_event(storage)
    }

    #[inline]
    pub fn enumerate_view_configurations(
        &self,
    ) -> openxr::Result<Vec<openxr::ViewConfigurationType>> {
        self.instance.enumerate_view_configurations(self.system_id)
    }

    #[inline]
    pub fn enumerate_view_configuration_views(
        &self,
        view_configuration_type: &openxr::ViewConfigurationType,
    ) -> openxr::Result<Vec<openxr::ViewConfigurationView>> {
        self.instance
            .enumerate_view_configuration_views(self.system_id, *view_configuration_type)
    }

    #[inline]
    pub fn enumerate_environment_blend_modes(
        &self,
        view_configuration_type: &openxr::ViewConfigurationType,
    ) -> openxr::Result<Vec<openxr::EnvironmentBlendMode>> {
        self.instance
            .enumerate_environment_blend_modes(self.system_id, *view_configuration_type)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn system_properties(&self) -> openxr::Result<openxr::SystemProperties> {
        self.instance.system_properties(self.system_id)
    }
}
