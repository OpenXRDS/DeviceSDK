use opus::{Encoder as OpusEncoder, Application as OpusApplication, Channels as OpusChannels};

pub fn encode_pcm_to_opus(
    opus_enc: &mut OpusEncoder,
    pcm: &[i16],
    frame_samples_per_channel: i32,
) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; 4000];
    match opus_enc.encode(pcm, &mut out) {
        Ok(len) => {
            out.truncate(len);
            Ok(out)
        }
        Err(e) => Err(format!("Opus encode error: {:?}", e)),
    }
}