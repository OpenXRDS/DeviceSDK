//! Pure JPEG frame-boundary detection.
//!
//! Camera frames can arrive split across arbitrary byte chunks. This scanner
//! locates a single complete JPEG (SOI `FF D8` … EOI `FF D9`) within a buffer,
//! ignoring any leading bytes before the SOI. It is device-free and unit-tested.

/// JPEG Start-Of-Image marker.
pub const SOI: [u8; 2] = [0xFF, 0xD8];
/// JPEG End-Of-Image marker.
pub const EOI: [u8; 2] = [0xFF, 0xD9];

/// Find one complete JPEG frame in `buf`.
///
/// Returns `Some((start, end))` such that `buf[start..end]` is a whole frame
/// (`start` at the SOI, `end` just past the EOI). Bytes before the SOI are
/// skipped. Returns `None` if no SOI is present, or an SOI is present but its
/// EOI has not arrived yet.
pub fn find_complete_jpeg(buf: &[u8]) -> Option<(usize, usize)> {
    let start = find_marker(buf, &SOI)?;
    // The EOI must come after the SOI's two bytes.
    let after_soi = start + SOI.len();
    let eoi_rel = find_marker(&buf[after_soi..], &EOI)?;
    let end = after_soi + eoi_rel + EOI.len();
    Some((start, end))
}

fn find_marker(buf: &[u8], marker: &[u8; 2]) -> Option<usize> {
    buf.windows(2).position(|w| w == marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut v = SOI.to_vec();
        v.extend_from_slice(payload);
        v.extend_from_slice(&EOI);
        v
    }

    #[test]
    fn finds_a_clean_frame() {
        let f = frame(&[1, 2, 3]);
        assert_eq!(find_complete_jpeg(&f), Some((0, f.len())));
    }

    #[test]
    fn skips_leading_garbage_before_soi() {
        let mut buf = vec![0x00, 0xAB, 0xCD];
        let start = buf.len();
        buf.extend_from_slice(&frame(&[9, 9]));
        let end = buf.len();
        assert_eq!(find_complete_jpeg(&buf), Some((start, end)));
    }

    #[test]
    fn none_when_eoi_missing() {
        let mut buf = SOI.to_vec();
        buf.extend_from_slice(&[1, 2, 3]); // no EOI yet
        assert_eq!(find_complete_jpeg(&buf), None);
    }

    #[test]
    fn none_when_no_soi() {
        assert_eq!(find_complete_jpeg(&[0x00, 0x11, 0x22]), None);
    }

    #[test]
    fn stops_at_first_eoi() {
        // Two frames concatenated: we should return only the first.
        let mut buf = frame(&[1]);
        let first_end = buf.len();
        buf.extend_from_slice(&frame(&[2]));
        assert_eq!(find_complete_jpeg(&buf), Some((0, first_end)));
    }
}
