use std::env;

extern crate cbindgen;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_cpp_compat(true)
        .with_include_guard("__XRDS_H__")
        .with_header(concat!(
            "// ***********************************\n",
            "// Auto generated header\n",
            "// ***********************************\n",
        ))
        .generate()
        .unwrap()
        .write_to_file("include/xrds/xrds.h");
}
