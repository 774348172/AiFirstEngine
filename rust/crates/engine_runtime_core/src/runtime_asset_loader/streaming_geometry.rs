//! Allocation bounds for the existing numeric geometry schemas.
//! RawValue avoids a temporary tagged-enum Content tree before the typed arrays.
use crate::{
    character_animation::{CharacterAsset, VertexInfluence},
    physics_3d::{CollisionShape3D, CookedCollision3D},
    render_contract::{
        CookedGeometry, CookedRenderAsset, RenderAssetData, RenderContractVersion,
        ValidateRenderContract,
    },
    runtime_asset_loader::StreamingError,
};
use serde::{de::IgnoredAny, de::SeqAccess, de::Visitor, Deserialize, Deserializer};
use serde_json::value::RawValue;
use std::marker::PhantomData;

fn invalid(error: impl std::fmt::Display) -> StreamingError {
    StreamingError::DecodeFailed(error.to_string())
}

// Check before serde allocates scratch for escaped strings or enters nested JSON.
pub(super) fn lexical_bounds(bytes: &[u8], ceiling: usize) -> Result<(), StreamingError> {
    let mut cursor = 0;
    let mut depth = 0usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => {
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && bytes[cursor] != b'"' {
                    if bytes[cursor] == b'\\' {
                        cursor += 1;
                    }
                    cursor += 1;
                    if cursor.saturating_sub(start) > ceiling / 64 {
                        return Err(StreamingError::BudgetExceeded);
                    }
                }
                if cursor >= bytes.len() {
                    return Err(invalid("Unterminated geometry string"));
                }
            }
            b'[' | b'{' => {
                depth += 1;
                if depth > 128 {
                    return Err(StreamingError::BudgetExceeded);
                }
            }
            b']' | b'}' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| invalid("JSON nesting"))?;
            }
            _ => (),
        }
        cursor += 1;
    }
    if depth != 0 {
        return Err(invalid("Unclosed geometry JSON"));
    }
    Ok(())
}

