use std::{
    ffi::OsStr,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use wgpu::SamplerDescriptor;

use crate::AssetServer;

use super::{Gltf, GltfImage};

pub struct GltfLoader {
    asset_server: Arc<AssetServer>,
    asset_path: PathBuf,
}

impl GltfLoader {
    pub fn new(asset_server: Arc<AssetServer>, asset_path: &Path) -> Self {
        Self {
            asset_server,
            asset_path: asset_path.to_path_buf(),
        }
    }

    pub async fn load_from_file(&self, path: &Path) -> anyhow::Result<Gltf> {
        let gltf_file_name = path
            .file_name()
            .ok_or(anyhow::Error::msg("Invalid file name"))?
            .to_string_lossy();
        let buf = Self::read_file(path).await?;
        self.load(&buf, &gltf_file_name).await
    }

    pub async fn load(&self, data: &[u8], name: &str) -> anyhow::Result<Gltf> {
        let gltf = gltf::Gltf::from_slice(data)?;

        let gltf_key = self.asset_path.join(name).to_string_lossy().to_string();
        log::debug!("Load gltf {}", &gltf_key);

        // Phase1. load gltf files into gltf structure
        let instant = Instant::now();
        let buffers: Vec<_> = gltf.buffers().collect();
        let raw_buffers: Vec<_> = buffers
            .par_iter()
            .map(|buffer| Self::load_buffer(&gltf, buffer, &self.asset_path))
            .filter_map(|res| res.ok())
            .collect();
        log::debug!(
            "Load {} raw buffers in {} secs",
            raw_buffers.len(),
            instant.elapsed().as_secs_f32()
        );
        let instant = Instant::now();
        let images: Vec<_> = gltf.images().collect();
        let raw_images: Vec<_> = images
            .par_iter()
            .map(|image| Self::load_image(image, &raw_buffers, &self.asset_path, &gltf_key))
            .filter_map(|res| res.ok())
            .collect();
        log::debug!(
            "Load {} raw images in {} secs",
            raw_images.len(),
            instant.elapsed().as_secs_f32()
        );
        let instant = Instant::now();
        let samplers: Vec<_> = gltf.samplers().collect();
        let sampler_descriptors: Vec<_> = samplers
            .iter()
            .map(|sampler| Self::load_sampler_descriptor(sampler))
            .collect();
        let default_sampler_descriptor = wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            min_filter: wgpu::FilterMode::Nearest,
            mag_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        };
        log::debug!(
            "Load {} sampler description in {} secs",
            sampler_descriptors.len(),
            instant.elapsed().as_secs_f32()
        );
        let textures: Vec<_> = gltf.textures().collect();
        textures.iter().map(|texture| {
            let index = texture.index();
            let name = texture.name();
            let image_index = texture.source().index();
        });

        Ok(Gltf::default())
    }

    fn load_sampler_descriptor<'a>(sampler: &'a gltf::texture::Sampler) -> SamplerDescriptor<'a> {
        let to_address_mode = |wrapping_mode: gltf::texture::WrappingMode| match wrapping_mode {
            gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
            gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
        };
        let address_mode_u = to_address_mode(sampler.wrap_s());
        let address_mode_v = to_address_mode(sampler.wrap_t());
        let mag_filter = if let Some(sampler_mag) = sampler.mag_filter() {
            match sampler_mag {
                gltf::texture::MagFilter::Linear => wgpu::FilterMode::Linear,
                gltf::texture::MagFilter::Nearest => wgpu::FilterMode::Nearest,
            }
        } else {
            wgpu::FilterMode::Nearest
        };
        let (min_filter, mipmap_filter) = if let Some(sampler_min) = sampler.min_filter() {
            match sampler_min {
                gltf::texture::MinFilter::Linear => {
                    (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::LinearMipmapLinear => {
                    (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::LinearMipmapNearest => {
                    (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
                }
                gltf::texture::MinFilter::Nearest => {
                    (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::NearestMipmapLinear => {
                    (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear)
                }
                gltf::texture::MinFilter::NearestMipmapNearest => {
                    (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest)
                }
            }
        } else {
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest)
        };
        wgpu::SamplerDescriptor {
            label: sampler.name(),
            address_mode_u,
            address_mode_v,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        }
    }

    fn load_buffer(
        gltf: &gltf::Gltf,
        buffer: &gltf::Buffer,
        asset_path: &Path,
    ) -> anyhow::Result<Vec<u8>> {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => {
                if let Some(blob) = gltf.blob.as_deref() {
                    blob.to_vec()
                } else {
                    anyhow::bail!("Buffer reference blob but gltf file not has blob data")
                }
            }
            gltf::buffer::Source::Uri(uri) => {
                let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
                if let Ok(uri) = UriData::parse(&decoded_uri) {
                    uri.data
                } else {
                    let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
                    let path = asset_path.to_path_buf().join(decoded_uri.to_string());
                    let mut file = File::open(path)?;
                    let mut res = Vec::new();
                    file.read_to_end(&mut res)?;

                    res
                }
            }
        };

        Ok(data)
    }

    fn load_image(
        image: &gltf::Image<'_>,
        raw_buffers: &[Vec<u8>],
        asset_path: &Path,
        super_key: &str,
    ) -> anyhow::Result<GltfImage> {
        let (name, loaded_image) = match image.source() {
            gltf::image::Source::View { view, mime_type: _ } => {
                let buffer_index = view.buffer().index();
                let buffer = &raw_buffers[buffer_index];
                let offset = view.offset();
                let length = view.length();
                let slice = &buffer[offset..offset + length];

                let name = if let Some(name) = view.name() {
                    format!("{}.{}", super_key, name)
                } else {
                    format!("{}.image.{}", super_key, image.index())
                };

                (name, image::load_from_memory(slice)?)
            }
            gltf::image::Source::Uri { uri, mime_type: _ } => {
                let decoded_uri = percent_encoding::percent_decode_str(uri).decode_utf8()?;
                let blob = if let Ok(uri) = UriData::parse(&decoded_uri) {
                    // data in uri
                    uri.data
                } else {
                    // path
                    let path = asset_path.to_path_buf().join(decoded_uri.to_string());
                    let mut file = File::open(path)?;
                    let mut res = Vec::new();
                    file.read_to_end(&mut res)?;

                    res
                };
                let name = format!("{}.{}", super_key, decoded_uri);
                (name, image::load_from_memory(&blob)?)
            }
        };

        Ok(GltfImage {
            name,
            index: image.index(),
            data: loaded_image.to_rgba8().into_vec(),
            width: loaded_image.width(),
            height: loaded_image.height(),
        })
    }

    async fn read_file(path: &Path) -> anyhow::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut f = File::open(path)?;
        f.read_to_end(&mut buf)?;

        Ok(buf)
    }
}

struct UriData<'a> {
    mime_type: &'a str,
    data: Vec<u8>,
}

impl<'a> UriData<'a> {
    fn parse(decoded_uri: &'a str) -> anyhow::Result<UriData<'a>> {
        if let Some(striped_uri) = decoded_uri.strip_prefix("data:") {
            if let Some((mime_type, data)) = striped_uri.split_once(',') {
                let (mime_type, is_base64) = match mime_type.strip_suffix(";base64") {
                    Some(m) => (m, true),
                    None => (mime_type, false),
                };
                let data = if is_base64 {
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)?
                } else {
                    data.as_bytes().to_owned()
                };
                Ok(UriData { mime_type, data })
            } else {
                anyhow::bail!("Invalid data uri format")
            }
        } else {
            // TODO: Define error
            anyhow::bail!("Not a data uri")
        }
    }
}
