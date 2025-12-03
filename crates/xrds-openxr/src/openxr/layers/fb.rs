use crate::openxr::layers::OpenXrCompositionLayer;

#[allow(unused)]
#[derive(Clone)]
pub struct OpenXrCompositionLayerPassthroughFB {}

#[allow(unused)]
#[derive(Clone)]
pub struct OpenXrCompositionLayerAlphaBlendFB {}

impl OpenXrCompositionLayer for OpenXrCompositionLayerPassthroughFB {
    fn as_raw(&self) -> &openxr::sys::CompositionLayerBaseHeader {
        unimplemented!()
    }
}

impl OpenXrCompositionLayer for OpenXrCompositionLayerAlphaBlendFB {
    fn as_raw(&self) -> &openxr::sys::CompositionLayerBaseHeader {
        unimplemented!()
    }
}