fn array_len<'de, T: Deserialize<'de>>(raw: &'de RawValue) -> Result<usize, StreamingError> {
    struct Count<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Count<T> {
        type Value = usize;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a geometry array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<usize, A::Error> {
            let mut count = 0usize;
            while sequence.next_element::<T>()?.is_some() {
                count = count.checked_add(1).ok_or_else(|| {
                    <A::Error as serde::de::Error>::custom("Geometry element count overflow")
                })?;
            }
            Ok(count)
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(raw.get());
    let count = decoder
        .deserialize_seq(Count::<T>(PhantomData))
        .map_err(invalid)?;
    decoder.end().map_err(invalid)?;
    Ok(count)
}

fn vector_bound(count: usize, element_bytes: usize) -> Result<usize, StreamingError> {
    if count == 0 {
        return Ok(0);
    }
    // serde_json has no array length hint. Include the old buffer during a
    // growth allocation as well as the final power-of-two capacity.
    count
        .max(if element_bytes == 1 { 8 } else { 4 })
        .checked_next_power_of_two()
        .and_then(|capacity| capacity.checked_mul(element_bytes))
        .and_then(|bytes| bytes.checked_add(bytes / 2))
        .ok_or(StreamingError::BudgetExceeded)
}

pub(super) fn mesh_metadata_copy_bound(asset: &CookedRenderAsset) -> Option<usize> {
    let RenderAssetData::Mesh(mesh) = &asset.asset else {
        return None;
    };
    let mut bytes = std::mem::size_of::<CookedRenderAsset>()
        .checked_add(asset.source_digest.capacity())?
        .checked_add(asset.recipe.capacity())?
        .checked_add(
            mesh.attributes
                .capacity()
                .checked_mul(std::mem::size_of::<crate::render_contract::VertexAttribute>())?,
        )?
        .checked_add(
            mesh.submeshes
                .capacity()
                .checked_mul(std::mem::size_of::<crate::render_contract::SubmeshRange>())?,
        )?
        .checked_add(
            mesh.material_slots
                .capacity()
                .checked_mul(std::mem::size_of::<String>())?,
        )?;
    for submesh in &mesh.submeshes {
        bytes = bytes.checked_add(submesh.material_slot.capacity())?;
    }
    for slot in &mesh.material_slots {
        bytes = bytes.checked_add(slot.capacity())?;
    }
    // Keep the existing four-copy metadata allowance. Numeric JSON decode
    // scratch is not metadata copied into each prepared renderer.
    bytes.checked_mul(4)
}

// Material preparation clones typed metadata; the worker's JSON decoder
// envelope and shared dependency pixels are not cloned into each renderer.
pub(super) fn material_metadata_copy_bound(asset: &CookedRenderAsset) -> Option<usize> {
    let RenderAssetData::Material(material) = &asset.asset else {
        return None;
    };
    fn map_nodes<V>(map: &std::collections::BTreeMap<String, V>) -> Option<usize> {
        let len = map.len();
        if len == 0 {
            return Some(0);
        }
        // The pinned std BTree node stores at most 11 entries. A non-root
        // node has at least 5; ceil(len/5) also covers a sparse one-entry root.
        // Include all internal edges and header padding for every node.
        let node = std::mem::size_of::<(String, V)>()
            .checked_mul(11)?
            .checked_add(16 * std::mem::size_of::<usize>())?;
        len.div_ceil(5).checked_mul(node)
    }
    let mut bytes = std::mem::size_of::<CookedRenderAsset>()
        .checked_add(asset.source_digest.capacity())?
        .checked_add(asset.recipe.capacity())?
        .checked_add(material.shader_source.capacity())?
        .checked_add(map_nodes(&material.parameters)?)?
        .checked_add(map_nodes(&material.textures)?)?
        .checked_add(map_nodes(&material.texture_sampling)?)?;
    for key in material
        .parameters
        .keys()
        .chain(material.texture_sampling.keys())
    {
        bytes = bytes.checked_add(key.capacity())?;
    }
    for (key, texture) in &material.textures {
        bytes = bytes.checked_add(key.capacity())?;
        let r = &texture.reference;
        for s in [&r.id, &r.asset_type].into_iter().chain(
            [r.guid.as_ref(), r.sub_asset.as_ref()]
                .into_iter()
                .flatten(),
        ) {
            bytes = bytes.checked_add(s.capacity())?;
        }
    }
    // Preserve the existing metadata clone allowance; no texture pixel data
    // or discarded JSON tree is retained by the material descriptor clone.
    bytes.checked_mul(4)
}

fn bound(
    source_bytes: usize,
    arrays: &[(usize, usize, usize)],
    ceiling: usize,
) -> Result<usize, StreamingError> {
    let mut metadata = source_bytes;
    let mut vectors = 0usize;
    for &(raw_bytes, count, element_bytes) in arrays {
        metadata = metadata
            .checked_sub(raw_bytes)
            .ok_or(StreamingError::BudgetExceeded)?;
        vectors = vectors
            .checked_add(vector_bound(count, element_bytes)?)
            .ok_or(StreamingError::BudgetExceeded)?;
    }
    let charge = metadata
        .checked_mul(64)
        .and_then(|bytes| bytes.checked_add(vectors))
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .ok_or(StreamingError::BudgetExceeded)?;
    if charge > ceiling {
        return Err(StreamingError::BudgetExceeded);
    }
    Ok(charge)
}

#[derive(Deserialize)]
struct RawCharacter<'a> {
    #[serde(borrow)]
    skins: &'a RawValue,
}
#[derive(Deserialize)]
struct RawSkin<'a> {
    #[serde(borrow)]
    influences: &'a RawValue,
}

pub(super) fn decode_character(
    bytes: &[u8],
    ceiling: usize,
) -> Result<(CharacterAsset, usize), StreamingError> {
    lexical_bounds(bytes, ceiling)?;
    let raw: RawCharacter<'_> = serde_json::from_slice(bytes).map_err(invalid)?;
    // Visit one borrowed skin at a time: the admission pass must not allocate a
    // Vec of skins or a JSON tree before the complete bound is known.
    struct Skins;
    impl<'de> Visitor<'de> for Skins {
        type Value = (usize, usize);
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("character skins")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            use serde::de::Error;
            let mut raw_bytes = 0usize;
            let mut vectors = 0usize;
            while let Some(skin) = sequence.next_element::<RawSkin<'de>>()? {
                let count = array_len::<VertexInfluence>(skin.influences)
                    .map_err(|e| A::Error::custom(format!("{e:?}")))?;
                let charge = vector_bound(count, std::mem::size_of::<VertexInfluence>())
                    .map_err(|e| A::Error::custom(format!("{e:?}")))?;
                raw_bytes = raw_bytes
                    .checked_add(skin.influences.get().len())
                    .ok_or_else(|| A::Error::custom("Character array size overflow"))?;
                vectors = vectors
                    .checked_add(charge)
                    .ok_or_else(|| A::Error::custom("Character vector size overflow"))?;
            }
            Ok((raw_bytes, vectors))
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(raw.skins.get());
    let (raw_arrays, vectors) = decoder.deserialize_seq(Skins).map_err(invalid)?;
    decoder.end().map_err(invalid)?;
    let metadata = bytes
        .len()
        .checked_sub(raw_arrays)
        .ok_or(StreamingError::BudgetExceeded)?;
    // Only fixed-size influences use the Vec growth bound. Joints, clips,
    // morphs, strings and all other metadata keep the existing 64x envelope.
    let charge = bound(metadata, &[], ceiling)?
        .checked_add(vectors)
        .ok_or(StreamingError::BudgetExceeded)?;
    if charge > ceiling {
        return Err(StreamingError::BudgetExceeded);
    }
    let asset: CharacterAsset = serde_json::from_slice(bytes).map_err(invalid)?;
    asset.validate().map_err(invalid)?;
    Ok((asset, charge))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawContract<'a> {
    #[serde(borrow)]
    schema_version: &'a RawValue,
    #[serde(borrow)]
    data: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawCooked<'a> {
    #[serde(borrow)]
    source_digest: &'a RawValue,
    #[serde(borrow)]
    recipe: &'a RawValue,
    #[serde(borrow)]
    asset: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAsset<'a> {
    #[serde(borrow)]
    kind: &'a RawValue,
    #[serde(borrow)]
    payload: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMeshArrays<'a> {
    #[serde(borrow)]
    vertex_bytes: &'a RawValue,
    #[serde(borrow)]
    index_bytes: &'a RawValue,
}

