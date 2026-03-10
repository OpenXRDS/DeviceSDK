/// Openxrs Graphics Wrapper
/// # Safety
///
pub unsafe trait OpenXrGraphicsExtend: openxr::Graphics {
    fn wrap<G: OpenXrGraphicsFamily>(inner: G::Inner<Self>) -> OpenXrGraphicsWrap<G>;
}
pub trait OpenXrGraphicsFamily {
    type Inner<G: OpenXrGraphicsExtend>;
}

impl OpenXrGraphicsFamily for () {
    type Inner<G: OpenXrGraphicsExtend> = ();
}

#[derive(Clone, Debug)]
pub enum OpenXrGraphicsWrap<G: OpenXrGraphicsFamily> {
    Vulkan(G::Inner<openxr::Vulkan>),
    OpenGl(G::Inner<openxr::OpenGL>),
    D3d12(G::Inner<openxr::D3D12>),
}

macro_rules! openxr_graphics {
    (
        $field:expr;
        $var:pat => $expr:expr $(=> $($return:tt)*)?
    ) => {
        match $field {
            $crate::openxr::graphics::OpenXrGraphicsWrap::Vulkan($var) => {
                #[allow(unused)]
                type Api = openxr::Vulkan;
                $expr
            }
            $crate::openxr::graphics::OpenXrGraphicsWrap::OpenGl($var) => {
                #[allow(unused)]
                type Api = openxr::OpenGL;
                $expr
            }
            #[cfg(target_os = "windows")]
            $crate::openxr::graphics::OpenXrGraphicsWrap::D3d12($var) => {
                #[allow(unused)]
                type Api = openxr::D3D12;
                $expr
            }
            #[allow(unreachable_patterns)]
            _ => {
                panic!("Unsupported graphics backends");
            }
        }
    };
}

pub(crate) use openxr_graphics;
