use crate::ids::EntityId;
use crate::math::Vec3;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
    Vec2 { x: f32, y: f32 },
    Vec3(Vec3),
    Color { r: f32, g: f32, b: f32, a: f32 },
    EntityRef(EntityId),
    AssetRef(String),
    Object(BTreeMap<String, RuntimeValue>),
    Array(Vec<RuntimeValue>),
}

impl RuntimeValue {
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub fn asset_ref(value: impl Into<String>) -> Self {
        Self::AssetRef(value.into())
    }

    pub fn object(fields: impl IntoIterator<Item = (impl Into<String>, RuntimeValue)>) -> Self {
        Self::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }
}

impl From<&str> for RuntimeValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for RuntimeValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<bool> for RuntimeValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for RuntimeValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<f64> for RuntimeValue {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_value_object_orders_fields_deterministically() {
        let value =
            RuntimeValue::object([("z", RuntimeValue::I64(1)), ("a", RuntimeValue::Bool(true))]);
        let RuntimeValue::Object(fields) = value else {
            panic!("expected object value");
        };
        let keys = fields.keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys, vec!["a".to_string(), "z".to_string()]);
    }

    #[test]
    fn runtime_value_supports_vec3_and_asset_ref() {
        let position = RuntimeValue::Vec3(Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });
        let asset = RuntimeValue::asset_ref("asset://sprite/player");

        assert_eq!(
            position,
            RuntimeValue::Vec3(Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            })
        );
        assert_eq!(
            asset,
            RuntimeValue::AssetRef("asset://sprite/player".to_string())
        );
    }
}
