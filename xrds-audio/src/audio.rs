use std::io::{BufReader, Read, Seek};

use cpal::traits::HostTrait;
use rodio::{DeviceTrait, OutputStream, OutputStreamHandle, Source, SpatialSink};

pub struct AudioDevice {
    pub name: String,
    device: cpal::Device,
}

pub struct SpatialAudio {
    _stream: OutputStream,
    _handle: OutputStreamHandle,
    sink: SpatialSink,
}

impl SpatialAudio {
    pub fn new<R: Read + Seek + Send + Sync + 'static>(
        emitter_position: [f32; 3],
        listener_left_position: [f32; 3],
        listener_right_position: [f32; 3],
        audio_device: &AudioDevice,
        bufreader: BufReader<R>,
    ) -> anyhow::Result<Self> {
        let (stream, handle) = rodio::OutputStream::try_from_device(&audio_device.device)?;
        let sink = rodio::SpatialSink::try_new(
            &handle,
            emitter_position,
            listener_left_position,
            listener_right_position,
        )?;

        let source: rodio::source::Repeat<rodio::Decoder<BufReader<R>>> =
            rodio::Decoder::new(bufreader)?.repeat_infinite();

        sink.append(source);
        sink.pause();

        Ok(Self {
            _stream: stream,
            _handle: handle,
            sink,
        })
    }

    pub fn get_device_list() -> anyhow::Result<Vec<AudioDevice>> {
        let mut device_list: Vec<AudioDevice> = Vec::new();

        let available_hosts = cpal::available_hosts();
        for host_id in available_hosts {
            let host = cpal::host_from_id(host_id)?;
            let devices: std::iter::Filter<cpal::Devices, fn(&cpal::Device) -> bool> =
                host.output_devices()?;

            for device in devices {
                device_list.push(AudioDevice {
                    name: device.name()?,
                    device,
                });
            }
        }

        Ok(device_list)
    }

    pub fn print_available_devices() -> anyhow::Result<()> {
        println!("Supports hosts: {:?}", cpal::ALL_HOSTS);
        let available_hosts = cpal::available_hosts();
        println!("Available hosts: {:?}\n", available_hosts);

        let mut host_index = 1;
        for host_id in available_hosts {
            println!("{}. {}", host_index, host_id.name());

            let host = cpal::host_from_id(host_id)?;
            let default_device = host.default_output_device().map(|e| e.name().unwrap());
            println!("  Default device: {:?}", default_device);

            let devices = host.devices()?;
            for (device_index, device) in devices.enumerate() {
                println!(
                    "\n  {}.{}. {}",
                    host_index,
                    device_index + 1,
                    device.name()?
                );

                if let Ok(conf) = device.default_input_config() {
                    println!("    Default input stream config: {:?}", conf);
                }

                let input_configs = match device.supported_input_configs() {
                    Ok(f) => f.collect(),
                    Err(e) => {
                        log::error!("Error getting supported input configs: {:?}", e);
                        Vec::new()
                    }
                };

                if !input_configs.is_empty() {
                    for config in input_configs {
                        println!("    {:?}", config);
                    }
                }

                if let Ok(conf) = device.default_output_config() {
                    println!("    Default output stream config: {:?}", conf);
                }

                let output_configs = match device.supported_output_configs() {
                    Ok(f) => f.collect(),
                    Err(e) => {
                        log::error!("Error getting supported output configs: {:?}", e);
                        Vec::new()
                    }
                };

                if !output_configs.is_empty() {
                    for config in output_configs {
                        println!("    {:?}", config);
                    }
                }
            }

            host_index += 1;
        }
        println!();

        Ok(())
    }

    pub fn play(&self) {
        if self.sink.is_paused() {
            self.sink.play();
        }
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    pub fn stop(&self) {
        self.sink.stop();
    }

    pub fn set_emitter_position(&self, position: [f32; 3]) {
        self.sink.set_emitter_position(position);
    }

    pub fn set_left_ear_position(&self, position: [f32; 3]) {
        self.sink.set_left_ear_position(position);
    }

    pub fn set_right_ear_position(&self, position: [f32; 3]) {
        self.sink.set_right_ear_position(position);
    }

    pub fn set_speed(&self, speed: f32) {
        self.sink.set_speed(speed);
    }

    pub fn set_volume(&self, volume: f32) {
        self.sink.set_volume(volume);
    }
}
