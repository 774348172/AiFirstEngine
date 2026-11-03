use super::*;
use crate::gameplay_command::RuntimeCommandContext;
use crate::runtime_package::RuntimeAssetRef;
use std::collections::BTreeSet;

type CommitWithReceipts = (
    ProjectRuntimeMutationCommitReport,
    Option<BTreeMap<u64, (EntityId, u64)>>,
);

impl ProjectRuntimeMutationBuffer {
    pub(crate) fn commit_with_runtime(
        mut self,
        world: &mut World,
        runtime: Option<RuntimeCommandContext<'_>>,
    ) -> Result<CommitWithReceipts, ProjectRuntimeMutationError> {
        let count = self.len();
        if !self.generated_content.is_empty() {
            let Some(runtime) = runtime else {
                return Err(ProjectRuntimeMutationError::unsupported(0, count));
            };
            let intents = std::mem::take(&mut self.generated_content);
            // Existing synchronous preflight/commit must succeed before any asynchronous
            // demand is admitted. Admission errors affect that demand, not other requests.
            let (mut report, receipts) = self.commit_with_runtime(
                world,
                Some(RuntimeCommandContext {
                    package: runtime.package,
                    instance_loader: &mut *runtime.instance_loader,
                }),
            )
            .map_err(|mut error| {
                error.report.staged_count += intents.len();
                error.report.rejected_count += intents.len();
                error
            })?;
            generated_content::admit(intents, runtime.instance_loader, world, &mut report);
            return Ok((report, receipts));
        }
        let rejected = |code, message: String| ProjectRuntimeMutationError {
            code,
            operation_index: 0,
            message,
            report: ProjectRuntimeMutationCommitReport::rejected(count),
        };
        if !self.spatial_transforms.is_empty()
            || self.operations.iter().any(|op| {
                matches!(
                    op,
                    ProjectRuntimeMutationOperation::SpatialLocalTransform { .. }
                )
            })
        {
            let owned = world
                .query_entities(&crate::query::QuerySpec::all([ComponentTypeId::new(
                    "engine.body3d",
                )]))
                .into_iter()
                .filter(|id| crate::physics_3d::project_transform_owned(world, id))
                .filter_map(|id| world.spatial_entity_key(&id))
                .map(|k| (k.world_epoch, k.index, k.generation))
                .collect::<std::collections::HashSet<_>>();
            if self.spatial_transforms.iter().any(|(key,_)|owned.contains(&(key.world_epoch,key.index,key.generation))) || self.operations.iter().any(|op|matches!(op,ProjectRuntimeMutationOperation::SpatialLocalTransform{entity,..} if owned.contains(&(entity.world_epoch,entity.index,entity.generation)))) {
                return Err(rejected("physics3d.transform_owned","Dynamic body transforms require physics commands or an explicit teleport.".into()));
            }
        }
        if !self.spatial_transforms.is_empty()
            && self.operations.is_empty()
            && self.animator2d_commands.is_empty()
            && self.animator_commands.is_empty()
            && self.audio_source_intents.is_empty()
            && self.particle_intents.is_empty()
        {
            world
                .commit_spatial_local_batch(self.spatial_transforms)
                .map_err(|e| ProjectRuntimeMutationError::from_world_error(0, count, e))?;
            let mut report = ProjectRuntimeMutationCommitReport::rejected(count);
            report.committed_count = count;
            report.rejected_count = 0;
            return Ok((report, None));
        }
        self.expand_spatial_transforms();
        if self
            .operations
            .iter()
            .any(|op| matches!(op, ProjectRuntimeMutationOperation::Cell(_)))
        {
            if self
                .operations
                .iter()
                .any(|op| !matches!(op, ProjectRuntimeMutationOperation::Cell(_)))
                || !self.animator2d_commands.is_empty()
                || !self.animator_commands.is_empty()
                || !self.audio_source_intents.is_empty()
                || !self.particle_intents.is_empty()
            {
                return Err(rejected(
                    "cell.separate_callback_required",
                    "Submit asynchronous cell demands separately from synchronous mutations."
                        .into(),
                ));
            }
            let runtime = runtime.ok_or_else(|| {
                rejected(
                    "cell.runtime_context_required",
                    "Cell commands require a RuntimePackage session.".into(),
                )
            })?;
            let intents = self
                .operations
                .into_iter()
                .map(|op| match op {
                    ProjectRuntimeMutationOperation::Cell(intent) => intent,
                    _ => unreachable!(),
                })
                .collect();
            runtime
                .instance_loader
                .accept_project_cells(intents, world)
                .map_err(|code| rejected(code, code.into()))?;
            let mut report = ProjectRuntimeMutationCommitReport::rejected(count);
            report.committed_count = count;
            report.rejected_count = 0;
            return Ok((report, None));
        }
        if !self.operations.is_empty()
            && self.animator2d_commands.is_empty()
            && self.animator_commands.is_empty()
            && self.audio_source_intents.is_empty()
            && self.particle_intents.is_empty()
            && self.operations.iter().all(|op| {
                matches!(
                    op,
                    ProjectRuntimeMutationOperation::SpatialLocalTransform { .. }
                )
            })
        {
            let updates = self
                .operations
                .into_iter()
                .map(|op| match op {
                    ProjectRuntimeMutationOperation::SpatialLocalTransform {
                        entity,
                        transform,
                    } => (entity, transform),
                    _ => unreachable!("pure typed spatial batch"),
                })
                .collect();
            world
                .commit_spatial_local_batch(updates)
                .map_err(|e| ProjectRuntimeMutationError::from_world_error(0, count, e))?;
            let mut report = ProjectRuntimeMutationCommitReport::rejected(count);
            report.committed_count = count;
            report.rejected_count = 0;
            return Ok((report, None));
        }
        for op in &self.operations {
            if let ProjectRuntimeMutationOperation::Spatial {
                intent:
                    project_runtime_sdk::ProjectRuntimeSpatialIntent::SetMaterial { material, .. },
                ..
            } = op
            {
                let runtime = runtime.as_ref().ok_or_else(|| {
                    rejected(
                        "three_d.runtime_context_required",
                        "Material binding requires RuntimePackage asset identity.".into(),
                    )
                })?;
                let reference = RuntimeAssetRef {
                    id: material.id.clone(),
                    asset_type: material.asset_type.clone(),
                    guid: material.guid.clone(),
                    sub_asset: material.sub_asset.clone(),
                };
                let record = runtime
                    .package
                    .runtime_asset_index
                    .resolve(&reference)
                    .map_err(|error| {
                        rejected("three_d.material_unresolved", format!("{error:?}"))
                    })?;
                if record.asset_id != reference.id || record.sub_asset_id != reference.sub_asset {
                    return Err(rejected(
                        "three_d.material_unresolved",
                        "Material id/GUID/sub_asset must identify the same package entry.".into(),
                    ));
                }
            }
        }
        if !self.operations.iter().any(|op| {
            matches!(
                op,
                ProjectRuntimeMutationOperation::InstantiateSpatialPrefab { .. }
                    | ProjectRuntimeMutationOperation::DespawnSpatialPrefab { .. }
            )
        }) {
            return self
                .commit_immediate_for_host(world)
                .map(|report| (report, None));
        }
        let mut spawns = Vec::new();
        let mut despawns = Vec::new();
        let mut ordinary = Vec::new();
        for op in self.operations {
            match op {
                ProjectRuntimeMutationOperation::InstantiateSpatialPrefab {
                    request_id,
                    prefab,
                } => spawns.push((request_id, prefab)),
                ProjectRuntimeMutationOperation::DespawnSpatialPrefab {
                    entity_id,
                    generation,
                } => despawns.push((entity_id, generation)),
                other => ordinary.push(other),
            }
        }
        self.operations = ordinary;
        let prepared = self.prepare(world).map_err(|mut error| {
            error.report = ProjectRuntimeMutationCommitReport::rejected(count);
            error
        })?;
        if spawns.is_empty() && despawns.is_empty() {
            return prepared.commit_for_host(world).map(|report| (report, None));
        }
        let Some(runtime) = runtime else {
            return Err(rejected(
                "three_d.runtime_context_required",
                "Prefab commands require the running RuntimePackage and instance loader.".into(),
            ));
        };
        if let Err(error) = world.check_spatial_budget(
            prepared.spatial_count,
            prepared.structure_requests.len() + spawns.len() + despawns.len(),
        ) {
            return Err(rejected(
                "three_d.spatial_budget_exceeded",
                format!("{}: {}", error.code, error.message),
            ));
        }
        if spawns.len() > world.spatial_profile().max_structure_intents {
            return Err(rejected(
                "three_d.spawn_limit",
                "Prefab batch exceeds configured structural capacity.".into(),
            ));
        }
        let mut keys = BTreeSet::new();
        for (key, prefab) in &spawns {
            if *key == 0
                || !keys.insert(*key)
                || prefab.asset_type != "prefab"
                || prefab.id.trim().is_empty()
                || prefab.guid.as_ref().is_none_or(|v| v.trim().is_empty())
            {
                return Err(rejected(
                    "three_d.spawn_request_invalid",
                    "Use unique nonzero request IDs and stable prefab AssetRefs.".into(),
                ));
            }
        }
        let mut instances = BTreeSet::new();
        let mut removed = BTreeSet::new();
        for (id, generation) in &despawns {
            if *generation == 0 || world.entity_generation(id) != Some(*generation) {
                return Err(rejected(
                    "three_d.stale_entity",
                    format!("Resolve current prefab root {id}"),
                ));
            }
            let instance = world
                .runtime_id_for_source(id)
                .and_then(|r| runtime.instance_loader.prefab_instance_for_root(r))
                .ok_or_else(|| {
                    rejected(
                        "three_d.prefab_root_required",
                        format!("{id} is not a live prefab root"),
                    )
                })?;
            if !instances.insert(instance) {
                return Err(rejected(
                    "three_d.duplicate_despawn",
                    format!("Duplicate root {id}"),
                ));
            }
            // Include all instance members, including disconnected roots, for mixed-batch conflicts.
            for member in runtime
                .instance_loader
                .prefab_instance(instance)
                .unwrap()
                .source_to_world_entity
                .values()
            {
                removed.extend(world.descendants(member));
            }
        }
        if prepared.generations.keys().any(|id| removed.contains(id))
            || prepared
                .audio_source_commands
                .iter()
                .any(|v| removed.contains(&v.entity_id))
            || prepared
                .particle_commands
                .iter()
                .any(|v| removed.contains(&v.entity_id))
            || (!removed.is_empty() && !prepared.animator2d_commands.is_empty())
        {
            return Err(rejected(
                "three_d.despawn_write_conflict",
                "A callback cannot mutate entities it destroys; submit in separate callbacks."
                    .into(),
            ));
        }
        let structure_count = spawns.len() + despawns.len();
        let had_spawns = !spawns.is_empty();
        let mut pending = runtime.instance_loader.deferred_prefabs();
        for (key, prefab) in spawns {
            pending
                .prepare(
                    key,
                    RuntimeAssetRef {
                        id: prefab.id,
                        asset_type: prefab.asset_type,
                        guid: prefab.guid,
                        sub_asset: prefab.sub_asset,
                    },
                    runtime.package,
                    world,
                )
                .map_err(|report| {
                    rejected(
                        "three_d.prefab_prepare_failed",
                        format!("{:?}", report.diagnostics),
                    )
                })?;
        }
        // No World changes occurred during preparation. Component commit rechecks all identities.
        // Prefab roots have no external parent; their infallible hydration cannot invalidate these writes.
        let mut report = prepared.commit_for_host(world).map_err(|mut error| {
            error.report = ProjectRuntimeMutationCommitReport::rejected(count);
            error
        })?;
        world.charge_spatial_budget(0, structure_count);
        let receipts = pending.commit(world);
        for instance in instances {
            runtime
                .instance_loader
                .despawn_prefab_instance(instance, world);
        }
        report.staged_count = count;
        report.committed_count = count;
        Ok((report, had_spawns.then_some(receipts)))
    }
}
