//! SDK bytes are consumed once into the existing neutral Body3D contract.
use super::*;
use crate::physics_3d::{Body3D, BodyMotion3D, CollisionShape3D};
use crate::spatial_contract::SpatialAssetRef;

pub(super) fn native_body(body: StaticBody3D) -> Body3D {
    let floats = |bytes: Vec<u8>| {
        bytes
            .chunks_exact(12)
            .map(|v| {
                std::array::from_fn(|i| f32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap()))
            })
            .collect()
    };
    let shape = match body.shape {
        StaticShape3D::Asset { source } => CollisionShape3D::Asset {
            source: SpatialAssetRef(RuntimeAssetRef {
                id: source.id,
                asset_type: source.asset_type,
                guid: source.guid,
                sub_asset: source.sub_asset,
            }),
        },
        StaticShape3D::Box { half_extents } => CollisionShape3D::Box { half_extents },
        StaticShape3D::Sphere { radius } => CollisionShape3D::Sphere { radius },
        StaticShape3D::Capsule {
            half_height,
            radius,
        } => CollisionShape3D::Capsule {
            half_height,
            radius,
        },
        StaticShape3D::Convex { vertex_bytes, .. } => CollisionShape3D::Convex {
            vertices: floats(vertex_bytes),
        },
        StaticShape3D::StaticMesh {
            vertex_bytes,
            triangle_bytes,
            ..
        } => CollisionShape3D::StaticMesh {
            vertices: floats(vertex_bytes),
            triangles: triangle_bytes
                .chunks_exact(12)
                .map(|v| {
                    std::array::from_fn(|i| {
                        u32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect(),
        },
    };
    Body3D {
        enabled: body.enabled,
        schema_version: "body3d.v1".into(),
        motion: BodyMotion3D::Static,
        shape,
        mass: body.mass,
        friction: body.friction,
        restitution: body.restitution,
        ccd: body.ccd,
        sensor: body.sensor,
        allow_sleeping: body.allow_sleeping,
    }
}
pub(super) fn failure(
    id: u64,
    failure: crate::spatial_contract::SpatialContractError,
) -> ContentError {
    let code = match failure.field_path.as_str() {
        "/generated/motion" => ContentErrorCode::UnsupportedMotionType,
        "/generated/capacity" => ContentErrorCode::BudgetExceeded,
        "/generated/identity" | "/generated/pose" => ContentErrorCode::RevisionConflict,
        _ => ContentErrorCode::InvalidArgument,
    };
    error(code, id, &format!("collision{}", failure.field_path))
}
