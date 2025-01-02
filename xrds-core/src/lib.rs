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
#[derive(Debug, Clone, Copy)]
pub struct HelloStruct {
    pub x: u64,
    pub y: u64,
    z: u64,
}

impl HelloStruct {
    pub fn new(x: u64, y: u64) -> Self {
        Self { x, y, z: x + y }
    }
}

#[no_mangle]
pub extern "C" fn xrds_core_new_hello(x: u64, y: u64) -> *mut HelloStruct {
    Box::leak(Box::new(new_hello(x, y)))
}

/// # Safety
///
/// Thid function should not be called with invalid HelloStruct pointer
#[no_mangle]
pub unsafe extern "C" fn xrds_core_destroy_hello(ptr: *mut HelloStruct) {
    if !ptr.is_null() {
        drop(Box::<HelloStruct>::from_raw(ptr))
    }
}

#[no_mangle]
pub extern "C" fn xrds_core_hello_rust(st: &HelloStruct) {
    hello_rust(st);
}

pub fn new_hello(x: u64, y: u64) -> HelloStruct {
    HelloStruct::new(x, y)
}

pub fn hello_rust(st: &HelloStruct) {
    println!("Hello Rust!! {} / {} / {}", st.x, st.y, st.z)
}
