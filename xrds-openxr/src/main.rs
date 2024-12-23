#[link(name = "openxr", kind = "static")]
extern "C" {
    fn test_libxrds_openxr();
}

fn main() {
    println!("Hello world! from rust");

    unsafe {
        test_libxrds_openxr();
    }
}
