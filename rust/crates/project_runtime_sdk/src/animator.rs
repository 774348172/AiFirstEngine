//! Pure-data Animator session wire. Uses the existing caller-owned JSON ABI.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
fn exact_u64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Exact {
        Number(u64),
        Text(String),
    }
    match Exact::deserialize(d)? {
        Exact::Number(n) => Ok(n),
        Exact::Text(s) => s.parse().map_err(serde::de::Error::custom),
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeAnimatorIntent {
    Play { name: String },
    Restart { name: String },
    Resume {},
    SetPaused { paused: bool },
    SetBool { name: String, value: bool },
    SetTrigger { name: String },
    FadeOutLayer { layer: String, duration_ticks: u32 },
    ClearLayer { layer: String },
    SetSkin { name: String },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorSocket {
    #[serde(deserialize_with = "exact_u64")]
    pub entity_generation: u64,
    #[serde(deserialize_with = "exact_u64")]
    pub tick: u64,
    #[serde(deserialize_with = "exact_u64")]
    pub pose_revision: u64,
    pub position: [f32; 2],
    pub rotation_degrees: f32,
    pub source_asset_id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorEventPayload {
    pub name: String,
    pub int_value: i32,
    pub float_value: f32,
    pub string_value: String,
    pub volume: f32,
    pub balance: f32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimatorEvent {
    pub entity_id: String,
    #[serde(deserialize_with = "exact_u64")]
    pub generation: u64,
    #[serde(deserialize_with = "exact_u64")]
    pub play_id: u64,
    pub track: u8,
    #[serde(deserialize_with = "exact_u64")]
    pub loop_index: u64,
    pub source_event_id: String,
    pub source_ordinal: u32,
    #[serde(deserialize_with = "exact_u64")]
    pub tick: u64,
    #[serde(deserialize_with = "exact_u64")]
    pub crossing_us: u64,
    pub payload: AnimatorEventPayload,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorSnapshot {
    #[serde(deserialize_with = "exact_u64")]
    pub entity_generation: u64,
    #[serde(deserialize_with = "exact_u64")]
    pub tick: u64,
    #[serde(deserialize_with = "exact_u64")]
    pub pose_revision: u64,
    pub sockets: BTreeMap<String, AnimatorSocket>,
    pub events: Vec<AnimatorEvent>,
}
impl AnimatorSnapshot {
    pub fn to_value(&self) -> crate::ProjectRuntimeValue {
        fn convert(v: serde_json::Value) -> crate::ProjectRuntimeValue {
            use crate::ProjectRuntimeValue as V;
            match v {
                serde_json::Value::Null => V::Null,
                serde_json::Value::Bool(b) => V::Bool(b),
                serde_json::Value::String(s) => V::String(s),
                serde_json::Value::Number(n) => {
                    if let Some(n) = n.as_i64() {
                        V::Integer(n)
                    } else {
                        V::String(n.to_string())
                    }
                }
                serde_json::Value::Array(a) => V::Array(a.into_iter().map(convert).collect()),
                serde_json::Value::Object(o) => {
                    V::Object(o.into_iter().map(|(k, v)| (k, convert(v))).collect())
                }
            }
        }
        // Preserve float values as Number and u64 values exactly, including >2^53.
        fn numeric(v: serde_json::Value) -> crate::ProjectRuntimeValue {
            use crate::ProjectRuntimeValue as V;
            match v {
                serde_json::Value::Number(n) if n.is_f64() => V::Number(n.as_f64().unwrap()),
                serde_json::Value::Array(a) => V::Array(a.into_iter().map(numeric).collect()),
                serde_json::Value::Object(o) => {
                    V::Object(o.into_iter().map(|(k, v)| (k, numeric(v))).collect())
                }
                v => convert(v),
            }
        }
        numeric(serde_json::to_value(self).expect("finite committed animator snapshot"))
    }
    pub fn from_value(v: crate::ProjectRuntimeValue) -> Result<Self, String> {
        fn convert(v: crate::ProjectRuntimeValue) -> Result<serde_json::Value, String> {
            use crate::ProjectRuntimeValue as V;
            Ok(match v {
                V::Null => serde_json::Value::Null,
                V::Bool(b) => b.into(),
                V::Integer(n) => n.into(),
                V::Number(n) => serde_json::Number::from_f64(n)
                    .ok_or("nonfinite animator number")?
                    .into(),
                V::String(s) => s.into(),
                V::Array(a) => a
                    .into_iter()
                    .map(convert)
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
                V::Object(o) => serde_json::Value::Object(
                    o.into_iter()
                        .map(|(k, v)| Ok((k, convert(v)?)))
                        .collect::<Result<_, String>>()?,
                ),
                _ => return Err("invalid animator snapshot value".into()),
            })
        }
        serde_json::from_value(convert(v)?).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animator374_wire_is_strict_and_preserves_old_mutations() {
        let intent = ProjectRuntimeAnimatorIntent::FadeOutLayer {
            layer: "upper".into(),
            duration_ticks: 12,
        };
        let value = serde_json::to_value(&intent).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"operation":"fade_out_layer","layer":"upper","duration_ticks":12})
        );
        for bad in [
            serde_json::json!({"operation":"seek","time":2}),
            serde_json::json!({"operation":"resume","name":"x"}),
            serde_json::json!({"operation":"set_bool","name":"x","value":1}),
        ] {
            assert!(serde_json::from_value::<ProjectRuntimeAnimatorIntent>(bad).is_err());
        }
        let old = serde_json::json!({"kind":"animator2_d","entity_id":"e","intent":{"operation":"resume"}});
        // Use the canonical old serializer so this check follows its established spelling.
        let old = serde_json::to_value(crate::ProjectRuntimeDeferredMutation::Animator2D {
            entity_id: "e".into(),
            intent: crate::ProjectRuntimeAnimator2DIntent::Resume,
        })
        .unwrap_or(old);
        assert!(serde_json::from_value::<crate::ProjectRuntimeDeferredMutation>(old).is_ok());
        let command = crate::ProjectRuntimeDeferredMutation::Animator {
            entity_id: "e".into(),
            generation: 9,
            intent,
        };
        assert_eq!(serde_json::to_value(&command).unwrap()["kind"], "animator");
        assert!(crate::PROJECT_RUNTIME_SDK_SCHEMA.contains("animator-session-intents.v1"));
    }
    #[test]
    fn animator374_snapshot_roundtrip_preserves_large_integers_and_floats() {
        let s = AnimatorSnapshot {
            entity_generation: u64::MAX,
            tick: 1_u64 << 54,
            pose_revision: u64::MAX - 1,
            sockets: BTreeMap::from([(
                "hand".into(),
                AnimatorSocket {
                    entity_generation: u64::MAX,
                    tick: 6,
                    pose_revision: 9,
                    position: [0.25, -4.0],
                    rotation_degrees: 13.5,
                    source_asset_id: "s".into(),
                },
            )]),
            events: vec![AnimatorEvent {
                entity_id: "e".into(),
                generation: u64::MAX,
                play_id: u64::MAX,
                track: 1,
                loop_index: u64::MAX,
                source_event_id: "event".into(),
                source_ordinal: 0,
                tick: u64::MAX,
                crossing_us: u64::MAX,
                payload: AnimatorEventPayload {
                    name: "step".into(),
                    int_value: -1,
                    float_value: 0.25,
                    string_value: "12".into(),
                    volume: 0.5,
                    balance: -0.5,
                },
            }],
        };
        assert_eq!(AnimatorSnapshot::from_value(s.to_value()).unwrap(), s);
    }
}
