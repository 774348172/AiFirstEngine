//! Large v2 prefabs: one bounded entity DOM, then the original typed tree validator.
use super::geometry_decode::lexical_bounds;
use crate::{
    runtime_asset_loader::StreamingError,
    runtime_package::{RuntimeAssetRef, RuntimeEntity, RuntimePrefabData},
    spatial_contract::{RuntimeEntityDocument, RuntimePrefabV2, RuntimeSpatial, SpatialAssetRef},
};
use serde::{de::SeqAccess, de::Visitor, Deserialize, Deserializer};
use serde_json::{value::RawValue, Value};
use std::mem::{align_of, size_of};

fn invalid(e: impl std::fmt::Display) -> StreamingError {
    StreamingError::DecodeFailed(e.to_string())
}
fn add(a: usize, b: usize) -> Result<usize, StreamingError> {
    a.checked_add(b).ok_or(StreamingError::BudgetExceeded)
}
fn mul(a: usize, b: usize) -> Result<usize, StreamingError> {
    a.checked_mul(b).ok_or(StreamingError::BudgetExceeded)
}
fn sum(values: impl IntoIterator<Item = usize>) -> Result<usize, StreamingError> {
    values.into_iter().try_fold(0, add)
}

// Pinned Rust1.96 BTree nodes have eleven key/value slots, twelve edges,
// parent/index/length. Every non-root node has at least five keys. Charge every
// possible node as an internal node, including padding; never one byte per key.
fn tree_bytes<K, V>(len: usize) -> Result<usize, StreamingError> {
    if len == 0 {
        return Ok(0);
    }
    let node = add(mul(size_of::<(K, V)>(), 11)?, 16 * size_of::<usize>())?;
    mul(1 + (len - 1) / 5, node)
}
fn json_heap(value: &Value) -> Result<usize, StreamingError> {
    match value {
        Value::String(s) => Ok(s.capacity()),
        Value::Array(values) => values.iter().try_fold(
            mul(values.capacity(), size_of::<Value>())?,
            |bytes, value| add(bytes, json_heap(value)?),
        ),
        Value::Object(values) => values.iter().try_fold(
            tree_bytes::<String, Value>(values.len())?,
            |bytes, (key, value)| add(add(bytes, key.capacity())?, json_heap(value)?),
        ),
        _ => Ok(0),
    }
}
fn reference_heap(reference: &RuntimeAssetRef) -> Result<usize, StreamingError> {
    sum([
        reference.id.capacity(),
        reference.asset_type.capacity(),
        reference.guid.as_ref().map_or(0, String::capacity),
        reference.sub_asset.as_ref().map_or(0, String::capacity),
    ])
}
fn entity_heap(entity: &RuntimeEntityDocument, raw_len: usize) -> Result<usize, StreamingError> {
    let RuntimeEntityDocument::Spatial(e) = entity else {
        // The unchanged legacy owner retains its original conservative envelope.
        return mul(raw_len, 64);
    };
    let RuntimeSpatial::World3D {
        visibility,
        mesh_renderer,
        camera,
        light,
        ..
    } = &e.spatial;
    if camera.is_some() || light.is_some() {
        return mul(raw_len, 64);
    }
    let mut bytes = sum([
        e.schema_version.capacity(),
        e.id.capacity(),
        e.name.capacity(),
        e.parent_id.as_ref().map_or(0, String::capacity),
        // From<RuntimeEntityV2> adds this short compatibility field.
        6,
        mul(
            e.components.capacity(),
            size_of::<crate::spatial_contract::ProjectDataComponent>(),
        )?,
    ])?;
    for component in &e.components {
        bytes = add(
            bytes,
            add(
                component.component_type.capacity(),
                json_heap(&component.data)?,
            )?,
        )?;
    }
    if let Some(v) = visibility {
        bytes = add(bytes, v.schema_version.capacity())?;
    }
    if let Some(mesh) = mesh_renderer {
        bytes = add(
            bytes,
            sum([
                mesh.schema_version.capacity(),
                reference_heap(&mesh.mesh.0)?,
                mul(mesh.layers.capacity(), size_of::<String>())?,
                tree_bytes::<String, SpatialAssetRef>(mesh.materials.len())?,
            ])?,
        )?;
        for layer in &mesh.layers {
            bytes = add(bytes, layer.capacity())?;
        }
        for (slot, reference) in &mesh.materials {
            bytes = add(bytes, add(slot.capacity(), reference_heap(&reference.0)?)?)?;
        }
    }
    Ok(bytes)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPrefab<'a> {
    #[serde(borrow)]
    schema_version: &'a RawValue,
    #[serde(borrow)]
    id: &'a RawValue,
    #[serde(borrow)]
    name: &'a RawValue,
    #[serde(borrow)]
    root_entity_id: &'a RawValue,
    #[serde(borrow)]
    entities: &'a RawValue,
}
struct Count;
impl<'de> Visitor<'de> for Count {
    type Value = usize;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("prefab entities")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<usize, A::Error> {
        let mut count = 0usize;
        while seq.next_element::<&'de RawValue>()?.is_some() {
            count = count
                .checked_add(1)
                .ok_or_else(|| serde::de::Error::custom("entity count overflow"))?;
        }
        Ok(count)
    }
}
struct Build {
    count: usize,
    base: usize,
    ceiling: usize,
}
impl<'de> Visitor<'de> for Build {
    type Value = (Vec<RuntimeEntityDocument>, usize);
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("bounded prefab entities")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error;
        let mut entities = Vec::with_capacity(self.count);
        let mut heap = 0usize;
        let mut peak = self.base;
        let mut largest_scratch = 0usize;
        while let Some(raw) = seq.next_element::<&'de RawValue>()? {
            let result = (|| {
                let scratch = add(mul(raw.get().len(), 64)?, 64 * 1024)?;
                largest_scratch = largest_scratch.max(scratch);
                let bound = add(add(self.base, heap)?, scratch)?;
                if bound > self.ceiling {
                    return Err(StreamingError::BudgetExceeded);
                }
                let value =
                    crate::spatial_contract::strict_json(raw.get().as_bytes()).map_err(invalid)?;
                let entity: RuntimeEntityDocument =
                    serde_json::from_value(value).map_err(invalid)?;
                let retained = entity_heap(&entity, raw.get().len())?;
                let after = add(add(add(self.base, heap)?, retained)?, largest_scratch)?;
                if after > self.ceiling {
                    return Err(StreamingError::BudgetExceeded);
                }
                heap = add(heap, retained)?;
                peak = peak.max(bound).max(after);
                Ok(entity)
            })();
            entities.push(result.map_err(|e| A::Error::custom(format!("{e:?}")))?);
        }
        Ok((entities, peak))
    }
}

