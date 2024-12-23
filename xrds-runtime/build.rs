use std::env;

extern crate cbindgen;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_cpp_compat(true)
        .with_include_guard("__XRDS_RUNTIME_H__")
        .with_include("xrds/core.h")
        .with_item_prefix("xrds_")
        .with_language(cbindgen::Language::C)
        .with_cpp_compat(true)
        .with_header(concat!(
            "// ***********************************\n",
            "// Auto generated header\n",
            "// ***********************************\n",
        ))
        .generate()
        .unwrap()
        .write_to_file("include/xrds/runtime.h");
}
