pub mod openxr;

fn main() {
    println!("Hello world! from rust");

    unsafe {
        openxr::initialize_openxr();
    }
}
