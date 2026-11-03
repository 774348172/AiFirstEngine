//! Decoration-to-gameplay transfer commits through the original World overlay.
//! The project owns and spawns the target Prefab; this helper owns no instances,
//! native bodies, renderer resources or asynchronous request registry.
use super::*;
use crate::{
    archetype::ComponentValue,
    component_value::RuntimeValue,
    components::ComponentTypeId,
    field_path::FieldPath,
    project_runtime_session::ProjectRuntimeMutationBuffer,
    terrain3d::{VegetationAsset3D, VegetationBinding3D},
};

fn invalid(message: impl Into<String>) -> CellError {
    CellError::Hydration(format!("vegetation.promotion: {}", message.into()))
}

/// Project-declared dynamic fields; the core does not name a game's tree type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VegetationPromotionFields3D {
    pub component_type: String,
    pub source_id: String,
    pub cluster_key: String,
    pub cluster_asset_id: String,
}

impl RuntimeInstanceLoader {
    /// Atomically transfer one stable decoration source into an already-spawned
    /// independent typed-mesh Prefab root. Source fields must predeclare empty
    /// strings; occupied targets reject. Their component and paths belong to
    /// the caller's project schema, not a core gameplay-type registry.
    /// A NotReady result requires the caller's existing project-cell pending
    /// owner to retain its request; this helper balances its own ready lease.
    pub fn promote_vegetation_to_entity(
        &mut self,
        owner: &SourceEntityId,
        owner_generation: u64,
        source_id: &str,
        target: &SourceEntityId,
        target_generation: u64,
        source_fields: &VegetationPromotionFields3D,
        world: &mut World,
    ) -> Result<(), CellError> {
        if owner == target || source_id.is_empty() || source_id.len() > 256 {
            return Err(CellError::InvalidRequest);
        }
        if world.entity_generation(owner) != Some(owner_generation)
            || world.entity_generation(target) != Some(target_generation)
        {
            return Err(CellError::StaleTicket);
        }
        let owner_type = ComponentTypeId::new("engine.vegetation3d");
        let Some(ComponentValue::Dynamic { value, .. }) = world.component_value(owner, &owner_type)
        else {
            return Err(invalid("Owner needs an existing vegetation binding."));
        };
        let mut binding: VegetationBinding3D =
            serde_json::from_value(crate::character_animation::dynamic_json(&value))
                .map_err(|e| invalid(e.to_string()))?;
        binding
            .cluster_asset
            .validate()
            .map_err(|e| invalid(e.to_string()))?;
        if binding
            .promoted_instance_ids
            .iter()
            .any(|id| id == source_id)
        {
            return Err(CellError::AlreadyCommitted);
        }
        if world.component_value(target, &owner_type).is_some() {
            return Err(invalid("Target cannot be a vegetation binding owner."));
        }
        if world
            .hierarchy(target)
            .is_some_and(|h| h.parent_id.is_some())
        {
            return Err(CellError::ParentChanged);
        }
        if world
            .query_entities(&crate::query::QuerySpec::all(
                [ComponentTypeId::hierarchy()],
            ))
            .iter()
            .any(|id| {
                world
                    .hierarchy(id)
                    .is_some_and(|h| h.parent_id.as_ref() == Some(target))
            })
        {
            return Err(CellError::ForeignChildren);
        }
        for name in [
            "engine.motor3d",
            "engine.character3d",
            "engine.character3d_control",
            "engine.character3d_state",
            "engine.ragdoll3d",
        ] {
            if world
                .component_value(target, &ComponentTypeId::new(name))
                .is_some()
            {
                return Err(invalid(
                    "Target cannot have native character/motor/ragdoll control.",
                ));
            }
        }
        if let Some(body) = world.component_value(target, &ComponentTypeId::new("engine.body3d")) {
            let ComponentValue::Dynamic { value, .. } = body else {
                return Err(invalid("Target body must be a static declaration."));
            };
            let body = crate::character_animation::dynamic_json(&value);
            if body["motion"].as_str() != Some("static") {
                return Err(invalid("Target cannot have a native-controlled body."));
            }
        }
        let renderer = world
            .mesh_renderer3d(target)
            .ok_or_else(|| invalid("Target needs an existing typed mesh renderer."))?;
        if world.transform3d(target).is_none() {
            return Err(invalid("Target needs an existing typed transform."));
        }
        if !source_fields.component_type.starts_with("project.")
            || source_fields.component_type.len() > 128
            || crate::components::ComponentRegistry::new()
                .contains(&ComponentTypeId::new(&source_fields.component_type))
        {
            return Err(invalid("Source component must have a bounded name."));
        }
        let names = [
            &source_fields.source_id,
            &source_fields.cluster_key,
            &source_fields.cluster_asset_id,
        ];
        if names[0] == names[1]
            || names[0] == names[2]
            || names[1] == names[2]
            || names.iter().any(|p| p.len() > 128)
        {
            return Err(invalid("Source field paths must be bounded and distinct."));
        }
        let paths = names
            .iter()
            .map(|p| FieldPath::parse(p.as_str()).map_err(|e| invalid(format!("{e:?}"))))
            .collect::<Result<Vec<_>, _>>()?;
        let source_type = ComponentTypeId::new(&source_fields.component_type);
        let Some(ComponentValue::Dynamic { value, .. }) =
            world.component_value(target, &source_type)
        else {
            return Err(invalid("Target must predeclare its dynamic source fields."));
        };
        for path in &paths {
            let mut current = &value;
            for segment in path.segments() {
                let RuntimeValue::Object(fields) = current else {
                    return Err(invalid("Source field object is absent."));
                };
                current = fields
                    .get(segment)
                    .ok_or_else(|| invalid("Source field is absent."))?;
            }
            if !matches!(current,RuntimeValue::String(value) if value.is_empty()) {
                return Err(invalid(
                    "Target source fields must be existing empty strings.",
                ));
            }
        }
        let state = world
            .spatial_world(owner)
            .ok_or_else(|| invalid("Owner needs a committed spatial world pose."))?;
        let anchor = state.anchor;
        let owner_matrix = state.world_matrix;
        if anchor.iter().any(|v| !v.is_finite()) || owner_matrix.iter().any(|v| !v.is_finite()) {
            return Err(invalid("Owner pose is not finite."));
        }

        let request = RuntimeAssetRef {
            id: binding.cluster_asset.asset_id.clone(),
            asset_type: "vegetation3d".into(),
            guid: Some(binding.cluster_asset.asset_guid.clone()),
            sub_asset: None,
        };
        let handle = self
            .asset_loader
            .retain_ready_reference(&request)
            .map_err(CellError::Asset)?
            .ok_or(CellError::NotReady)?;
        let payload = (|| {
            match self.asset_loader.get_handle_state(&handle) {
                Some(RuntimeAssetLoadState::Loading) => return Err(CellError::NotReady),
                Some(RuntimeAssetLoadState::Ready) => {}
                _ => return Err(invalid("Vegetation source failed or was released.")),
            }
            let bytes = self
                .asset_loader
                .get_asset_bytes(&handle)
                .ok_or(CellError::NotReady)?;
            let strict =
                crate::spatial_contract::strict_json(&bytes).map_err(|e| invalid(e.to_string()))?;
            let asset: VegetationAsset3D =
                serde_json::from_value(strict).map_err(|e| invalid(e.to_string()))?;
            asset
                .validate_binding(&binding)
                .map_err(|e| invalid(e.to_string()))?;
            // Validate the proposed final binding and source before any mutation.
            binding.promoted_instance_ids.push(source_id.into());
            let promotion = asset
                .promote(&binding, source_id)
                .map_err(|e| invalid(e.to_string()))?;
            Ok(promotion)
        })();
        let released = self.asset_loader.release(&handle);
        let promotion = payload?;
        if released.is_err() {
            return Err(invalid("Vegetation source request release failed."));
        }
        if renderer.mesh.0 != promotion.mesh.reference
            || renderer.materials.is_empty()
            || renderer
                .materials
                .values()
                .any(|material| material.0 != promotion.material.reference)
        {
            return Err(invalid(
                "Target mesh/material must exactly match the source cluster.",
            ));
        }
        let matrix = crate::render_contract::multiply_matrix(owner_matrix, promotion.matrix);
        if matrix.iter().any(|v| !v.is_finite()) {
            return Err(invalid("Composed pose is not finite."));
        }
        let transform =
            World::decompose_spatial_matrix(target, matrix).map_err(|e| invalid(e.message))?;
        let mut batch = ProjectRuntimeMutationBuffer::new();
        // Both anchor and TRS enter one final overlay, so large-coordinate roots
        // preserve the owner's double-precision anchor without f32 truncation.
        batch.spatial_command(
            target.clone(),
            target_generation,
            project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
                anchor,
                preserve_world: false,
            },
        );
        batch.spatial_command(
            target.clone(),
            target_generation,
            project_runtime_sdk::ProjectRuntimeSpatialIntent::Teleport {
                transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                    translation: transform.translation,
                    rotation_xyzw: transform.rotation_xyzw,
                    scale: transform.scale,
                },
            },
        );
        batch.replace_component(
            owner.clone(),
            owner_type.clone(),
            ComponentValue::Dynamic {
                component_type: owner_type,
                value: crate::character_animation::runtime_json(
                    serde_json::to_value(&binding).map_err(|e| invalid(e.to_string()))?,
                ),
            },
        );
        for (path, value) in paths.into_iter().zip([
            promotion.source_id,
            promotion.cluster_key,
            binding.cluster_asset.asset_id,
        ]) {
            batch.write_component_field(
                target.clone(),
                source_type.clone(),
                path,
                RuntimeValue::String(value),
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
