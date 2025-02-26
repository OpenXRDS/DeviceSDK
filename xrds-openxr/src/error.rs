use std::fmt::Display;

#[derive(Debug)]
pub enum OpenXrError {
    SwapchainNotInitialized,
    SwapchainNotAcquired,
    SwapchainAcquireFailed,
    IndexOutOfBounds { index: usize, max: usize },
    NoViewTypeAvailable,
    NoBlendModeAvailable,
    ReferenceSpaceNotAvailable(i32),
}

impl std::error::Error for OpenXrError {}

impl Display for OpenXrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SwapchainNotInitialized => write!(f, "Swapchain not initialized"),
            Self::SwapchainNotAcquired => write!(f, "Swapchain not acquired"),
            Self::SwapchainAcquireFailed => write!(f, "Swapchain acquire failed"),
            Self::IndexOutOfBounds { index, max } => write!(
                f,
                "Index out of bounds (index: {}, max_bound: {})",
                index, max
            ),
            Self::NoViewTypeAvailable => write!(f, "No view type available"),
            Self::NoBlendModeAvailable => write!(f, "No blend mode available"),
            Self::ReferenceSpaceNotAvailable(id) => {
                write!(
                    f,
                    "Reference space not available (ty: {:?})",
                    openxr::ReferenceSpaceType::from_raw(*id)
                )
            }
        }
    }
}
