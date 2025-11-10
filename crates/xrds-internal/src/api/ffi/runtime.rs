use std::{ffi::c_void, ptr};

use xrds_runtime::RuntimeHandler;

use crate::{Runtime, RuntimeBuilder};

#[repr(C)]
pub struct CRuntimeHandler {
    pub on_construct: unsafe extern "C" fn(*mut c_void),
    pub on_begin: unsafe extern "C" fn(*mut c_void),
    pub on_resumed: unsafe extern "C" fn(*mut c_void),
    pub on_suspended: unsafe extern "C" fn(*mut c_void),
    pub on_end: unsafe extern "C" fn(*mut c_void),
    pub on_update: unsafe extern "C" fn(*mut c_void),
    pub on_deconstruct: unsafe extern "C" fn(*mut c_void),
}

pub struct CRuntimeApp {
    func: Box<CRuntimeHandler>,
    user_private: u64,
}

impl CRuntimeApp {
    pub fn new(app_functions: Box<CRuntimeHandler>, user_private: u64) -> Self {
        Self {
            func: app_functions,
            user_private,
        }
    }
}

impl RuntimeHandler for CRuntimeApp {
    fn on_begin(&mut self) {
        unsafe { (self.func.on_begin)(self.user_private as *mut c_void) }
    }
    fn on_construct(&mut self) {
        unsafe { (self.func.on_construct)(self.user_private as *mut c_void) }
    }
    fn on_deconstruct(&mut self) {
        unsafe { (self.func.on_deconstruct)(self.user_private as *mut c_void) }
    }
    fn on_end(&mut self) {
        unsafe { (self.func.on_end)(self.user_private as *mut c_void) }
    }
    fn on_resumed(&mut self) {
        unsafe { (self.func.on_resumed)(self.user_private as *mut c_void) }
    }
    fn on_suspended(&mut self) {
        unsafe { (self.func.on_suspended)(self.user_private as *mut c_void) }
    }
    fn on_update(&mut self) {
        unsafe { (self.func.on_update)(self.user_private as *mut c_void) }
    }
}

#[no_mangle]
extern "C" fn xrds_Runtime_new() -> *mut Runtime {
    let runtime = Runtime::new();
    match runtime {
        Ok(r) => Box::leak(Box::new(r)),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
extern "C" fn xrds_Runtime_builder() -> *mut RuntimeBuilder {
    Box::leak(Box::new(Runtime::builder()))
}

#[no_mangle]
extern "C" fn xrds_RuntimeBuilder_build(builder: *mut RuntimeBuilder) -> *mut Runtime {
    if !builder.is_null() {
        let runtime = unsafe { Box::from_raw(builder) }.build();
        if let Ok(r) = runtime {
            Box::leak(Box::new(r))
        } else {
            ptr::null_mut()
        }
    } else {
        ptr::null_mut()
    }
}

#[no_mangle]
unsafe extern "C" fn xrds_Runtime_Run(
    runtime: *mut Runtime,
    runtime_handler: *mut CRuntimeHandler,
    user_private: u64,
) {
    if !runtime.is_null() && !runtime_handler.is_null() {
        let (runtime, handler) =
            unsafe { (Box::from_raw(runtime), Box::from_raw(runtime_handler)) };

        let app = CRuntimeApp::new(handler, user_private);
        runtime.run(app).expect("Could not run CRuntimeHandler");
    }
}
