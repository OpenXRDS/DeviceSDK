//! Pure PCM sample-format conversion.
//!
//! cpal delivers input samples as `f32`, `i16`, or `u16` depending on the device.
//! We normalise everything to signed `i16`. These functions are deliberately
//! device-free so they can be unit-tested without any hardware.

/// Convert `f32` samples (nominally in `[-1.0, 1.0]`) to `i16` PCM.
///
/// Out-of-range values are clamped before scaling so they don't wrap.
pub fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect()
}

/// Convert unsigned 16-bit samples (mid-point `0x8000`) to signed `i16`.
pub fn u16_to_i16(samples: &[u16]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s as i32 - 0x8000) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_maps_full_scale_and_zero() {
        assert_eq!(f32_to_i16(&[0.0]), vec![0]);
        assert_eq!(f32_to_i16(&[1.0]), vec![i16::MAX]);
        // -1.0 * 32767 = -32767 (not i16::MIN, matching the symmetric scaling)
        assert_eq!(f32_to_i16(&[-1.0]), vec![-i16::MAX]);
    }

    #[test]
    fn f32_clamps_out_of_range() {
        assert_eq!(f32_to_i16(&[2.0]), vec![i16::MAX]);
        assert_eq!(f32_to_i16(&[-2.0]), vec![-i16::MAX]);
    }

    #[test]
    fn u16_offsets_midpoint_to_zero() {
        assert_eq!(u16_to_i16(&[0x8000]), vec![0]);
        assert_eq!(u16_to_i16(&[0xFFFF]), vec![0x7FFF]);
        assert_eq!(u16_to_i16(&[0x0000]), vec![-0x8000]);
    }

    #[test]
    fn preserves_length_and_order() {
        let out = f32_to_i16(&[0.0, 1.0, 0.0]);
        assert_eq!(out.len(), 3);
        assert_eq!(out, vec![0, i16::MAX, 0]);
    }
}
