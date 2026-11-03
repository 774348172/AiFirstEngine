use super::*;
use std::hash::{Hash, Hasher};

/// Immutable verified byte product. Deserialization validates before it can reach RHI.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "BufferWire")]
pub struct GeometryBuffers {
    wire: Arc<BufferWire>,
    content_hash: u64,
    byte_identity: [u8; 32],
    upload_source: Option<Arc<super::mesh_residency::PackageMeshBytes>>,
}

impl Serialize for GeometryBuffers {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // The wire remains a complete validated mesh, even after CPU eviction.
        let source = self
            .acquire_upload_source()
            .map_err(serde::ser::Error::custom)?;
        source.wire.serialize(serializer)
    }
}

impl PartialEq for GeometryBuffers {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.wire, &other.wire)
            || (self.same_identity(other)
                && (self.upload_source.is_some()
                    || other.upload_source.is_some()
                    || self.wire == other.wire))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_fingerprints_never_admit_different_mesh_bytes() {
        let (_, source, _, _) = super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let original = source.buffers.as_ref();
        let mut wire = original.wire.as_ref().clone();
        let x = f32::from_le_bytes(wire.vertex_bytes[..4].try_into().unwrap()) + 0.125;
        Arc::make_mut(&mut wire.vertex_bytes)[..4].copy_from_slice(&x.to_le_bytes());
        let mut different = GeometryBuffers::try_from(wire).unwrap();
        assert_ne!(original, &different);
        different.content_hash = original.content_hash;
        assert_ne!(
            original, &different,
            "hash collision replaced full equality"
        );
        let detached = GeometryBuffers::try_from(original.wire.as_ref().clone()).unwrap();
        assert_eq!(original, &detached);
        assert!(!Arc::ptr_eq(&original.wire, &detached.wire));
    }
}
impl Eq for GeometryBuffers {}
impl Hash for GeometryBuffers {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.content_hash.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BufferWire {
    vertex_bytes: Arc<Vec<u8>>,
    index_bytes: Arc<Vec<u8>>,
    vertex_stride: u32,
    vertex_count: u32,
    index_count: u32,
    index_format: GeometryIndexFormat,
    attributes: Vec<VertexAttribute>,
}

impl TryFrom<BufferWire> for GeometryBuffers {
    type Error = ContractError;
    fn try_from(w: BufferWire) -> Result<Self, Self::Error> {
        // Reuse U1's complete layout/index/finite validator; bounds are reconstructed
        // from positions solely for validation and are not a second culling truth.
        if w.vertex_stride == 0
            || w.vertex_bytes.len() as u64 != u64::from(w.vertex_stride) * u64::from(w.vertex_count)
        {
            return Err(invalid("geometry.bytes"));
        }
        let position = w
            .attributes
            .iter()
            .find(|a| a.semantic == VertexSemantic::Position)
            .ok_or_else(|| invalid("geometry.position"))?;
        if position
            .offset
            .checked_add(12)
            .is_none_or(|end| end > w.vertex_stride)
        {
            return Err(invalid("geometry.position"));
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in w.vertex_bytes.chunks_exact(w.vertex_stride as usize) {
            for i in 0..3 {
                let offset = position.offset as usize + i * 4;
                let value = f32::from_le_bytes(v[offset..offset + 4].try_into().unwrap());
                min[i] = min[i].min(value);
                max[i] = max[i].max(value);
            }
        }
        let center: [f32; 3] = std::array::from_fn(|i| min[i] * 0.5 + max[i] * 0.5);
        let radius = ((0..3)
            .map(|i| (f64::from(max[i]) - f64::from(center[i])).powi(2))
            .sum::<f64>())
        .sqrt() as f32;
        let data = CookedGeometry {
            vertex_bytes: w.vertex_bytes.as_ref().clone(),
            index_bytes: w.index_bytes.as_ref().clone(),
            vertex_stride: w.vertex_stride,
            attributes: w.attributes.clone(),
            vertex_count: w.vertex_count,
            index_count: w.index_count,
            index_format: w.index_format,
            topology: GeometryTopology::TriangleList,
            submeshes: vec![SubmeshRange {
                first_index: 0,
                index_count: w.index_count,
                material_slot: "default".into(),
            }],
            bounds: GeometryBounds {
                min,
                max,
                sphere_center: center,
                sphere_radius: radius,
            },
            coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
            material_slots: vec!["default".into()],
        };
        data.validate()?;
        Ok(Self::verified(w))
    }
}

#[cfg(test)]
pub(crate) fn package_streamable_for_test(
    source: &Arc<GeometryBuffers>,
    loader: &crate::runtime_asset_loader::RuntimeAssetLoader,
    provenance: &Provenance,
) -> Arc<GeometryBuffers> {
    GeometryBuffers::make_package_streamable(source, loader, provenance).unwrap()
}

impl GeometryBuffers {
    pub(super) fn validate_standard_lit_inputs(
        &self,
        range: std::ops::Range<u32>,
        normal_map: bool,
        textured: bool,
    ) -> Result<(), ContractError> {
        VerifiedMaterialGeometry {
            vertex_bytes: self.vertex_bytes(),
            index_bytes: self.index_bytes(),
            vertex_stride: self.vertex_stride(),
            attributes: self.attributes(),
            index_format: self.index_format(),
            index_count: self.index_count(),
        }
        .validate(range, normal_map, textured)
    }
    /// Retain an explicit CPU source while an upload or CPU reader consumes it.
    #[doc(hidden)]
    pub fn acquire_upload_source(&self) -> Result<Arc<Self>, String> {
        match &self.upload_source {
            Some(source) => source.acquire(),
            None => Ok(Arc::new(self.clone())),
        }
    }

