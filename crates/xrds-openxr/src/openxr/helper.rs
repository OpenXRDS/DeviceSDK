//! FFI helper functions from openxr-rs

pub(crate) fn cvt(x: openxr::sys::Result) -> openxr::Result<openxr::sys::Result> {
    if x.into_raw() >= 0 {
        Ok(x)
    } else {
        Err(x)
    }
}

pub(crate) fn get_arr_init<T: Copy>(
    init: T,
    mut getter: impl FnMut(u32, &mut u32, *mut T) -> openxr::sys::Result,
) -> openxr::Result<Vec<T>> {
    let mut output = 0;
    cvt(getter(0, &mut output, std::ptr::null_mut()))?;
    let mut buffer = vec![init; output as usize];
    loop {
        match cvt(getter(output, &mut output, buffer.as_mut_ptr() as _)) {
            Ok(_) => {
                buffer.truncate(output as usize);
                return Ok(buffer);
            }
            Err(openxr::sys::Result::ERROR_SIZE_INSUFFICIENT) => {
                buffer.resize(output as usize, init);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
}
