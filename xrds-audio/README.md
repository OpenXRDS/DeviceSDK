# XRDS Audio

Audio renderer for XRDS application

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