mod error;
mod runtime;
pub mod viewer;
pub mod xrds_api;

pub use error::*;
pub use runtime::*;
pub use xrds_api::*;
// XR input state — re-exported so desktop hosts (e.g. the editor's play mode)
// can drive it synthetically without depending on xrds-openxr directly.
pub use xrds_openxr::XrInput;

pub mod sdk {
    pub use xrds_components::*;
}