pub(super) fn decode_mesh(
    bytes: &[u8],
    ceiling: usize,
) -> Result<(CookedRenderAsset, usize), StreamingError> {
    lexical_bounds(bytes, ceiling)?;
    let contract: RawContract<'_> = serde_json::from_slice(bytes).map_err(invalid)?;
    let cooked: RawCooked<'_> = serde_json::from_str(contract.data.get()).map_err(invalid)?;
    let asset: RawAsset<'_> = serde_json::from_str(cooked.asset.get()).map_err(invalid)?;
    let arrays: RawMeshArrays<'_> = serde_json::from_str(asset.payload.get()).map_err(invalid)?;
    let charge = bound(
        bytes.len(),
        &[
            (
                arrays.vertex_bytes.get().len(),
                array_len::<u8>(arrays.vertex_bytes)?,
                1,
            ),
            (
                arrays.index_bytes.get().len(),
                array_len::<u8>(arrays.index_bytes)?,
                1,
            ),
        ],
        ceiling,
    )?;
    let _: RenderContractVersion =
        serde_json::from_str(contract.schema_version.get()).map_err(invalid)?;
    let kind: String = serde_json::from_str(asset.kind.get()).map_err(invalid)?;
    if kind != "mesh" {
        return Err(invalid("Large mesh payload has another render kind"));
    }
    let geometry: CookedGeometry = serde_json::from_str(asset.payload.get()).map_err(invalid)?;
    let asset = CookedRenderAsset {
        source_digest: serde_json::from_str(cooked.source_digest.get()).map_err(invalid)?,
        recipe: serde_json::from_str(cooked.recipe.get()).map_err(invalid)?,
        asset: RenderAssetData::Mesh(geometry),
    };
    asset.validate().map_err(invalid)?;
    Ok((asset, charge))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawCollision<'a> {
    #[serde(borrow)]
    schema_version: &'a RawValue,
    #[serde(borrow)]
    shape: &'a RawValue,
}
#[derive(Deserialize)]
struct RawCollisionArrays<'a> {
    #[serde(borrow)]
    kind: &'a RawValue,
    #[serde(borrow)]
    vertices: &'a RawValue,
    #[serde(borrow)]
    triangles: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StaticMesh {
    kind: String,
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
}

pub(super) fn decode_collision(
    bytes: &[u8],
    ceiling: usize,
) -> Result<(CookedCollision3D, usize), StreamingError> {
    lexical_bounds(bytes, ceiling)?;
    let raw: RawCollision<'_> = serde_json::from_slice(bytes).map_err(invalid)?;
    let arrays: RawCollisionArrays<'_> = serde_json::from_str(raw.shape.get()).map_err(invalid)?;
    let charge = bound(
        bytes.len(),
        &[
            (
                arrays.vertices.get().len(),
                array_len::<IgnoredAny>(arrays.vertices)?,
                12,
            ),
            (
                arrays.triangles.get().len(),
                array_len::<IgnoredAny>(arrays.triangles)?,
                12,
            ),
        ],
        ceiling,
    )?;
    let kind: String = serde_json::from_str(arrays.kind.get()).map_err(invalid)?;
    if kind != "staticMesh" {
        return Err(invalid("Large collision payload is not a static mesh"));
    }
    let shape: StaticMesh = serde_json::from_str(raw.shape.get()).map_err(invalid)?;
    debug_assert_eq!(shape.kind, kind);
    let asset = CookedCollision3D {
        schema_version: serde_json::from_str(raw.schema_version.get()).map_err(invalid)?,
        shape: CollisionShape3D::StaticMesh {
            vertices: shape.vertices,
            triangles: shape.triangles,
        },
    };
    asset.validate().map_err(invalid)?;
    Ok((asset, charge))
}
