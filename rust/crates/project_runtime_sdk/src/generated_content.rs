//! Project-owned geometry in bounded session messages. These are data, never GPU/native handles.
use crate::{ProjectRuntimeCellEntity, ProjectRuntimeSpatialAssetRef};
use serde::{Deserialize, Serialize};
mod query;
pub use query::*;

pub const GENERATED_CONTENT_SCHEMA: &str = "generated-content.v1;session-bulk.v2;triangle-list;fixed-layout-patch;static-collision;latest-admitted;opaque-project-bytes;envelope<=8388608;receipts<=128";
pub const GENERATED_RECEIPT_LIMIT: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Attribute3D {
    Position,
    Normal,
    TexCoord0,
    Color0,
}
impl Attribute3D {
    pub const fn components(self) -> usize {
        match self {
            Self::Position | Self::Normal => 3,
            Self::TexCoord0 => 2,
            Self::Color0 => 4,
        }
    }
    pub const fn byte_len(self) -> usize {
        self.components() * 4
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexFormat3D {
    Uint16,
    Uint32,
}
impl IndexFormat3D {
    pub const fn byte_len(self) -> usize {
        match self {
            Self::Uint16 => 2,
            Self::Uint32 => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeLayout3D {
    pub semantic: Attribute3D,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeometrySection3D {
    pub first_index: u32,
    pub index_count: u32,
    pub material_slot: String,
    pub material: ProjectRuntimeSpatialAssetRef,
}

/// TriangleList, right-handed Y-up metres, camera forward -Z, matching CookedGeometry.
/// Payloads are binary attachments; serde only carries their bounded descriptors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry3D {
    pub vertex_stride: u32,
    pub vertex_count: u32,
    pub index_count: u32,
    pub index_format: IndexFormat3D,
    pub attributes: Vec<AttributeLayout3D>,
    pub sections: Vec<GeometrySection3D>,
    #[serde(skip)]
    pub vertex_bytes: Vec<u8>,
    #[serde(skip)]
    pub index_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentError {
    pub code: ContentErrorCode,
    pub field_path: String,
    pub request_id: u64,
    pub expected: Option<u64>,
    pub actual: Option<u64>,
    pub next_action: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentErrorCode {
    InvalidArgument,
    BudgetExceeded,
    RevisionConflict,
    StaleTarget,
    StaleSession,
    PendingUpdate,
    ReceiptExpired,
    AlreadyApplied,
    UnsupportedMotionType,
    CapabilityGap,
    StorageBusy,
    NotFound,
    CommitUnknown,
}
impl ContentError {
    pub fn invalid(field_path: impl Into<String>) -> Self {
        Self {
            code: ContentErrorCode::InvalidArgument,
            field_path: field_path.into(),
            request_id: 0,
            expected: None,
            actual: None,
            next_action: "Correct the indicated field and submit a new request.".into(),
        }
    }
}
impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} request_id={} field_path={} expected={:?} actual={:?}: {}",
            self.code,
            self.request_id,
            self.field_path,
            self.expected,
            self.actual,
            self.next_action
        )
    }
}
impl std::error::Error for ContentError {}

/// Safe array builder: no project code calculates stride, offsets, attachment lengths or counts.
pub struct GeometryBuilder3D<'a> {
    positions: &'a [[f32; 3]],
    normals: Option<&'a [[f32; 3]]>,
    uv: Option<&'a [[f32; 2]]>,
    colors: Option<&'a [[f32; 4]]>,
}
impl<'a> GeometryBuilder3D<'a> {
    pub fn new(positions: &'a [[f32; 3]]) -> Self {
        Self {
            positions,
            normals: None,
            uv: None,
            colors: None,
        }
    }
    pub fn normals(mut self, values: &'a [[f32; 3]]) -> Self {
        self.normals = Some(values);
        self
    }
    pub fn uv(mut self, values: &'a [[f32; 2]]) -> Self {
        self.uv = Some(values);
        self
    }
    pub fn colors(mut self, values: &'a [[f32; 4]]) -> Self {
        self.colors = Some(values);
        self
    }
    pub fn triangles_u16(
        self,
        indices: &[u16],
        sections: Vec<GeometrySection3D>,
    ) -> Result<Geometry3D, ContentError> {
        self.check_size(indices.len(), 2)?;
        self.build(
            indices.len(),
            IndexFormat3D::Uint16,
            indices.iter().flat_map(|v| v.to_le_bytes()).collect(),
            sections,
        )
    }
    pub fn triangles_u32(
        self,
        indices: &[u32],
        sections: Vec<GeometrySection3D>,
    ) -> Result<Geometry3D, ContentError> {
        self.check_size(indices.len(), 4)?;
        self.build(
            indices.len(),
            IndexFormat3D::Uint32,
            indices.iter().flat_map(|v| v.to_le_bytes()).collect(),
            sections,
        )
    }
    fn check_size(&self, index_count: usize, index_stride: usize) -> Result<(), ContentError> {
        let stride = 12
            + usize::from(self.normals.is_some()) * 12
            + usize::from(self.uv.is_some()) * 8
            + usize::from(self.colors.is_some()) * 16;
        let len = self
            .positions
            .len()
            .checked_mul(stride)
            .and_then(|n| {
                index_count
                    .checked_mul(index_stride)
                    .and_then(|i| n.checked_add(i))
            })
            .ok_or_else(|| ContentError::invalid("geometry.bytes"))?;
        if len > crate::spatial_bulk::MAX_SESSION_BYTES {
            return Err(ContentError::invalid("geometry.bytes"));
        }
        Ok(())
    }
    fn build(
        self,
        count: usize,
        index_format: IndexFormat3D,
        index_bytes: Vec<u8>,
        sections: Vec<GeometrySection3D>,
    ) -> Result<Geometry3D, ContentError> {
        let mut streams: Vec<(Attribute3D, &[f32])> =
            vec![(Attribute3D::Position, self.positions.as_flattened())];
        if let Some(v) = self.normals {
            streams.push((Attribute3D::Normal, v.as_flattened()));
        }
        if let Some(v) = self.uv {
            streams.push((Attribute3D::TexCoord0, v.as_flattened()));
        }
        if let Some(v) = self.colors {
            streams.push((Attribute3D::Color0, v.as_flattened()));
        }
        let mut stride = 0u32;
        let mut attributes = Vec::new();
        for (semantic, stream) in &streams {
            if stream.len()
                != self
                    .positions
                    .len()
                    .checked_mul(semantic.components())
                    .ok_or_else(|| ContentError::invalid("geometry.vertexCount"))?
            {
                return Err(ContentError::invalid("geometry.attributes.count"));
            }
            attributes.push(AttributeLayout3D {
                semantic: *semantic,
                offset: stride,
            });
            stride += semantic.byte_len() as u32;
        }
        let len = self
            .positions
            .len()
            .checked_mul(stride as usize)
            .ok_or_else(|| ContentError::invalid("geometry.vertexCount"))?;
        if len.saturating_add(index_bytes.len()) > crate::spatial_bulk::MAX_SESSION_BYTES {
            return Err(ContentError::invalid("geometry.bytes"));
        }
        let mut vertex_bytes = Vec::with_capacity(len);
        for i in 0..self.positions.len() {
            for (semantic, stream) in &streams {
                for value in &stream[i * semantic.components()..(i + 1) * semantic.components()] {
                    vertex_bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        let mesh = Geometry3D {
            vertex_stride: stride,
            vertex_count: u32::try_from(self.positions.len())
                .map_err(|_| ContentError::invalid("geometry.vertexCount"))?,
            index_count: u32::try_from(count)
                .map_err(|_| ContentError::invalid("geometry.indexCount"))?,
            index_format,
            attributes,
            sections,
            vertex_bytes,
            index_bytes,
        };
        mesh.validate()?;
        Ok(mesh)
    }
}
fn finite_bytes(bytes: &[u8]) -> bool {
    bytes.len().is_multiple_of(4)
        && bytes
            .chunks_exact(4)
            .all(|b| f32::from_le_bytes(b.try_into().expect("four bytes")).is_finite())
}
impl Geometry3D {
    pub fn validate(&self) -> Result<(), ContentError> {
        let invalid = ContentError::invalid;
        if self.vertex_stride == 0
            || !self.vertex_stride.is_multiple_of(4)
            || self.vertex_count == 0
            || u64::from(self.vertex_stride) * u64::from(self.vertex_count)
                != self.vertex_bytes.len() as u64
        {
            return Err(invalid("geometry.vertexBytes"));
        }
        if self.index_count == 0
            || !self.index_count.is_multiple_of(3)
            || u64::from(self.index_count) * self.index_format.byte_len() as u64
                != self.index_bytes.len() as u64
        {
            return Err(invalid("geometry.indexBytes"));
        }
        if self.attributes.len() > 4
            || !self
                .attributes
                .iter()
                .any(|a| a.semantic == Attribute3D::Position)
        {
            return Err(invalid("geometry.attributes"));
        }
        for (i, a) in self.attributes.iter().enumerate() {
            let end = a
                .offset
                .checked_add(a.semantic.byte_len() as u32)
                .ok_or_else(|| invalid("geometry.attributes.offset"))?;
            if !a.offset.is_multiple_of(4)
                || end > self.vertex_stride
                || self.attributes[..i].iter().any(|b| {
                    b.semantic == a.semantic
                        || (a.offset < b.offset + b.semantic.byte_len() as u32 && b.offset < end)
                })
            {
                return Err(invalid("geometry.attributes"));
            }
            for v in self.vertex_bytes.chunks_exact(self.vertex_stride as usize) {
                if !finite_bytes(&v[a.offset as usize..end as usize]) {
                    return Err(invalid("geometry.vertexBytes.nonFinite"));
                }
            }
        }
        for b in self.index_bytes.chunks_exact(self.index_format.byte_len()) {
            let index = match self.index_format {
                IndexFormat3D::Uint16 => u32::from(u16::from_le_bytes(b.try_into().unwrap())),
                IndexFormat3D::Uint32 => u32::from_le_bytes(b.try_into().unwrap()),
            };
            if index >= self.vertex_count {
                return Err(invalid("geometry.indexBytes.range"));
            }
        }
        if self.sections.is_empty() || self.sections.len() > 256 {
            return Err(invalid("geometry.sections"));
        }
        let mut ranges: Vec<_> = self.sections.iter().collect();
        ranges.sort_by_key(|s| s.first_index);
        let mut next = 0u32;
        let mut materials = std::collections::BTreeMap::new();
        for s in ranges {
            if s.first_index != next
                || s.index_count == 0
                || !s.index_count.is_multiple_of(3)
                || s.material_slot.trim().is_empty()
                || s.material_slot.len() > 256
                || s.material.asset_type != "material"
                || s.material.id.trim().is_empty()
                || s.material.id.len() > 256
                || s.material
                    .guid
                    .as_ref()
                    .is_none_or(|g| g.trim().is_empty() || g.len() > 256)
            {
                return Err(invalid("geometry.sections"));
            }
            next = next
                .checked_add(s.index_count)
                .ok_or_else(|| invalid("geometry.sections"))?;
            if materials
                .insert(&s.material_slot, &s.material)
                .is_some_and(|old| old != &s.material)
            {
                return Err(invalid("geometry.sections.materialSlot"));
            }
        }
        if next != self.index_count {
            return Err(invalid("geometry.sections"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributePatch3D {
    pub semantic: Attribute3D,
    pub first_vertex: u32,
    pub vertex_count: u32,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}
impl AttributePatch3D {
    pub fn positions(first_vertex: u32, values: &[[f32; 3]]) -> Result<Self, ContentError> {
        Self::from_values(
            Attribute3D::Position,
            first_vertex,
            values.iter().flatten().copied(),
        )
    }
    pub fn normals(first_vertex: u32, values: &[[f32; 3]]) -> Result<Self, ContentError> {
        Self::from_values(
            Attribute3D::Normal,
            first_vertex,
            values.iter().flatten().copied(),
        )
    }
    pub fn uv(first_vertex: u32, values: &[[f32; 2]]) -> Result<Self, ContentError> {
        Self::from_values(
            Attribute3D::TexCoord0,
            first_vertex,
            values.iter().flatten().copied(),
        )
    }
    pub fn colors(first_vertex: u32, values: &[[f32; 4]]) -> Result<Self, ContentError> {
        Self::from_values(
            Attribute3D::Color0,
            first_vertex,
            values.iter().flatten().copied(),
        )
    }
    fn from_values(
        semantic: Attribute3D,
        first_vertex: u32,
        values: impl Iterator<Item = f32>,
    ) -> Result<Self, ContentError> {
        let mut bytes = Vec::new();
        for value in values {
            if bytes.len() >= crate::spatial_bulk::MAX_SESSION_BYTES || !value.is_finite() {
                return Err(ContentError::invalid("patch.bytes"));
            }
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let vertex_count = u32::try_from(bytes.len() / semantic.byte_len())
            .map_err(|_| ContentError::invalid("patch.vertexCount"))?;
        let patch = Self {
            semantic,
            first_vertex,
            vertex_count,
            bytes,
        };
        patch.validate()?;
        Ok(patch)
    }
    pub fn validate(&self) -> Result<(), ContentError> {
        if self.vertex_count == 0
            || self.first_vertex.checked_add(self.vertex_count).is_none()
            || u64::from(self.vertex_count) * self.semantic.byte_len() as u64
                != self.bytes.len() as u64
            || !finite_bytes(&self.bytes)
        {
            return Err(ContentError::invalid("patch.bytes"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum StaticShape3D {
    Asset {
        source: ProjectRuntimeSpatialAssetRef,
    },
    Box {
        half_extents: [f32; 3],
    },
    Sphere {
        radius: f32,
    },
    Capsule {
        half_height: f32,
        radius: f32,
    },
    Convex {
        vertex_count: u32,
        #[serde(skip)]
        vertex_bytes: Vec<u8>,
    },
    StaticMesh {
        vertex_count: u32,
        triangle_count: u32,
        #[serde(skip)]
        vertex_bytes: Vec<u8>,
        #[serde(skip)]
        triangle_bytes: Vec<u8>,
    },
}
impl StaticShape3D {
    pub fn triangles(vertices: &[[f32; 3]], triangles: &[[u32; 3]]) -> Result<Self, ContentError> {
        if vertices
            .len()
            .checked_add(triangles.len())
            .and_then(|n| n.checked_mul(12))
            .is_none_or(|n| n > crate::spatial_bulk::MAX_SESSION_BYTES)
        {
            return Err(ContentError::invalid("body.shape.bytes"));
        }
        let shape = Self::StaticMesh {
            vertex_count: u32::try_from(vertices.len())
                .map_err(|_| ContentError::invalid("body.shape.vertexCount"))?,
            triangle_count: u32::try_from(triangles.len())
                .map_err(|_| ContentError::invalid("body.shape.triangleCount"))?,
            vertex_bytes: vertices
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            triangle_bytes: triangles
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
        };
        shape.validate()?;
        Ok(shape)
    }
    pub fn validate(&self) -> Result<(), ContentError> {
        let positive = |v: f32| v.is_finite() && v > 0. && v <= 4096.;
        let vertices_ok = |count: u32, bytes: &[u8]| {
            count <= 1_000_000
                && u64::from(count) * 12 == bytes.len() as u64
                && finite_bytes(bytes)
                && bytes
                    .chunks_exact(4)
                    .all(|b| f32::from_le_bytes(b.try_into().unwrap()).abs() <= 4096.)
        };
        let valid = match self {
            Self::Asset { source } => {
                source.asset_type == "collision3d"
                    && !source.id.trim().is_empty()
                    && source.guid.as_ref().is_none_or(|g| !g.trim().is_empty())
            }
            Self::Box { half_extents } => half_extents.iter().all(|v| positive(*v)),
            Self::Sphere { radius } => positive(*radius),
            Self::Capsule {
                half_height,
                radius,
            } => {
                positive(*radius) && half_height.is_finite() && (0.0..=4096.).contains(half_height)
            }
            Self::Convex {
                vertex_count,
                vertex_bytes,
            } => (4..=4096).contains(vertex_count) && vertices_ok(*vertex_count, vertex_bytes),
            Self::StaticMesh {
                vertex_count,
                triangle_count,
                vertex_bytes,
                triangle_bytes,
            } => {
                *vertex_count >= 3
                    && vertices_ok(*vertex_count, vertex_bytes)
                    && (1..=1_000_000).contains(triangle_count)
                    && u64::from(*triangle_count) * 12 == triangle_bytes.len() as u64
                    && triangle_bytes.chunks_exact(12).all(|t| {
                        let ids: [u32; 3] = std::array::from_fn(|i| {
                            u32::from_le_bytes(t[i * 4..i * 4 + 4].try_into().unwrap())
                        });
                        ids.iter().all(|i| i < vertex_count)
                            && ids[0] != ids[1]
                            && ids[1] != ids[2]
                            && ids[0] != ids[2]
                    })
            }
        };
        if !valid {
            return Err(ContentError::invalid("body.shape"));
        }
        Ok(())
    }
    pub(crate) fn payloads_mut(&mut self, f: &mut impl FnMut(&mut Vec<u8>)) {
        match self {
            Self::Convex { vertex_bytes, .. } => f(vertex_bytes),
            Self::StaticMesh {
                vertex_bytes,
                triangle_bytes,
                ..
            } => {
                f(vertex_bytes);
                f(triangle_bytes);
            }
            _ => (),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticBody3D {
    pub enabled: bool,
    pub shape: StaticShape3D,
    pub mass: f32,
    pub friction: f32,
    pub restitution: f32,
    pub ccd: bool,
    pub sensor: bool,
    pub allow_sleeping: bool,
}
impl StaticBody3D {
    pub fn new(shape: StaticShape3D) -> Self {
        Self {
            enabled: true,
            shape,
            mass: 1.,
            friction: 0.5,
            restitution: 0.,
            ccd: false,
            sensor: false,
            allow_sleeping: true,
        }
    }
    pub fn validate(&self) -> Result<(), ContentError> {
        self.shape.validate()?;
        if !self.mass.is_finite()
            || !(0.0..=1e6).contains(&self.mass)
            || self.mass == 0.
            || !self.friction.is_finite()
            || !(0.0..=10.).contains(&self.friction)
            || !self.restitution.is_finite()
            || !(0.0..=1.).contains(&self.restitution)
            || (self.sensor && (self.ccd || matches!(self.shape, StaticShape3D::StaticMesh { .. })))
        {
            return Err(ContentError::invalid("body"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum CollisionUpdate3D {
    Keep,
    Replace { body: StaticBody3D },
    Clear,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum GeneratedOperation3D {
    Replace {
        geometry: Geometry3D,
        collision: CollisionUpdate3D,
    },
    UpdateAttributes {
        patch: AttributePatch3D,
    },
    ReplaceCollision {
        collision: CollisionUpdate3D,
    },
    Release,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedContentRequest {
    pub request_id: u64,
    pub target: ProjectRuntimeCellEntity,
    pub expected_applied_revision: u64,
    pub operation: GeneratedOperation3D,
}
impl GeneratedContentRequest {
    pub fn validate(&self) -> Result<(), ContentError> {
        let result = (|| {
            if self.request_id == 0
                || self.target.generation == 0
                || self.target.entity_id.trim().is_empty()
                || self.target.entity_id.len() > 256
            {
                return Err(ContentError::invalid("request.target"));
            }
            let collision = match &self.operation {
                GeneratedOperation3D::Replace {
                    geometry,
                    collision,
                } => {
                    geometry.validate()?;
                    Some(collision)
                }
                GeneratedOperation3D::UpdateAttributes { patch } => {
                    patch.validate()?;
                    None
                }
                GeneratedOperation3D::ReplaceCollision { collision } => Some(collision),
                GeneratedOperation3D::Release => None,
            };
            if let Some(CollisionUpdate3D::Replace { body }) = collision {
                body.validate()?;
            }
            Ok(())
        })();
        result.map_err(|mut e: ContentError| {
            e.request_id = self.request_id;
            e
        })
    }
    pub(crate) fn payloads_mut(&mut self, mut f: impl FnMut(&mut Vec<u8>)) {
        let collision = match &mut self.operation {
            GeneratedOperation3D::Replace {
                geometry,
                collision,
            } => {
                f(&mut geometry.vertex_bytes);
                f(&mut geometry.index_bytes);
                Some(collision)
            }
            GeneratedOperation3D::UpdateAttributes { patch } => {
                f(&mut patch.bytes);
                None
            }
            GeneratedOperation3D::ReplaceCollision { collision } => Some(collision),
            GeneratedOperation3D::Release => None,
        };
        if let Some(CollisionUpdate3D::Replace { body }) = collision {
            body.shape.payloads_mut(&mut f);
        }
    }
    pub fn payload_bytes(&self) -> usize {
        let collision = |c: &CollisionUpdate3D| match c {
            CollisionUpdate3D::Replace { body } => match &body.shape {
                StaticShape3D::Convex { vertex_bytes, .. } => vertex_bytes.len(),
                StaticShape3D::StaticMesh {
                    vertex_bytes,
                    triangle_bytes,
                    ..
                } => vertex_bytes.len().saturating_add(triangle_bytes.len()),
                _ => 0,
            },
            _ => 0,
        };
        match &self.operation {
            GeneratedOperation3D::Replace {
                geometry,
                collision: c,
            } => geometry
                .vertex_bytes
                .len()
                .saturating_add(geometry.index_bytes.len())
                .saturating_add(collision(c)),
            GeneratedOperation3D::UpdateAttributes { patch } => patch.bytes.len(),
            GeneratedOperation3D::ReplaceCollision { collision: c } => collision(c),
            GeneratedOperation3D::Release => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentStatus {
    Accepted,
    Preparing,
    Applied,
    Failed,
    Cancelled,
    Superseded,
}
impl ContentStatus {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Applied | Self::Failed | Self::Cancelled | Self::Superseded
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentTicket {
    pub session_generation: u64,
    pub request_id: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentReceipt {
    pub ticket: ContentTicket,
    pub target: ProjectRuntimeCellEntity,
    pub status: ContentStatus,
    pub candidate_revision: u64,
    pub applied_revision: u64,
    pub pending_revision: Option<u64>,
    pub collision_revision: u64,
    pub commit_epoch: Option<u64>,
    pub error: Option<ContentError>,
}

#[cfg(test)]
mod tests;
