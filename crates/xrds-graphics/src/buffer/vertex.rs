use crate::XrdsGraphicsError;

use super::{XrdsBufferType, XrdsBufferView};

#[derive(Debug, Clone, Default)]
pub struct XrdsVertexInputType {
    pub normal: bool,
    pub tangent: bool,
    pub texcoords_n: u32,
    pub colors_n: u32,
    pub joints_n: u32,
    pub weights_n: u32,
}

#[derive(Debug, Clone)]
pub enum XrdsVertexBuffer {
    Discrete(DiscreteVertex),
    Linear(LinearVertex),
}

#[derive(Debug, Clone)]
pub struct DiscreteVertex {
    position: XrdsBufferView,
    normal: Option<XrdsBufferView>,
    tangent: Option<XrdsBufferView>,
    texcoords: Vec<XrdsBufferView>,
    colors: Vec<XrdsBufferView>,
    joints: Vec<XrdsBufferView>,
    weights: Vec<XrdsBufferView>,
    vertex_input_type: XrdsVertexInputType,
    vertex_attributes: Vec<wgpu::VertexAttribute>,
}

#[derive(Debug, Clone)]
pub struct LinearVertex {
    vertices: XrdsBufferView,
    vertex_input_type: XrdsVertexInputType,
}

impl PartialEq<XrdsVertexInputType> for XrdsVertexInputType {
    fn eq(&self, other: &XrdsVertexInputType) -> bool {
        (self.normal == other.normal)
            && (self.tangent == other.tangent)
            && (self.texcoords_n == other.texcoords_n)
            && (self.colors_n == other.colors_n)
            && (self.joints_n == other.joints_n)
            && (self.weights_n == other.weights_n)
    }
}

impl DiscreteVertex {
    pub fn new(
        position: XrdsBufferView,
        normal: Option<XrdsBufferView>,
        tangent: Option<XrdsBufferView>,
        texcoords: &[XrdsBufferView],
        colors: &[XrdsBufferView],
        joints: &[XrdsBufferView],
        weights: &[XrdsBufferView],
    ) -> Result<Self, XrdsGraphicsError> {
        let mut vertex_attributes = Vec::new();
        let mut vertex_input_type = XrdsVertexInputType::default();
        let (attr, mut last_offset, mut last_location) =
            Self::get_vertex_attr(position.ty(), wgpu::VertexFormat::Float32x3, 0u64, 0u32)?;
        vertex_attributes.push(attr);

        if let Some(view) = &normal {
            let (attr, offset, location) = Self::get_vertex_attr(
                view.ty(),
                wgpu::VertexFormat::Float32x3,
                last_offset,
                last_location,
            )?;
            vertex_attributes.push(attr);
            vertex_input_type.normal = true;
            last_offset = offset;
            last_location = location;
        }
        if let Some(view) = &tangent {
            let (attr, offset, location) = Self::get_vertex_attr(
                view.ty(),
                wgpu::VertexFormat::Float32x4,
                last_offset,
                last_location,
            )?;
            vertex_attributes.push(attr);
            vertex_input_type.tangent = true;
            last_offset = offset;
            last_location = location;
        }
        for view in texcoords {
            let (attr, offset, location) = Self::get_vertex_attr(
                view.ty(),
                wgpu::VertexFormat::Float32x2,
                last_offset,
                last_location,
            )?;
            vertex_attributes.push(attr);
            vertex_input_type.tangent = true;
            last_offset = offset;
            last_location = location;
        }
        let texcoords = texcoords.iter().map(|v| v.clone()).collect();
        for view in colors {
            let (attr, offset, location) = Self::get_vertex_attr(
                view.ty(),
                wgpu::VertexFormat::Float32x4,
                last_offset,
                last_location,
            )?;
            vertex_attributes.push(attr);
            vertex_input_type.tangent = true;
            last_offset = offset;
            last_location = location;
        }
        let colors = colors.iter().map(|v| v.clone()).collect();
        for view in joints {
            let (attr, offset, location) = Self::get_vertex_attr(
                view.ty(),
                wgpu::VertexFormat::Float32x4,
                last_offset,
                last_location,
            )?;
            vertex_attributes.push(attr);
            vertex_input_type.tangent = true;
            last_offset = offset;
            last_location = location;
        }
        let joints = joints.iter().map(|v| v.clone()).collect();
        for view in weights {
            let (attr, offset, location) = Self::get_vertex_attr(
                view.ty(),
                wgpu::VertexFormat::Float32x4,
                last_offset,
                last_location,
            )?;
            vertex_attributes.push(attr);
            vertex_input_type.tangent = true;
            last_offset = offset;
            last_location = location;
        }
        let weights = weights.iter().map(|v| v.clone()).collect();

        Ok(Self {
            position,
            normal,
            tangent,
            texcoords,
            colors,
            joints,
            weights,
            vertex_input_type,
            vertex_attributes,
        })
    }

