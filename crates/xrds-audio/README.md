# XRDS Audio

Device-level spatial audio for XRDS applications — the **expert layer** for audio.

> **Which audio path do I want?**
>
> For an authored scene, you almost certainly want neither this crate nor rodio directly:
> `XrdsSceneAudioClip` drives Bevy's spatial audio (every XRDS camera is a listener), and that
> is what the scene document serializes and what the editor exposes.
>
> Reach for this crate when you need what Bevy does not offer: **choosing the output device**,
> or **driving emitter and per-ear listener positions yourself**.
>
> Adopted from the `init-spatial-audio` branch (April 2025), which built it standalone before
> the Bevy-based path existed.

## Requirements

XRDS audio works in enviroments supported rodio and cpal library.

XRDS Audio plays WAV audio streams stored either through file I/O or in a memory buffer.
The audio stream can be played back all at once or in real-time by continuously accumulating data into the buffer using a custom BufReader.
It also supports spatial audio playback by receiving 3D coordinate inputs in real time.

## Support Protocols

- Wav

## Platform/Architecture

- Windows arm/arm64
- Linux x86/x64

## Dependencies

### rodio
- rodio: 0.20.1
- https://docs.rs/rodio/

### cpal
- cpal: 0.15.3
- https://docs.rs/cpal/

### anyhow
- anyhow: 1.0.95
- https://docs.rs/anyhow/

### log
- log: 0.4.25
- https://docs.rs/log/