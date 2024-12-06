extern crate xrds_core;

use xrds_core::{hello_rust, HelloStruct};

pub fn hello_runtime() {
    println!("Hello XRDS runtime!");
    hello_rust(&HelloStruct::new(10, 20));
}

pub struct Runtime {}

pub fn create_runtime() -> Runtime {
    Runtime {}
}
