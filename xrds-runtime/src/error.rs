use core::fmt;
use std::{error::Error, fmt::Debug};

#[derive(Debug)]
pub enum RuntimeError {
    OPENXR,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OPENXR => write!(f, "OpenXR error"),
        }
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
