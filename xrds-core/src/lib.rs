/*
Copyright 2024 OpenXRDS

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/
use ::safer_ffi::prelude::*;

#[derive_ReprC]
#[repr(opaque)]
#[derive(Debug, Clone, Copy)]
pub struct HelloStruct {
    pub x: u64,
    pub y: u64,
    z: u64,
}

impl HelloStruct {
    pub fn new(x: u64, y: u64) -> Self {
        Self {
            x: x,
            y: y,
            z: x + y,
        }
    }
}

#[ffi_export]
fn xrds_core_new_hello(x: u64, y: u64) -> repr_c::Box<HelloStruct> {
    repr_c::Box::<HelloStruct>::new(new_hello(x, y))
}

#[ffi_export]
fn xrds_core_destroy_hello(ptr: repr_c::Box<HelloStruct>) {
    drop(ptr)
}

#[ffi_export]
fn xrds_core_hello_rust(st: &HelloStruct) {
    hello_rust(st);
}

pub fn new_hello(x: u64, y: u64) -> HelloStruct {
    HelloStruct::new(x, y)
}

pub fn hello_rust(st: &HelloStruct) {
    println!("Hello Rust!! {} / {} / {}", st.x, st.y, st.z)
}

#[cfg(feature = "headers")]
pub fn generate_headers() -> std::io::Result<()> {
    safer_ffi::headers::builder()
        .with_guard("__XRDS_CORE_H__")
        .to_file("include/xrds/core.h")?
        .generate()
}
