pub mod runtime;
pub mod runtime_builder;

use std::{
    ffi::{c_char, CStr},
    ptr::null_mut,
};

use runtime::Runtime;
use runtime_builder::RuntimeBuilder;

/// Create builder for create runtime object
#[no_mangle]
pub extern "C" fn xrds_CreateRuntimeBuilder() -> *mut RuntimeBuilder {
    Box::leak(Box::new(runtime_builder::new()))
}

#[no_mangle]
pub unsafe extern "C" fn xrds_RuntimeBuilder_SetApplicationName(
    raw_builder: &mut RuntimeBuilder,
    raw_application_name: *const c_char,
) {
    let an = CStr::from_ptr(raw_application_name);
    raw_builder.set_application_name(an.to_str().unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn xrds_RuntimeBuilder_Build(
    raw_builder: *mut RuntimeBuilder,
) -> *mut Runtime {
    if raw_builder.is_null() {
        null_mut()
    } else {
        unsafe {
            let builder = Box::<RuntimeBuilder>::from_raw(raw_builder);
            Box::leak(Box::new(builder.build()))
        }
    }
}