    #[doc(hidden)]
    pub fn release_upload_source(&self) {
        if let Some(source) = &self.upload_source {
            source.release();
        }
    }

    pub(super) fn same_identity(&self, other: &Self) -> bool {
        self.content_hash == other.content_hash
            && self.byte_identity == other.byte_identity
            && self.wire.vertex_stride == other.wire.vertex_stride
            && self.wire.vertex_count == other.wire.vertex_count
            && self.wire.index_count == other.wire.index_count
            && self.wire.index_format == other.wire.index_format
            && self.wire.attributes == other.wire.attributes
    }

    pub(super) fn without_bytes(&self) -> Self {
        let mut descriptor = self.clone();
        let mut wire = self.wire.as_ref().clone();
        wire.vertex_bytes = Arc::default();
        wire.index_bytes = Arc::default();
        descriptor.wire = Arc::new(wire);
        descriptor.upload_source = None;
        descriptor
    }

    pub(super) fn make_package_streamable(
        source: &Arc<Self>,
        loader: &crate::runtime_asset_loader::RuntimeAssetLoader,
        provenance: &Provenance,
    ) -> Option<Arc<Self>> {
        let origin = super::mesh_residency::PackageMeshBytes::new(loader, source, provenance)?;
        let mut descriptor = source.without_bytes();
        descriptor.upload_source = Some(Arc::new(origin));
        Some(Arc::new(descriptor))
    }

    #[doc(hidden)]
    pub fn vertex_storage(&self) -> &Arc<Vec<u8>> {
        &self.wire.vertex_bytes
    }

    pub(super) fn index_storage(&self) -> &Arc<Vec<u8>> {
        &self.wire.index_bytes
    }

    #[doc(hidden)]
    pub fn observe_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        sample.cpu_arc(&self.wire, "assets", &[role]);
        sample.cpu_arc(&self.wire.vertex_bytes, "assets", &[role]);
        sample.cpu_arc(&self.wire.index_bytes, "assets", &[role]);
        sample.cpu_vec(&self.wire.vertex_bytes, "assets", &["decodedCache", role]);
        sample.cpu_vec(&self.wire.index_bytes, "assets", &["decodedCache", role]);
        sample.cpu_vec(&self.wire.attributes, "assets", &["decodedCache", role]);
        if let Some(source) = &self.upload_source {
            sample.cpu_arc(source, "assets", &[role]);
            source.observe_memory(sample, role);
        }
        sample.partial("cpu.owner.assets", "geometry.sourceBacking.capacity");
    }

