//! Bounded logical save contract on the existing cell/source identity mapping.
use crate::{
    components::{ComponentRegistry, ComponentTypeId},
    field_path::FieldPath,
    spatial_contract::{ContractResult, SpatialContractError},
};
pub use project_runtime_sdk::{
    WorldLogicalDeclaration3D, WorldLogicalField3D, WorldLogicalRecord3D, WorldLogicalSave3D,
    WorldPersistence3D,
};
use std::collections::BTreeSet;
pub(crate) fn invalid(message: impl Into<String>) -> SpatialContractError {
    SpatialContractError::new("world_save.invalid", "/snapshot", message)
}
pub fn validate_declaration(d: &WorldLogicalDeclaration3D) -> ContractResult<()> {
    if d.world_id.trim().is_empty()
        || d.world_id.len() > 128
        || d.revision == 0
        || d.placements.is_empty()
        || d.placements.len() > 128
        || d.fields.is_empty()
        || d.fields.len() > 64
    {
        return Err(invalid(
            "Bounded world identity, placements and declared fields are required.",
        ));
    }
    let mut placements = BTreeSet::new();
    let mut fields = BTreeSet::new();
    for p in &d.placements {
        if p.trim().is_empty() || p.len() > 256 || !placements.insert(p) {
            return Err(invalid("Duplicate or invalid placement."));
        }
    }
    for f in &d.fields {
        if f.component.trim().is_empty()
            || f.component.len() > 128
            || f.component.starts_with("engine.")
            || ComponentRegistry::new().contains(&ComponentTypeId::new(&f.component))
            || FieldPath::parse(&f.field).is_err()
            || f.field.len() > 128
            || !fields.insert((&f.component, &f.field))
        {
            return Err(invalid(
                "Only unique declared project logical fields may be stored.",
            ));
        }
    }
    Ok(())
}
pub(crate) fn declaration_digest(d: &WorldLogicalDeclaration3D) -> String {
    crate::canonical_digest::sha256_prefixed(&serde_json::to_vec(d).unwrap())
}
pub(crate) fn validate_value(v: &project_runtime_sdk::ProjectRuntimeValue, depth: usize) -> bool {
    use project_runtime_sdk::ProjectRuntimeValue as V;
    if depth > 8 {
        return false;
    }
    match v {
        V::EntityRef(_) => false,
        V::Number(v) => v.is_finite(),
        V::String(v) | V::AssetRef(v) => v.len() <= 4096,
        V::Vec2(v) => v.iter().all(|n| n.is_finite()),
        V::Vec3(v) => v.iter().all(|n| n.is_finite()),
        V::Color(v) => v.iter().all(|n| n.is_finite()),
        V::Array(a) => a.len() <= 256 && a.iter().all(|v| validate_value(v, depth + 1)),
        V::Object(o) => {
            o.len() <= 64
                && o.iter()
                    .all(|(k, v)| k.len() <= 128 && validate_value(v, depth + 1))
        }
        _ => true,
    }
}
pub fn decode_logical_save(bytes: &[u8]) -> ContractResult<WorldLogicalSave3D> {
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("Save document exceeds 1 MiB."));
    }
    let v = crate::spatial_contract::strict_json(bytes)?;
    let save: WorldLogicalSave3D = serde_json::from_value(v).map_err(|e| invalid(e.to_string()))?;
    if save.schema_version != "world-logical-save.v1"
        || save.records.len() > 4096
        || save.records.iter().any(|r| !validate_value(&r.value, 0))
    {
        return Err(invalid(
            "Invalid save version, field budget or logical value.",
        ));
    }
    Ok(save)
}
pub fn validate_binding(b: &WorldPersistence3D) -> ContractResult<()> {
    if b.schema_version != "world-persistence3d.v1" {
        return Err(invalid("Unsupported persistence binding."));
    }
    validate_declaration(&b.declaration)?;
    if let Some(s) = &b.restore {
        decode_logical_save(&serde_json::to_vec(s).map_err(|e| invalid(e.to_string()))?)?;
    }
    Ok(())
}
pub(crate) fn prepare_request(
    world: &crate::world::World,
    overlay: &std::collections::BTreeMap<
        (crate::ids::EntityId, ComponentTypeId),
        crate::archetype::ComponentValue,
    >,
    id: &crate::ids::EntityId,
    request_id: u64,
    snapshot: Option<WorldLogicalSave3D>,
) -> ContractResult<crate::archetype::ComponentValue> {
    use crate::{
        archetype::ComponentValue,
        character_animation::{dynamic_json, runtime_json},
    };
    let ty = ComponentTypeId::new("engine.world_persistence3d");
    let value = overlay
        .get(&(id.clone(), ty.clone()))
        .cloned()
        .or_else(|| world.component_value(id, &ty))
        .ok_or_else(|| invalid("Persistence declaration absent."))?;
    let ComponentValue::Dynamic { value, .. } = value else {
        return Err(invalid("Expected persistence declaration."));
    };
    let mut binding: WorldPersistence3D =
        serde_json::from_value(dynamic_json(&value)).map_err(|e| invalid(e.to_string()))?;
    if request_id == 0 || request_id <= binding.request_id {
        return Err(invalid("Persistence request IDs must increase."));
    }
    binding.request_id = request_id;
    binding.restore = snapshot;
    validate_binding(&binding)?;
    Ok(ComponentValue::Dynamic {
        component_type: ty,
        value: runtime_json(serde_json::to_value(binding).unwrap()),
    })
}
