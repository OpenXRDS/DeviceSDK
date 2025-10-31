use ffmpeg_next::{self as ffmpeg, Rational};
use ffmpeg::{codec, format, util::error::Error, encoder, Packet};
use crate::client::xrds_webrtc::media::transcoding::jpeg2h264::H264Packet;

/**
 * This class is to verify streaming MP4 writing from H.264 packets.
 * It uses ffmpeg-next to write H.264 packets into an MP4 file in real
 * time as they arrive.
 */
pub struct StreamingMP4Writer {
    octx: format::context::Output,
    encoder_time_base: Rational,
    stream_time_base: Rational,
    packet_count: u64,
}

impl StreamingMP4Writer {
    pub fn new(output_path: &str, width: u32, height: u32, fps: u32) -> Result<Self, Error> {
        let mut octx = format::output(output_path)?;
        let codec = encoder::find(codec::Id::H264).ok_or(Error::EncoderNotFound)?;
        let mut ost = octx.add_stream(codec)?;

        let fps_rational = Rational::new(fps as i32, 1);
        let time_base = fps_rational.invert(); // 1/30
        ost.set_time_base(time_base);

        unsafe {
            let stream = ost.as_mut_ptr();
            if let Some(codecpar) = (*stream).codecpar.as_mut() {
                codecpar.codec_type = ffmpeg::ffi::AVMediaType::AVMEDIA_TYPE_VIDEO;
                codecpar.codec_id = ffmpeg::ffi::AVCodecID::AV_CODEC_ID_H264;
                codecpar.width = width as i32;
                codecpar.height = height as i32;
                codecpar.format = ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_YUV420P as i32;
            }

            (*stream).r_frame_rate = ffmpeg::ffi::AVRational {
                num: fps as i32,
                den: 1,
            };
        }
        
        octx.write_header()?;

        let stream = octx.stream(0).unwrap();
        let stream_time_base = stream.time_base();
        let encoder_time_base = Rational::new(1, fps as i32);

        println!("🎬 Streaming MP4 writer ready: {}x{} @ {}fps", width, height, fps);

        Ok(StreamingMP4Writer {
            octx,
            encoder_time_base,
            stream_time_base,
            packet_count: 0,
        })
    }

    /// Write H.264 packets immediately as they arrive
    pub fn write_packets(&mut self, h264_packets: &[H264Packet]) -> Result<(), Error> {
        for h264_pkt in h264_packets {
            if h264_pkt.data.is_empty() { continue; }

            let mut packet = Packet::new(h264_pkt.data.len());
            if let Some(data) = packet.data_mut() {
                data.copy_from_slice(&h264_pkt.data);
            }
        
            packet.set_stream(0);
            packet.set_pts(Some(h264_pkt.pts));
            packet.set_dts(Some(h264_pkt.dts));
            packet.set_duration(1);

            if h264_pkt.is_keyframe {
                packet.set_flags(ffmpeg::codec::packet::Flags::KEY);
            }

            // CRITICAL: Rescale timestamps
            packet.rescale_ts(self.encoder_time_base, self.stream_time_base);
            packet.write_interleaved(&mut self.octx)?;
            
            self.packet_count += 1;

            if self.packet_count.is_multiple_of(100) {
                println!("📦 Written {} packets", self.packet_count);
            }
        }
        Ok(())
    }

    /// Finalize the MP4 file (call when stream ends)
    pub fn finalize(mut self) -> Result<(), Error> {
        self.octx.write_trailer()?;
        println!("✅ Streaming MP4 finalized: {} packets written", self.packet_count);
        Ok(())
    }
}