pub(super) fn decode_prefab(
    bytes: &[u8],
    ceiling: usize,
) -> Result<(RuntimePrefabData, usize), StreamingError> {
    lexical_bounds(bytes, ceiling)?;
    let raw: RawPrefab<'_> = serde_json::from_slice(bytes).map_err(invalid)?;
    let schema: String = serde_json::from_str(raw.schema_version.get()).map_err(invalid)?;
    if schema != crate::spatial_contract::RUNTIME_PREFAB_V2 {
        return Err(invalid("Large prefab requires the existing v2 contract"));
    }
    let mut count_decoder = serde_json::Deserializer::from_str(raw.entities.get());
    let count = count_decoder.deserialize_seq(Count).map_err(invalid)?;
    count_decoder.end().map_err(invalid)?;
    let in_place = size_of::<RuntimeEntityDocument>() == size_of::<RuntimeEntity>()
        && align_of::<RuntimeEntityDocument>() == align_of::<RuntimeEntity>();
    let entity_size = if in_place {
        size_of::<RuntimeEntityDocument>()
    } else {
        add(
            size_of::<RuntimeEntityDocument>(),
            size_of::<RuntimeEntity>(),
        )?
    };
    // Exact-capacity entity Vec, complete-tree by-id/done/chain/root work, and
    // header strings. The equal-layout map/collect in wire.rs reuses its input
    // allocation on pinned Rust1.96; other layouts reserve both buffers.
    let base = sum([
        mul(count, entity_size)?,
        mul(count, 256)?,
        64 * 1024,
        mul(
            bytes
                .len()
                .checked_sub(raw.entities.get().len())
                .ok_or(StreamingError::BudgetExceeded)?,
            64,
        )?,
    ])?;
    if base > ceiling {
        return Err(StreamingError::BudgetExceeded);
    }
    let mut decoder = serde_json::Deserializer::from_str(raw.entities.get());
    let (entities, peak) = decoder
        .deserialize_seq(Build {
            count,
            base,
            ceiling,
        })
        .map_err(|e| {
            if e.to_string().starts_with("BudgetExceeded") {
                StreamingError::BudgetExceeded
            } else {
                invalid(e)
            }
        })?;
    decoder.end().map_err(invalid)?;
    let mut prefab = RuntimePrefabV2 {
        schema_version: schema,
        id: serde_json::from_str(raw.id.get()).map_err(invalid)?,
        name: serde_json::from_str(raw.name.get()).map_err(invalid)?,
        root_entity_id: serde_json::from_str(raw.root_entity_id.get()).map_err(invalid)?,
        entities,
    };
    prefab.validate_and_canonicalize().map_err(invalid)?;
    let allocation = prefab.entities.as_ptr() as usize;
    let prefab: RuntimePrefabData = prefab.into();
    if in_place && !prefab.entities.is_empty() && prefab.entities.as_ptr() as usize != allocation {
        return Err(invalid(
            "Pinned equal-layout prefab conversion did not reuse its allocation",
        ));
    }
    Ok((prefab, peak))
}
