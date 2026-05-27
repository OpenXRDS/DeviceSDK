use crate::tessellation::MeshData;
use serde_json::{json, Value};

/// Indices into the glTF accessors array for one mesh primitive.
pub struct MeshAccessors {
    pub position: usize,
    pub normal: usize,
    pub texcoord: usize,
    pub indices: usize,
}

/// Accumulates all binary geometry into one flat byte buffer, emitting
/// matching glTF `bufferView` and `accessor` JSON objects.
#[derive(Default)]
pub struct BufferBuilder {
    pub bytes: Vec<u8>,
    pub buffer_views: Vec<Value>,
    pub accessors: Vec<Value>,
}

impl BufferBuilder {
    pub fn push_mesh(&mut self, mesh: MeshData) -> MeshAccessors {
        let (mn, mx) = mesh.position_min_max();
        let position = self.push_f32_vec3(&mesh.pos, Some((mn, mx)));
        let normal   = self.push_f32_vec3(&mesh.nor, None);
        let texcoord = self.push_f32_vec2(&mesh.uvs);
        let indices  = self.push_u32_indices(&mesh.idx);
        MeshAccessors { position, normal, texcoord, indices }
    }

    fn push_f32_vec3(&mut self, data: &[f32], min_max: Option<([f32;3],[f32;3])>) -> usize {
        let offset = self.aligned_offset(4);
        self.bytes.extend(data.iter().flat_map(|v| v.to_le_bytes()));
        let view_idx = self.push_view(offset, data.len() * 4, None, 34962); // ARRAY_BUFFER
        let count = data.len() / 3;
        let mut acc = json!({
            "bufferView": view_idx,
            "byteOffset": 0,
            "componentType": 5126, // FLOAT
            "count": count,
            "type": "VEC3"
        });
        if let Some((mn, mx)) = min_max {
            acc["min"] = json!([mn[0], mn[1], mn[2]]);
            acc["max"] = json!([mx[0], mx[1], mx[2]]);
        }
        let idx = self.accessors.len();
        self.accessors.push(acc);
        idx
    }

    fn push_f32_vec2(&mut self, data: &[f32]) -> usize {
        let offset = self.aligned_offset(4);
        self.bytes.extend(data.iter().flat_map(|v| v.to_le_bytes()));
        let view_idx = self.push_view(offset, data.len() * 4, None, 34962);
        let idx = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view_idx,
            "byteOffset": 0,
            "componentType": 5126,
            "count": data.len() / 2,
            "type": "VEC2"
        }));
        idx
    }

    fn push_u32_indices(&mut self, data: &[u32]) -> usize {
        let offset = self.aligned_offset(4);
        self.bytes.extend(data.iter().flat_map(|v| v.to_le_bytes()));
        let view_idx = self.push_view(offset, data.len() * 4, None, 34963); // ELEMENT_ARRAY_BUFFER
        let idx = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view_idx,
            "byteOffset": 0,
            "componentType": 5125, // UNSIGNED_INT
            "count": data.len(),
            "type": "SCALAR"
        }));
        idx
    }

    fn push_view(&mut self, offset: usize, byte_len: usize, stride: Option<usize>, target: u32) -> usize {
        let idx = self.buffer_views.len();
        let mut v = json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": byte_len,
            "target": target
        });
        if let Some(s) = stride { v["byteStride"] = json!(s); }
        self.buffer_views.push(v);
        idx
    }

    /// Advance the buffer to the next 4-byte boundary and return the new offset.
    fn aligned_offset(&mut self, align: usize) -> usize {
        let rem = self.bytes.len() % align;
        if rem != 0 {
            let pad = align - rem;
            self.bytes.extend(std::iter::repeat(0u8).take(pad));
        }
        self.bytes.len()
    }
}
