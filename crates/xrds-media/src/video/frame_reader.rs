//! Blocking [`Read`] adapter over a channel of byte chunks.
//!
//! This is the buffering/reassembly logic that used to be fused into
//! `WebcamReader` inside xrds-net. It is device-free: it reads `Vec<u8>` chunks
//! (typically whole JPEG frames) from a channel and hands them out as a byte
//! stream, so it can be tested with a plain in-memory channel.

use std::io::{self, Read};
use std::sync::mpsc::Receiver;

/// A blocking [`Read`] over a channel of byte chunks.
///
/// Each received `Vec<u8>` is streamed out in order. If the caller's buffer is
/// smaller than the current chunk, the remainder is retained for the next
/// `read`. When the sending side is dropped and no buffered bytes remain, `read`
/// returns `Ok(0)` (EOF). Empty chunks are skipped rather than misread as EOF.
pub struct FrameReader {
    rx: Receiver<Vec<u8>>,
    remainder: Vec<u8>,
    pos: usize,
}

impl FrameReader {
    pub fn new(rx: Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            remainder: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for FrameReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Refill from the channel if the current chunk is exhausted. Loop so that
        // empty chunks are skipped instead of being reported as EOF.
        while self.pos >= self.remainder.len() {
            match self.rx.recv() {
                Ok(chunk) => {
                    self.remainder = chunk;
                    self.pos = 0;
                }
                Err(_) => return Ok(0), // sender dropped, nothing buffered → EOF
            }
        }

        let available = &self.remainder[self.pos..];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.pos += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn streams_chunks_in_order_then_eof() {
        let (tx, rx) = channel();
        tx.send(vec![1, 2, 3]).unwrap();
        tx.send(vec![4, 5]).unwrap();
        drop(tx);

        let mut r = FrameReader::new(rx);
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn retains_remainder_when_buf_smaller_than_chunk() {
        let (tx, rx) = channel();
        tx.send(vec![10, 20, 30, 40]).unwrap();
        drop(tx);

        let mut r = FrameReader::new(rx);
        let mut small = [0u8; 3];
        let n = r.read(&mut small).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&small[..3], &[10, 20, 30]);

        // Next read picks up the retained remainder.
        let n = r.read(&mut small).unwrap();
        assert_eq!(n, 1);
        assert_eq!(&small[..1], &[40]);
    }

    #[test]
    fn eof_on_closed_empty_channel() {
        let (tx, rx) = channel::<Vec<u8>>();
        drop(tx);
        let mut r = FrameReader::new(rx);
        let mut buf = [0u8; 8];
        assert_eq!(r.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn skips_empty_chunks() {
        let (tx, rx) = channel();
        tx.send(vec![]).unwrap();
        tx.send(vec![7]).unwrap();
        drop(tx);

        let mut r = FrameReader::new(rx);
        let mut buf = [0u8; 8];
        let n = r.read(&mut buf).unwrap();
        assert_eq!(n, 1);
        assert_eq!(buf[0], 7);
    }
}
