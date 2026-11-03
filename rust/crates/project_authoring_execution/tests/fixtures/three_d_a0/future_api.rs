//! Compile-time A3 interface fixture only. This is NOT an exported SDK or a runtime writer.
use engine_runtime::spatial_contract::{CanonicalTransform3D, SpatialAssetRef, Transform3D};
use project_game_sdk::EntityHandle;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum SpatialIntentFixture {
    SetLocalTransform {
        transform: CanonicalTransform3D,
    },
    SetMeshVisible {
        visible: bool,
    },
    SetMaterial {
        slot: String,
        material: SpatialAssetRef,
    },
    SelectMainCamera,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialMutationFixture {
    pub kind: String,
    pub entity_id: String,
    pub generation: u64,
    pub intent: SpatialIntentFixture,
}
pub fn write_local_transform3d(
    entity: &EntityHandle,
    transform: &Transform3D,
) -> engine_runtime::spatial_contract::ContractResult<SpatialMutationFixture> {
    Ok(SpatialMutationFixture {
        kind: "spatial".into(),
        entity_id: entity.identity().into(),
        generation: entity.generation(),
        intent: SpatialIntentFixture::SetLocalTransform {
            transform: transform.canonicalize()?,
        },
    })
}
pub fn set_mesh_visible(entity: &EntityHandle, visible: bool) -> SpatialMutationFixture {
    SpatialMutationFixture {
        kind: "spatial".into(),
        entity_id: entity.identity().into(),
        generation: entity.generation(),
        intent: SpatialIntentFixture::SetMeshVisible { visible },
    }
}
pub fn set_material(
    entity: &EntityHandle,
    slot: String,
    material: SpatialAssetRef,
) -> SpatialMutationFixture {
    SpatialMutationFixture {
        kind: "spatial".into(),
        entity_id: entity.identity().into(),
        generation: entity.generation(),
        intent: SpatialIntentFixture::SetMaterial { slot, material },
    }
}
pub fn select_main_camera(entity: &EntityHandle) -> SpatialMutationFixture {
    SpatialMutationFixture {
        kind: "spatial".into(),
        entity_id: entity.identity().into(),
        generation: entity.generation(),
        intent: SpatialIntentFixture::SelectMainCamera,
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialReadFixture {
    pub source_entity_id: String,
    pub entity_id: String,
    pub generation: u64,
    pub local: CanonicalTransform3D,
    /// Column-major affine matrix, not decomposed TRS: parent nonuniform scale may shear.
    pub world_matrix: [f32; 16],
    pub authored_visible: bool,
    pub inherited_visible: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialObservationFixture {
    pub schema_version: String,
    pub revision: String,
    pub delivery_digest: String,
    pub run_ref: String,
    pub operation_id: String,
    pub tick: u64,
    pub frame_sequence: u64,
    pub main_camera: String,
    pub viewport: [u32; 2],
    pub view_version: u64,
    pub suspended: bool,
    pub entity: SpatialReadFixture,
    pub frustum_included: bool,
    pub resource_state: ResourceStateFixture,
    pub frame_stage: FrameStageFixture,
    pub truncated: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceStateFixture {
    Prepared,
    Deferred,
    Failed,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameStageFixture {
    Submitted,
    Presented,
    Captured,
}
