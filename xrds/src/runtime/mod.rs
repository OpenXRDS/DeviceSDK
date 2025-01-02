// pub mod builder;

// use std::{
//     ffi::{c_char, c_void, CStr},
//     ptr::null_mut,
// };

// use xrds_runtime::runtime::{Runtime, RuntimeHandler};
// use xrds_runtime::runtime_builder::{self, RuntimeBuilder};

// #[repr(C)]
// pub struct AppFunctions {
//     pub on_construct: unsafe extern "C" fn(*mut c_void),
//     pub on_begin: unsafe extern "C" fn(*mut c_void),
//     pub on_resumed: unsafe extern "C" fn(*mut c_void),
//     pub on_suspended: unsafe extern "C" fn(*mut c_void),
//     pub on_end: unsafe extern "C" fn(*mut c_void),
//     pub on_update: unsafe extern "C" fn(*mut c_void),
//     pub on_deconstruct: unsafe extern "C" fn(*mut c_void),
// }

// pub struct FunctionApp {
//     func: AppFunctions,
//     user_private: u64,
// }

// impl FunctionApp {
//     pub fn new(app_functions: Box<AppFunctions>, user_private: u64) -> Box<Self> {
//         Box::new(Self {
//             func: *app_functions,
//             user_private,
//         })
//     }
// }

// impl RuntimeHandler for FunctionApp {
//     fn on_begin(&mut self) -> anyhow::Result<()> {
//         unsafe { (self.func.on_begin)(self.user_private as *mut c_void) };
//         Ok(())
//     }
//     fn on_construct(&mut self) -> anyhow::Result<()> {
//         Ok(())
//     }
//     fn on_end(&mut self) -> anyhow::Result<()> {
//         Ok(())
//     }
//     fn on_resumed(&mut self) -> anyhow::Result<()> {
//         Ok(())
//     }
//     fn on_suspended(&mut self) -> anyhow::Result<()> {
//         Ok(())
//     }
//     fn on_update(&mut self) -> anyhow::Result<()> {
//         Ok(())
//     }
//     fn on_deconstruct(&mut self) -> anyhow::Result<()> {
//         Ok(())
//     }
// }

// /// Create builder for create runtime object
// #[no_mangle]
// extern "C" fn xrds_CreateRuntimeBuilder() -> *mut RuntimeBuilder {
//     Box::leak(Box::new(runtime_builder::new()))
// }

// #[no_mangle]
// unsafe extern "C" fn xrds_RuntimeBuilder_SetApplicationName(
//     builder: &mut RuntimeBuilder,
//     application_name: *const c_char,
// ) {
//     let an = CStr::from_ptr(application_name);
//     builder.set_application_name(an.to_str().unwrap());
// }

// #[no_mangle]
// unsafe extern "C" fn xrds_RuntimeBuilder_SetAppFunctions(
//     builder: *mut RuntimeBuilder,
//     app: *mut AppFunctions,
//     user_private: *mut c_void,
// ) {
//     if !builder.is_null() {
//         let (mut builder, app_functions) = unsafe {
//             let builder = Box::<RuntimeBuilder>::from_raw(builder);
//             let app_functions = Box::<AppFunctions>::from_raw(app);
//             (builder, app_functions)
//         };
//         builder.set_runtime_application(FunctionApp::new(app_functions, user_private as u64));
//     }
// }

// #[no_mangle]
// unsafe extern "C" fn xrds_RuntimeBuilder_Build(builder: *mut RuntimeBuilder) -> *mut Runtime {
//     if builder.is_null() {
//         null_mut()
//     } else {
//         unsafe {
//             let builder = Box::<RuntimeBuilder>::from_raw(builder);
//             Box::leak(Box::new(builder.build()))
//         }
//     }
// }

// #[no_mangle]
// unsafe extern "C" fn xrds_Runtime_Run(runtime: *mut Runtime) {
//     if !runtime.is_null() {
//         let boxed_runtime = unsafe { Box::<Runtime>::from_raw(runtime) };
//         boxed_runtime.run().expect("Run failure");
//     }
// }
