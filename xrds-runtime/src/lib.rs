use xrds_core::{hello_rust, HelloStruct};

// #[derive_ReprC]
// #[repr(opaque)]
#[derive(Debug, Clone, Copy)]
pub struct Runtime;

impl Runtime {
    pub fn hello_runtime(self) {
        println!("Hello XRDS runtime!");
        hello_rust(&HelloStruct::new(10, 20));
    }
}

#[no_mangle]
pub extern "C" fn xrds_runtime_create_runtime() -> *mut Runtime {
    Box::leak(Box::new(create_runtime()))
}

#[no_mangle]
pub extern "C" fn xrds_hello_runtime(runtime: *mut Runtime) {
    if runtime.is_null() {
        return;
    } else {
        let r = unsafe { Box::<Runtime>::from_raw(runtime) };
        r.hello_runtime()
    }
}

pub fn create_runtime() -> Runtime {
    Runtime {}
}

// #[cfg(feature = "runtime_headers")]
// pub fn generate_headers() -> std::io::Result<()> {
//     safer_ffi::headers::builder()
//         .with_guard("__XRDS_RUNTIME_H__")
//         .to_file("include/xrds/runtime.h")?
//         .generate()
// }
