extern crate cmake;
use cmake::Config;
use std::env;

fn main() {
    let dst = Config::new("lib")
        .define("CARGO_TARGET_DIR", env::var("OUT_DIR").unwrap())
        .build();

    println!("cargo:rustc-link-search={}", dst.display());
    println!("cargo:rustc-link-lib=openxr-wrapper");
    println!("cargo:rerun-if-changed=lib/CMakeLists.txt");
    println!("cargo:rerun-if-changed=lib/src/openxr-wrapper.cpp");
    println!("cargo:rerun-if-changed=lib/src/openxr-wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("lib/src/openxr-wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file("src/openxr.rs")
        .expect("Couldn't write bindings!");
}