    fn get_vertex_attr(
        ty: &XrdsBufferType,
        fmt: wgpu::VertexFormat,
        last_offset: u64,
        last_location: u32,
    ) -> Result<(wgpu::VertexAttribute, u64, u32), XrdsGraphicsError> {
        let res = if let XrdsBufferType::Vertex(v) = ty {
            if *v == fmt {
                (
                    wgpu::VertexAttribute {
                        format: *v,
                        offset: last_offset,
                        shader_location: last_location,
                    },
                    last_offset + fmt.size(),
                    last_location + 1,
                )
            } else {
                return Err(XrdsGraphicsError::VertexFormatMismatched {
                    fmt: *v,
                    expected: fmt,
                });
            }
        } else {
            return Err(XrdsGraphicsError::BufferTypeMismatched {
                ty: ty.clone(),
                expected: XrdsBufferType::Vertex(fmt),
            });
        };

        Ok(res)
    }

    pub fn vertex_input_type(&self) -> &XrdsVertexInputType {
        &self.vertex_input_type
    }

    pub fn to_vertex_layout(&self) -> wgpu::VertexBufferLayout {
        wgpu::VertexBufferLayout {
            array_stride: 0,
            attributes: &self.vertex_attributes,
            step_mode: wgpu::VertexStepMode::Vertex,
        }
    }

    pub fn position(&self) -> &XrdsBufferView {
        &self.position
    }

    pub fn normal(&self) -> Option<&XrdsBufferView> {
        self.normal.as_ref()
    }

    pub fn tangent(&self) -> Option<&XrdsBufferView> {
        self.tangent.as_ref()
    }

    pub fn texcoords(&self) -> Vec<&XrdsBufferView> {
        self.texcoords.iter().collect()
    }

    pub fn texcoords_n(&self, idx: usize) -> Option<&XrdsBufferView> {
        self.texcoords.get(idx)
    }

    pub fn colors(&self) -> Vec<&XrdsBufferView> {
        self.colors.iter().collect()
    }

    pub fn colors_n(&self, idx: usize) -> Option<&XrdsBufferView> {
        self.colors.get(idx)
    }

    pub fn joints(&self) -> Vec<&XrdsBufferView> {
        self.joints.iter().collect()
    }

    pub fn joints_n(&self, idx: usize) -> Option<&XrdsBufferView> {
        self.joints.get(idx)
    }

    pub fn weights(&self) -> Vec<&XrdsBufferView> {
        self.weights.iter().collect()
    }

    pub fn weights_n(&self, idx: usize) -> Option<&XrdsBufferView> {
        self.weights.get(idx)
    }
}

impl LinearVertex {
    pub fn vertices(&self) -> &XrdsBufferView {
        &self.vertices
    }

    pub fn vertex_input_type(&self) -> &XrdsVertexInputType {
        &self.vertex_input_type
    }
}

#[cfg(test)]
#[tokio::test]
async fn test_discrete_vertex_creation() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();
    let (device, _) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .unwrap();
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 128,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let position = XrdsBufferView::new(
        buffer.clone(),
        0,
        XrdsBufferType::Vertex(wgpu::VertexFormat::Float32x3),
        3,
    );
    let normal = XrdsBufferView::new(
        buffer.clone(),
        0,
        XrdsBufferType::Vertex(wgpu::VertexFormat::Float32x3),
        3,
    );
    let tangent = XrdsBufferView::new(
        buffer.clone(),
        0,
        XrdsBufferType::Vertex(wgpu::VertexFormat::Float32x4),
        3,
    );
    let texcoords_0 = XrdsBufferView::new(
        buffer.clone(),
        0,
        XrdsBufferType::Vertex(wgpu::VertexFormat::Float32x2),
        3,
    );
    let colors_0 = XrdsBufferView::new(
        buffer.clone(),
        0,
        XrdsBufferType::Vertex(wgpu::VertexFormat::Float32x4),
        3,
    );
    let joints_0 = XrdsBufferView::new(
        buffer.clone(),
        0,
        XrdsBufferType::Vertex(wgpu::VertexFormat::Float32x4),
        3,
    );
    let weights_0 = XrdsBufferView::new(
        buffer.clone(),
        0,
        XrdsBufferType::Vertex(wgpu::VertexFormat::Float32x4),
        3,
    );

    let discrete_vertex = DiscreteVertex::new(
        position,
        Some(normal),
        Some(tangent),
        &[texcoords_0],
        &[colors_0],
        &[joints_0],
        &[weights_0],
    )
    .unwrap();
    log::debug!("{:?}", discrete_vertex);

    let vertex_layout = discrete_vertex.to_vertex_layout();
    log::debug!("{:?}", vertex_layout);
}
