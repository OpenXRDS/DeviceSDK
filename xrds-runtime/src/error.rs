use core::fmt;
use std::{error::Error, fmt::Debug};

use xrds_openxr::OpenXrError;

#[derive(Debug)]
pub enum RuntimeError {
    OpenXrError(OpenXrError),
    WinitEventLoopError(winit::error::EventLoopError),
    SyncError,
    OpenXrNotInitialized,
    NoWorldLoaded,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenXrError(e) => write!(f, "OpenXR error: {}", e),
            Self::OpenXrNotInitialized => write!(f, "OpenXR not initialized"),
            Self::WinitEventLoopError(e) => write!(f, "Winit event loop error: {}", e),
            Self::SyncError => write!(f, "Sync error"),
            Self::NoWorldLoaded => write!(f, "No world loaded"),
        }
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
