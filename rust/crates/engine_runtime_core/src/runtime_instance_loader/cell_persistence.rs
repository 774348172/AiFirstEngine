use super::*;
use crate::{
    archetype::ComponentValue, component_value::RuntimeValue, components::ComponentTypeId,
    field_path::FieldPath, world_persistence::*,
};
impl RuntimeInstanceLoader {
    pub fn capture_partition_logical(
        &self,
        declaration: &WorldLogicalDeclaration3D,
        world: &World,
    ) -> crate::spatial_contract::ContractResult<WorldLogicalSave3D> {
        validate_declaration(declaration)?;
        let mut records = Vec::new();
        for placement in &declaration.placements {
            let entry = self.logical_entry(placement, world)?;
            let instance = &self.prefab_instances[&entry.ticket.instance];
            for (source, id) in &instance.source_to_world_entity {
                for field in &declaration.fields {
                    let Some(ComponentValue::Dynamic { value, .. }) =
                        world.component_value(id, &ComponentTypeId::new(&field.component))
                    else {
                        continue;
                    };
                    let mut v = &value;
                    for segment in FieldPath::parse(&field.field).unwrap().segments() {
                        let RuntimeValue::Object(fields) = v else {
                            return Err(invalid("Declared field is absent."));
                        };
                        v = fields
                            .get(segment)
                            .ok_or_else(|| invalid("Declared field is absent."))?;
                    }
                    let value = crate::project_runtime_native_adapter::project_value(v.clone());
                    if !validate_value(&value, 0) {
                        return Err(invalid(
                            "Runtime entity handles or oversized values cannot be stored.",
                        ));
                    }
                    records.push(WorldLogicalRecord3D {
                        placement: placement.clone(),
                        prefab_guid: entry.root.asset_guid.clone(),
                        source_entity: source.to_string(),
                        component: field.component.clone(),
                        field: field.field.clone(),
                        value,
                    });
                    if records.len() > 4096 {
                        return Err(invalid("Save field budget exceeded."));
                    }
                }
            }
        }
        let save = WorldLogicalSave3D {
            schema_version: "world-logical-save.v1".into(),
            world_id: declaration.world_id.clone(),
            revision: declaration.revision,
            declaration_digest: declaration_digest(declaration),
            records,
        };
        decode_logical_save(&serde_json::to_vec(&save).map_err(|e| invalid(e.to_string()))?)
    }
    fn logical_entry(
        &self,
        placement: &str,
        world: &World,
    ) -> crate::spatial_contract::ContractResult<&Entry> {
        let entry = self
            .cells
            .entries
            .values()
            .find(|e| e.request.placement_id == placement && e.status == CellStatus::Active)
            .ok_or_else(|| invalid("A declared partition is not active."))?;
        self.cell_context(entry.ticket, world)
            .map_err(|_| invalid("Partition belongs to a stale World."))?;
        let instance = self
            .prefab_instances
            .get(&entry.ticket.instance)
            .ok_or_else(|| invalid("Partition instance is absent."))?;
        if instance.source_to_world_entity.iter().any(|(source, id)| {
            world.runtime_id_for_source(id)
                != instance.source_to_runtime_entity.get(source).copied()
        }) {
            return Err(invalid("Partition incarnation changed."));
        }
        Ok(entry)
    }
    pub fn restore_partition_logical(
        &self,
        declaration: &WorldLogicalDeclaration3D,
        bytes: &[u8],
        world: &mut World,
    ) -> crate::spatial_contract::ContractResult<()> {
        validate_declaration(declaration)?;
        let save = decode_logical_save(bytes)?;
        if save.world_id != declaration.world_id
            || save.revision != declaration.revision
            || save.declaration_digest != declaration_digest(declaration)
        {
            return Err(invalid(
                "Save world/source revision or declared schema changed.",
            ));
        }
        // Capture the exact expected logical layout of the new incarnations first.
        // Missing, duplicate, extra or mistyped fields reject before the one World commit.
        let expected = self.capture_partition_logical(declaration, world)?;
        let key = |r: &WorldLogicalRecord3D| {
            (
                r.placement.clone(),
                r.prefab_guid.clone(),
                r.source_entity.clone(),
                r.component.clone(),
                r.field.clone(),
            )
        };
        let layout: BTreeMap<_, _> = expected
            .records
            .iter()
            .map(|r| (key(r), std::mem::discriminant(&r.value)))
            .collect();
        if layout.len() != save.records.len() {
            return Err(invalid("Save field layout changed."));
        }
        let mut seen = BTreeSet::new();
        let mut batch = crate::project_runtime_session::ProjectRuntimeMutationBuffer::new();
        for r in save.records {
            let k = key(&r);
            if !seen.insert(k.clone()) || layout.get(&k) != Some(&std::mem::discriminant(&r.value))
            {
                return Err(invalid("Duplicate, foreign or mistyped saved field."));
            }
            let entry = self.logical_entry(&r.placement, world)?;
            let id = self.prefab_instances[&entry.ticket.instance]
                .source_to_world_entity
                .get(&SourceEntityId::from(r.source_entity))
                .ok_or_else(|| invalid("Saved source is absent."))?
                .clone();
            batch.write_component_field(
                id,
                ComponentTypeId::new(&r.component),
                FieldPath::parse(r.field).unwrap(),
                crate::project_runtime_native_adapter::runtime_value(r.value).map_err(invalid)?,
            );
        }
        batch
            .prepare(world)
            .map_err(|e| invalid(e.message))?
            .commit(world)
            .map_err(|e| invalid(e.message))?;
        Ok(())
    }
}
