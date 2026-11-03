//! Synchronous, bounded reads of the existing physics owner's committed state.
use serde::{Deserialize, Serialize};

pub const MAX_PHYSICS_QUERIES_3D: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PhysicsQueryShape3D {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}
impl PhysicsQueryShape3D {
    pub fn is_valid(&self) -> bool {
        let positive = |v: f32| v.is_finite() && v > 0. && v <= 4096.;
        match self {
            Self::Box { half_extents } => half_extents.iter().all(|v| positive(*v)),
            Self::Sphere { radius } => positive(*radius),
            Self::Capsule {
                half_height,
                radius,
            } => {
                positive(*radius)
                    && half_height.is_finite()
                    && *half_height >= 0.
                    && *half_height <= 4096.
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PhysicsQuery3D {
    Raycast {
        origin: [f64; 3],
        displacement: [f32; 3],
    },
    Sweep {
        origin: [f64; 3],
        displacement: [f32; 3],
        shape: PhysicsQueryShape3D,
        rotation_xyzw: [f32; 4],
    },
}
impl PhysicsQuery3D {
    pub fn is_valid(&self) -> bool {
        let (origin, displacement) = match self {
            Self::Raycast {
                origin,
                displacement,
            }
            | Self::Sweep {
                origin,
                displacement,
                ..
            } => (origin, displacement),
        };
        let length_squared = displacement.iter().map(|v| v * v).sum::<f32>();
        origin.iter().all(|v| v.is_finite())
            && displacement.iter().all(|v| v.is_finite())
            && length_squared.is_finite()
            && length_squared > 1e-12
            && match self {
                Self::Raycast { .. } => true,
                Self::Sweep {
                    shape,
                    rotation_xyzw,
                    ..
                } => {
                    let norm = rotation_xyzw.iter().map(|v| v * v).sum::<f32>();
                    shape.is_valid()
                        && rotation_xyzw.iter().all(|v| v.is_finite())
                        && norm.is_finite()
                        && (norm - 1.).abs() <= 1e-4
                }
            }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicsQueryBatch3D {
    pub queries: Vec<PhysicsQuery3D>,
}
impl PhysicsQueryBatch3D {
    pub fn is_valid(&self) -> bool {
        !self.queries.is_empty()
            && self.queries.len() <= MAX_PHYSICS_QUERIES_3D
            && self.queries.iter().all(PhysicsQuery3D::is_valid)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicsQueryHit3D {
    pub entity: String,
    pub generation: u64,
    pub fraction: f32,
    pub distance: f32,
    pub position: [f64; 3],
    pub normal: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicsQueryResult3D {
    /// None means the real owner has not committed a solver step yet.
    pub tick: Option<u64>,
    /// Input order. Empty only when tick is None; a ready miss is an explicit None.
    pub hits: Vec<Option<PhysicsQueryHit3D>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_batch_reply_does_not_repeat_host_work_with_reserved_storage() {
        use project_runtime_abi::*;
        use std::cell::Cell;
        thread_local! { static CALLS: Cell<u32> = const { Cell::new(0) }; }
        unsafe extern "C" fn reply(
            _: ProjectRuntimeOpaqueHandle,
            _: ProjectRuntimeByteSlice,
            output: *mut ProjectRuntimeByteBuffer,
        ) -> ProjectRuntimeAbiStatus {
            crate::ffi_boundary(output, || {
                CALLS.with(|calls| calls.set(calls.get() + 1));
                Ok(PhysicsQueryResult3D {
                    tick: Some(9),
                    hits: vec![
                        Some(PhysicsQueryHit3D {
                            entity: "dynamic-0999".into(),
                            generation: 42,
                            fraction: 0.5,
                            distance: 4.,
                            position: [0., 0., 0.],
                            normal: [0., 1., 0.],
                        });
                        250
                    ],
                })
            })
        }
        CALLS.with(|calls| calls.set(0));
        let result: PhysicsQueryResult3D = crate::call_host_json_with_capacity(
            reply,
            ProjectRuntimeOpaqueHandle {
                value: 1,
                generation: 1,
            },
            &(),
            250 * 1024,
        )
        .unwrap();
        assert_eq!(result.hits.len(), 250);
        assert_eq!(
            CALLS.with(Cell::get),
            1,
            "sizing retry repeated the entire query batch"
        );
        CALLS.with(|calls| calls.set(0));
        let retry: PhysicsQueryResult3D = crate::call_host_json_with_capacity(
            reply,
            ProjectRuntimeOpaqueHandle {
                value: 1,
                generation: 1,
            },
            &(),
            1,
        )
        .unwrap();
        assert_eq!(retry, result);
        assert_eq!(
            CALLS.with(Cell::get),
            2,
            "capacity hints must retain the existing retry behavior"
        );
    }
    #[test]
    fn batch_limits_and_all_query_inputs_are_validated_before_execution() {
        let ray = PhysicsQuery3D::Raycast {
            origin: [1e6, 4., 0.],
            displacement: [0., -8., 0.],
        };
        assert!(PhysicsQueryBatch3D {
            queries: vec![ray.clone(); 256]
        }
        .is_valid());
        assert!(!PhysicsQueryBatch3D {
            queries: vec![ray.clone(); 257]
        }
        .is_valid());
        assert!(!PhysicsQueryBatch3D { queries: vec![] }.is_valid());
        for displacement in [[0.; 3], [f32::NAN, 0., 0.], [f32::MAX, 0., 0.]] {
            assert!(!PhysicsQuery3D::Raycast {
                origin: [0.; 3],
                displacement
            }
            .is_valid());
        }
        assert!(!PhysicsQuery3D::Raycast {
            origin: [f64::INFINITY, 0., 0.],
            displacement: [0., 1., 0.]
        }
        .is_valid());
        for rotation_xyzw in [[0.; 4], [0., 0., 0., 2.], [f32::NAN, 0., 0., 1.]] {
            assert!(!PhysicsQuery3D::Sweep {
                origin: [0.; 3],
                displacement: [0., 1., 0.],
                shape: PhysicsQueryShape3D::Sphere { radius: 0.5 },
                rotation_xyzw
            }
            .is_valid());
        }
        for shape in [
            PhysicsQueryShape3D::Sphere { radius: -1. },
            PhysicsQueryShape3D::Box {
                half_extents: [1., 0., 1.],
            },
            PhysicsQueryShape3D::Capsule {
                half_height: -1.,
                radius: 0.5,
            },
        ] {
            assert!(!shape.is_valid());
        }
        let wire = serde_json::to_value(&ray).unwrap();
        assert_eq!(serde_json::from_value::<PhysicsQuery3D>(wire).unwrap(), ray);
        assert!(serde_json::from_value::<PhysicsQuery3D>(serde_json::json!({"kind":"raycast","origin":[0,1,0],"displacement":[0,-1,0],"solverHandle":1})).is_err());
    }
    #[test]
    fn physics_selector_is_exclusive_and_old_query_wire_is_unchanged() {
        let valid = serde_json::json!({"physics":{"queries":[{"kind":"raycast","origin":[0,4,0],"displacement":[0,-8,0]}]},"all":[],"none":[]});
        let request: crate::ProjectRuntimeWorldQueryRequest =
            serde_json::from_value(valid.clone()).unwrap();
        assert!(request.validate_identity_query().is_ok());
        for (key, value) in [
            ("entityId", serde_json::json!("actor")),
            ("cell", serde_json::json!({"requestId":1})),
            ("spawnRequestId", serde_json::json!(1)),
            ("all", serde_json::json!(["engine.body3d"])),
            ("none", serde_json::json!(["engine.body3d"])),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            let request: crate::ProjectRuntimeWorldQueryRequest =
                serde_json::from_value(invalid).unwrap();
            assert!(request.validate_identity_query().is_err());
        }
        let old = serde_json::json!({"all":[],"none":[]});
        let request: crate::ProjectRuntimeWorldQueryRequest =
            serde_json::from_value(old.clone()).unwrap();
        assert_eq!(serde_json::to_value(request).unwrap(), old);
    }
}
