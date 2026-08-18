//! Microphone device capture via `cpal`.
//!
//! Opens the default input device and streams PCM `i16` chunks over a channel,
//! normalising the device's native sample format via the pure [`convert`]
//! helpers. Dropping the [`Microphone`] stops the stream.

use std::sync::mpsc::{channel, Receiver};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::audio::{convert, AudioFormat};

/// A running microphone capture session.
///
/// Holds the live `cpal` stream; keep it alive to keep capturing. Dropping it
/// stops the stream.
pub struct Microphone {
    _stream: cpal::Stream,
}

impl Microphone {
    /// Open the default input device and start capturing.
    ///
    /// Returns the session handle, a receiver of interleaved PCM `i16` chunks,
    /// and the device's [`AudioFormat`]. Sample-format conversion to `i16` is
    /// applied inside the cpal callback.
    pub fn open_default() -> Result<(Self, Receiver<Vec<i16>>, AudioFormat), String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no default input device found")?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("failed to get default input config: {e}"))?;

        let format = AudioFormat::new(supported.sample_rate().0, supported.channels());
        log::info!(
            "microphone: {}Hz, {} channels, format {:?}",
            format.sample_rate,
            format.channels,
            supported.sample_format()
        );

        let (tx, rx) = channel::<Vec<i16>>();
        let config = supported.config();

        let stream = match supported.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    let _ = tx.send(convert::f32_to_i16(data));
                },
                err_fn,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    let _ = tx.send(data.to_vec());
                },
                err_fn,
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    let _ = tx.send(convert::u16_to_i16(data));
                },
                err_fn,
            ),
        }
        .map_err(|e| format!("failed to build input stream: {e}"))?;

        stream.play().map_err(|e| format!("failed to start stream: {e}"))?;

        Ok((Self { _stream: stream }, rx, format))
    }
}

fn err_fn(e: cpal::StreamError) {
    log::error!("audio stream error: {e}");
}
