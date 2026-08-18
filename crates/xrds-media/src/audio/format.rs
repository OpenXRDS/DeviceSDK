//! PCM stream format descriptor.

/// Describes the PCM stream emitted by a capture source.
///
/// Sent alongside the audio channel so the consumer (e.g. the xrds-net Opus
/// encoder) knows how to interpret and resample the `i16` samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    /// Samples per second per channel (e.g. 48_000).
    pub sample_rate: u32,
    /// Number of interleaved channels (1 = mono, 2 = stereo).
    pub channels: u16,
}

impl AudioFormat {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_and_compares() {
        let a = AudioFormat::new(48_000, 2);
        assert_eq!(a.sample_rate, 48_000);
        assert_eq!(a.channels, 2);
        assert_eq!(a, AudioFormat::new(48_000, 2));
        assert_ne!(a, AudioFormat::new(44_100, 2));
    }
}
