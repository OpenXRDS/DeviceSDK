//! Webcam device capture via `nokhwa`.
//!
//! Opens a camera, captures frames on a dedicated thread, and sends one
//! complete JPEG frame per `Vec<u8>` on the returned channel. Dropping the
//! [`Webcam`] stops capture and joins the thread.
//!
//! The frame receiver is the primary output because a camera naturally
//! produces discrete frames, not a continuous byte stream — wrap it in a
//! [`FrameReader`] only if a specific consumer needs a `std::io::Read`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::thread::JoinHandle;

use nokhwa::pixel_format::YuyvFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

/// A running webcam capture session.
///
/// Keep it alive for as long as you want frames; dropping it signals the capture
/// thread to stop and joins it.
pub struct Webcam {
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Webcam {
    /// Open webcam `device_id` and start capturing.
    ///
    /// Returns the session handle plus a channel yielding one complete JPEG
    /// frame per message. Capture runs on a background thread until the
    /// returned `Webcam` is dropped.
    pub fn open(device_id: u32) -> Result<(Self, Receiver<Vec<u8>>), String> {
        let requested =
            RequestedFormat::new::<YuyvFormat>(RequestedFormatType::AbsoluteHighestResolution);
        let mut camera =
            Camera::new(CameraIndex::Index(device_id), requested).map_err(|e| e.to_string())?;
        camera.open_stream().map_err(|e| e.to_string())?;

        let (tx, rx) = channel::<Vec<u8>>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();

        let handle = std::thread::spawn(move || {
            log::info!("webcam {device_id}: capture thread started");
            while !stop.load(Ordering::Relaxed) {
                match camera.frame() {
                    Ok(frame) => {
                        // Sender dropped means the consumer is gone; stop cleanly.
                        if tx.send(frame.buffer().to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("webcam {device_id}: frame error: {e}");
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
            let _ = camera.stop_stream();
            log::info!("webcam {device_id}: capture thread stopped");
        });

        Ok((
            Self {
                shutdown,
                handle: Some(handle),
            },
            rx,
        ))
    }
}

impl Drop for Webcam {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Enumerate available webcam devices as `"index: name"` strings.
///
/// Returns `Err` (never panics) when no devices are present or the query fails.
pub fn list_available_devices() -> Result<Vec<String>, String> {
    let devices =
        nokhwa::query(ApiBackend::Auto).map_err(|e| format!("failed to query devices: {e}"))?;
    if devices.is_empty() {
        return Err("no webcam devices found".to_string());
    }
    Ok(devices
        .into_iter()
        .enumerate()
        .map(|(i, info)| format!("{i}: {}", info.human_name()))
        .collect())
}
