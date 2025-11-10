use bevy::prelude::*;

use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::anyhow;
use libloading::Library;
use openxr::{
    sys::loader::{
        FnNegotiateLoaderRuntimeInterface, XrNegotiateLoaderInfo, XrNegotiateRuntimeRequest,
    },
    Entry, NegotiateLoaderInfo, NegotiateRuntimeRequest, Version,
};
use serde::Deserialize;
use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

#[derive(Debug, Deserialize)]
struct RuntimeInfo {
    name: String,
    library_path: String,
    #[allow(dead_code)]
    functions: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct OpenXrRuntimeInfo {
    #[allow(dead_code)]
    file_format_version: String,
    runtime: RuntimeInfo,
}

pub fn try_load_windows_oxr_runtime() -> anyhow::Result<(Entry, Arc<Library>)> {
    let runtime_library_path = runtime_library_path_from_registry()?;
    let entry = load_openxr_entry(&runtime_library_path)?;

    Ok(entry)
}

fn runtime_library_path_from_registry() -> anyhow::Result<PathBuf> {
    let _span = info_span!("xrds-openxr::find_runtime_from_registry");
    info!("Finding OpenXR runtime from registry");

    let key_path = "SOFTWARE\\Khronos\\OpenXR\\1";
    let value_name = "ActiveRuntime";

    let json_path = match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key_path) {
        Ok(key) => match key.get_value::<String, _>(value_name) {
            Ok(value) => PathBuf::from(value),
            Err(e) => {
                return Err(anyhow!(
                    "Could not found registry value for OpenXR runtime: {}",
                    e
                ))
            }
        },
        Err(e) => {
            return Err(anyhow!(
                "Could not found registry key for OpenXR runtime: {}",
                e
            ))
        }
    };

    let json_file = File::open(&json_path)?;
    let buf_reader = BufReader::new(json_file);
    let openxr_runtime_info: OpenXrRuntimeInfo = serde_json::from_reader(buf_reader)?;

    info!("OpenXR runtime found: {}", openxr_runtime_info.runtime.name);

    let library_path = PathBuf::from(openxr_runtime_info.runtime.library_path);
    let library_path = if library_path.is_relative() {
        json_path.parent().unwrap().join(library_path)
    } else {
        library_path
    };

    Ok(library_path)
}

fn load_openxr_entry(path: &Path) -> anyhow::Result<(Entry, Arc<Library>)> {
    let _span = debug_span!("xrds-openxr::load_openxr_entry");

    let oxr_entry = unsafe {
        let lib = libloading::Library::new(path)?;
        let xr_negotiate_loader_runtime_interface: FnNegotiateLoaderRuntimeInterface =
            *lib.get(b"xrNegotiateLoaderRuntimeInterface\0")?;

        let negotiate_loader_info = XrNegotiateLoaderInfo {
            ty: XrNegotiateLoaderInfo::TYPE,
            struct_version: XrNegotiateLoaderInfo::VERSION,
            struct_size: std::mem::size_of::<NegotiateLoaderInfo>(),
            min_interface_version: 0,
            max_interface_version: openxr::sys::CURRENT_LOADER_RUNTIME_VERSION as u32,
            min_api_version: Version::new(0, 0, 0),
            max_api_version: Version::new(1, 1, 49), // openxr::sys::CURRENT_API_VERSION,
        };

        let mut negotiate_runtime_request = XrNegotiateRuntimeRequest {
            ty: XrNegotiateRuntimeRequest::TYPE,
            struct_version: XrNegotiateRuntimeRequest::VERSION,
            struct_size: std::mem::size_of::<NegotiateRuntimeRequest>(),
            runtime_api_version: openxr::sys::CURRENT_API_VERSION,
            runtime_interface_version: 0,
            get_instance_proc_addr: None,
        };

        let result = xr_negotiate_loader_runtime_interface(
            &negotiate_loader_info,
            &mut negotiate_runtime_request,
        );
        debug!(
            "OpenXR runtime version: {}",
            negotiate_runtime_request.runtime_api_version
        );

        match result {
            openxr::sys::Result::SUCCESS => {
                if let Some(get_instance_proc_addr) =
                    negotiate_runtime_request.get_instance_proc_addr
                {
                    (
                        openxr::Entry::from_get_instance_proc_addr(get_instance_proc_addr)?,
                        Arc::new(lib),
                    )
                } else {
                    return Err(anyhow!("get_instance_proc_addr not found"));
                }
            }
            _ => {
                return Err(anyhow!("xrNegotiateRuntimeRequest failed: {}", result));
            }
        }
    };

    Ok(oxr_entry)
}
