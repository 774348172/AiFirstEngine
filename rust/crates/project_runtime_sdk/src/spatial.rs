//! Spatial command DTOs; authoritative validation and commit stay in the engine World.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRuntimeCharacterMode3D {
    #[default]
    Animated,
    RootMotion,
    Ragdoll,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeTransform3D {
    pub translation: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
}
impl ProjectRuntimeTransform3D {
    pub fn identity() -> Self {
        Self {
            translation: [0.0; 3],
            rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRuntimeSpatialAssetRef {
    pub id: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub guid: Option<String>,
    pub sub_asset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProjectRuntimeSpatialIntent {
    WorldLogicalCapture {
        request_id: u64,
    },
    WorldLogicalRestore {
        request_id: u64,
        snapshot: crate::WorldLogicalSave3D,
    },
    WorldStreamingSources {
        sources: Vec<crate::ProjectRuntimeStreamingSource3D>,
    },
    NavigationTarget {
        target: [f64; 3],
        deadline_ticks: u32,
    },
    NavigationCancel {},
    BodyVelocity {
        velocity: [f32; 3],
    },
    BodyImpulse {
        impulse: [f32; 3],
    },
    MotorMove {
        displacement: [f32; 3],
    },
    MotorJump {
        speed: f32,
    },
    CharacterParameter {
        name: String,
        value: f32,
    },
    CharacterPlay {
        state: String,
        reset: bool,
    },
    CharacterSeek {
        seconds: f64,
    },
    CharacterPaused {
        paused: bool,
    },
    CharacterMode {
        mode: ProjectRuntimeCharacterMode3D,
    },
    SetLocalTransform {
        transform: ProjectRuntimeTransform3D,
    },
    Teleport {
        transform: ProjectRuntimeTransform3D,
    },
    SetInterpolation {
        enabled: bool,
    },
    Reparent {
        parent_entity_id: Option<String>,
        parent_generation: u64,
        preserve_world: bool,
    },
    SetWorldAnchor {
        anchor: [f64; 3],
        preserve_world: bool,
    },
    SetMeshVisible {
        visible: bool,
    },
    SetMaterial {
        slot: String,
        material: ProjectRuntimeSpatialAssetRef,
    },
    /// Toggle an existing authored Light3D, without replacing its kind or layers.
    SetLightEnabled {
        enabled: bool,
    },
    /// Linear RGB. The engine validates finite nonnegative values atomically.
    SetLightColor {
        color: [f32; 3],
    },
    /// Lux for directional lights, candela for point/spot lights; kind is unchanged.
    SetLightIntensity {
        intensity: f32,
    },
    SelectMainCamera {},
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn spatial_foundation_wire_has_canonical_parent_and_anchor_fields() {
        let intent = ProjectRuntimeSpatialIntent::Reparent {
            parent_entity_id: Some("parent".into()),
            parent_generation: 4294967297,
            preserve_world: true,
        };
        let value = serde_json::to_value(&intent).unwrap();
        assert_eq!(
            value,
            json!({"operation":"reparent","parentEntityId":"parent","parentGeneration":4294967297u64,"preserveWorld":true})
        );
        assert_eq!(
            serde_json::from_value::<ProjectRuntimeSpatialIntent>(value).unwrap(),
            intent
        );
        assert!(serde_json::from_value::<ProjectRuntimeSpatialIntent>(json!({"operation":"reparent","parent_entity_id":null,"parentGeneration":0,"preserveWorld":true})).is_err());
    }
    #[test]
    fn s2b_light_wire_is_strict_and_roundtrips_explicit_units() {
        for (intent, value) in [
            (
                ProjectRuntimeSpatialIntent::SetLightEnabled { enabled: false },
                json!({"operation":"setLightEnabled","enabled":false}),
            ),
            (
                ProjectRuntimeSpatialIntent::SetLightColor {
                    color: [0., 1., 0.],
                },
                json!({"operation":"setLightColor","color":[0.,1.,0.]}),
            ),
            (
                ProjectRuntimeSpatialIntent::SetLightIntensity { intensity: 12. },
                json!({"operation":"setLightIntensity","intensity":12.}),
            ),
        ] {
            assert_eq!(serde_json::to_value(&intent).unwrap(), value);
            assert_eq!(
                serde_json::from_value::<ProjectRuntimeSpatialIntent>(value.clone()).unwrap(),
                intent
            );
            let mut bad = value;
            bad["unrecognized"] = true.into();
            assert!(serde_json::from_value::<ProjectRuntimeSpatialIntent>(bad).is_err());
        }
        assert!(serde_json::from_value::<ProjectRuntimeSpatialIntent>(
            json!({"operation":"setLightIntensity","illuminanceLux":12.})
        )
        .is_err());
    }
    #[test]
    fn spatial_wire_rejects_unknown_intent_fields_and_legacy_euler_payloads() {
        for value in [
            json!({"operation":"selectMainCamera","extra":true}),
            json!({"operation":"setLocalTransform","transform":{"position":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]}}),
            json!({"operation":"setMaterial","slot":"surface","material":{"id":"x","type":"material","guid":"g","path":"x"}}),
        ] {
            assert!(serde_json::from_value::<ProjectRuntimeSpatialIntent>(value).is_err());
        }
        let command = crate::ProjectRuntimeDeferredMutation::Spatial {
            entity_id: "cube".into(),
            generation: 4294967297,
            intent: ProjectRuntimeSpatialIntent::SetLocalTransform {
                transform: ProjectRuntimeTransform3D::identity(),
            },
        };
        let value = serde_json::to_value(&command).unwrap();
        assert_eq!(value["kind"], "spatial");
        assert_eq!(value["entityId"], "cube");
        assert_eq!(
            serde_json::from_value::<crate::ProjectRuntimeDeferredMutation>(value).unwrap(),
            command
        );
    }
}
