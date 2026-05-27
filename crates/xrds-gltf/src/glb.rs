/// Write a binary glTF (GLB) file from a JSON chunk and a binary chunk.
///
/// GLB layout:
///   12-byte header: magic, version, total length
///   JSON chunk:  chunk_length(4) + chunk_type(4=0x4E4F534A) + JSON bytes (padded with 0x20)
///   BIN  chunk:  chunk_length(4) + chunk_type(4=0x004E4942) + bin  bytes (padded with 0x00)
pub fn write_glb(json_bytes: &[u8], bin_bytes: &[u8]) -> Vec<u8> {
    const MAGIC: u32 = 0x46546C67; // "glTF"
    const VERSION: u32 = 2;
    const JSON_CHUNK_TYPE: u32 = 0x4E4F534A;
    const BIN_CHUNK_TYPE: u32 = 0x004E4942;

    let json_padded = pad_to(json_bytes, 4, b' ');
    let bin_padded = pad_to(bin_bytes, 4, b'\0');

    let header_len = 12u32;
    let json_chunk_len = 8 + json_padded.len() as u32;
    let bin_chunk_len = if bin_bytes.is_empty() {
        0
    } else {
        8 + bin_padded.len() as u32
    };
    let total = header_len + json_chunk_len + bin_chunk_len;

    let mut out = Vec::with_capacity(total as usize);

    // Header
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());

    // JSON chunk
    out.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
    out.extend_from_slice(&JSON_CHUNK_TYPE.to_le_bytes());
    out.extend_from_slice(&json_padded);

    // BIN chunk (omit entirely if empty)
    if !bin_bytes.is_empty() {
        out.extend_from_slice(&(bin_padded.len() as u32).to_le_bytes());
        out.extend_from_slice(&BIN_CHUNK_TYPE.to_le_bytes());
        out.extend_from_slice(&bin_padded);
    }

    out
}

fn pad_to(data: &[u8], align: usize, pad_byte: u8) -> Vec<u8> {
    let mut v = data.to_vec();
    let rem = v.len() % align;
    if rem != 0 {
        v.extend(std::iter::repeat(pad_byte).take(align - rem));
    }
    v
}