    fn verified(wire: BufferWire) -> Self {
        use sha2::{Digest, Sha256};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        wire.hash(&mut hash);
        let mut identity = Sha256::new();
        identity.update((wire.vertex_bytes.len() as u64).to_le_bytes());
        identity.update(wire.vertex_bytes.as_slice());
        identity.update((wire.index_bytes.len() as u64).to_le_bytes());
        identity.update(wire.index_bytes.as_slice());
        Self {
            wire: Arc::new(wire),
            content_hash: hash.finish(),
            byte_identity: identity.finalize().into(),
            upload_source: None,
        }
    }
    pub fn from_cooked(g: &CookedGeometry) -> Result<Self, ContractError> {
        g.validate()?;
        Ok(Self::verified(BufferWire {
            vertex_bytes: g.vertex_bytes.clone().into(),
            index_bytes: g.index_bytes.clone().into(),
            vertex_stride: g.vertex_stride,
            attributes: g.attributes.clone(),
            vertex_count: g.vertex_count,
            index_count: g.index_count,
            index_format: g.index_format,
        }))
    }
    /// Consume a validated CPU candidate without retaining a duplicate vertex/index allocation.
    pub(crate) fn from_cooked_owned(g: CookedGeometry) -> Result<Self, ContractError> {
        g.validate()?;
        Ok(Self::verified(BufferWire {
            vertex_bytes: g.vertex_bytes.into(),
            index_bytes: g.index_bytes.into(),
            vertex_stride: g.vertex_stride,
            attributes: g.attributes,
            vertex_count: g.vertex_count,
            index_count: g.index_count,
            index_format: g.index_format,
        }))
    }

    /// Fixed-layout replacement: original validated topology and index backing remain shared.
    pub(super) fn with_vertex_bytes(&self, vertex_bytes: Vec<u8>) -> Result<Self, ContractError> {
        if self.upload_source.is_some() || vertex_bytes.len() != self.vertex_byte_len() {
            return Err(invalid("geometry.vertexBytes"));
        }
        for vertex in vertex_bytes.chunks_exact(self.wire.vertex_stride as usize) {
            for attribute in &self.wire.attributes {
                let width = match attribute.format {
                    VertexFormat::Float32x2 => 8,
                    VertexFormat::Float32x3 => 12,
                    VertexFormat::Float32x4 => 16,
                };
                let start = attribute.offset as usize;
                if vertex[start..start + width].chunks_exact(4).any(|bytes| {
                    !f32::from_le_bytes(bytes.try_into().expect("validated float layout"))
                        .is_finite()
                }) {
                    return Err(invalid("geometry.vertexBytes.nonFinite"));
                }
            }
        }
        let mut wire = self.wire.as_ref().clone();
        wire.vertex_bytes = Arc::new(vertex_bytes);
        Ok(Self::verified(wire))
    }
    /// Ordinary buffers contain bytes. Production package descriptors require an
    /// explicit `acquire_upload_source` lease before reading CPU bytes.
    pub fn vertex_bytes(&self) -> &[u8] {
        &self.wire.vertex_bytes
    }
    pub fn index_bytes(&self) -> &[u8] {
        &self.wire.index_bytes
    }
    #[inline]
    pub fn vertex_byte_len(&self) -> usize {
        self.wire.vertex_stride as usize * self.wire.vertex_count as usize
    }
    #[inline]
    pub fn index_byte_len(&self) -> usize {
        self.wire.index_format.byte_len() * self.wire.index_count as usize
    }
    #[inline]
    pub fn vertex_stride(&self) -> u32 {
        self.wire.vertex_stride
    }
    #[inline]
    pub fn vertex_count(&self) -> u32 {
        self.wire.vertex_count
    }
    #[inline]
    pub fn index_count(&self) -> u32 {
        self.wire.index_count
    }
    #[inline]
    pub fn index_format(&self) -> GeometryIndexFormat {
        self.wire.index_format
    }
    #[inline]
    pub fn attributes(&self) -> &[VertexAttribute] {
        &self.wire.attributes
    }
}
