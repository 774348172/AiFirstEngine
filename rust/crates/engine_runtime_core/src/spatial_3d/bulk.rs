//! Immediate typed writes reuse the current World's slot/row and publication owner.
//! All plans are local to the exclusive commit; no location or proof survives it.
use super::*;
use project_runtime_sdk::{spatial_bulk::SpatialEntityKey, ProjectRuntimeTransform3D};

struct SimpleWrite {
    entity_id: EntityId,
    runtime_id: RuntimeEntityId,
    location: super::super::EntityLocation,
    node: Node,
}

// A successful rotation/scale check is pure and can be shared only inside this
// exclusive commit. Translation is always checked, including duplicate inputs
// that a later overlay replaces. No entity/generation or world proof is cached.
type LocalLinearCheck = ([f32; 4], [f32; 3], CanonicalTransform3D);

fn canonical_local(
    transform: &ProjectRuntimeTransform3D,
    previous: &mut Option<LocalLinearCheck>,
) -> crate::spatial_contract::ContractResult<CanonicalTransform3D> {
    if let Some((_, _, canonical)) = previous.as_ref().filter(|(rotation, scale, _)| {
        *rotation == transform.rotation_xyzw && *scale == transform.scale
    }) {
        crate::spatial_contract::finite(&transform.translation, "/translation")?;
        return Ok(CanonicalTransform3D {
            translation: transform.translation,
            ..canonical.clone()
        });
    }
    let value = CanonicalTransform3D {
        translation: transform.translation,
        rotation_xyzw: crate::spatial_contract::canonical_quaternion(transform.rotation_xyzw)?,
        scale: transform.scale,
    };
    value.validate()?;
    *previous = Some((transform.rotation_xyzw, transform.scale, value.clone()));
    Ok(value)
}

impl World {
    pub(super) fn try_commit_simple_spatial(
        &mut self,
        updates: &[(SpatialEntityKey, ProjectRuntimeTransform3D)],
    ) -> Result<bool, WorldMutationError> {
        // The normal owner handles trees and any unrelated pending dirty work.
        if !self.spatial_dirty.is_empty() {
            return Ok(false);
        }
        // A successful final overlay has unique runtime IDs. Matching its entire
        // sequence proves only input uniqueness, never that these keys are live.
        let unchanged_unique_input =
            self.spatial_timeline
                .simple_order
                .as_ref()
                .is_some_and(|(ids, _)| {
                    ids.len() == updates.len()
                        && ids.iter().zip(updates).all(|(id, (key, _))| {
                            *id == RuntimeEntityId::new(key.index, key.generation)
                        })
                });
        let mut positions = (!unchanged_unique_input)
            .then(|| std::collections::HashMap::with_capacity(updates.len()));
        let mut final_rows = Vec::with_capacity(updates.len());
        let mut local_check = None;
        for (key, transform) in updates {
            let slot = self.resolve_spatial_slot(*key)?;
            let id = &slot.source_id;
            let table = self
                .archetypes
                .get(&slot.location.archetype_id)
                .expect("live row table");
            let Some(mut node) = self.spatial_node_at(id, table, slot.location.row) else {
                return Ok(false);
            };
            if node.transform.is_none()
                || node.parent.is_some()
                || self
                    .hierarchy_children
                    .get(id)
                    .is_some_and(|children| !children.is_empty())
            {
                return Ok(false);
            }
            let canonical = canonical_local(transform, &mut local_check)
                .map_err(|e| failure(id, "three_d.transform_invalid", e.to_string()))?;
            node.transform = Some(canonical);
            let runtime_id = RuntimeEntityId::new(key.index, key.generation);
            let row = SimpleWrite {
                entity_id: id.clone(),
                runtime_id,
                location: slot.location,
                node,
            };
            if let Some(positions) = &mut positions {
                #[cfg(test)]
                SPATIAL_OVERLAY_ENTRIES.with(|n| n.set(n.get() + 1));
                match positions.entry(runtime_id) {
                    std::collections::hash_map::Entry::Occupied(position) => {
                        final_rows[*position.get()] = row;
                    }
                    std::collections::hash_map::Entry::Vacant(position) => {
                        position.insert(final_rows.len());
                        final_rows.push(row);
                    }
                }
            } else {
                final_rows.push(row);
            }
        }
        // Invalid earlier TRS values fail above; duplicate targets evaluate the final
        // overlay only, just like the general transaction (including singularity).
        // Sort compact indices, not full row payloads. Validation and publication
        // retain exactly the source order of the general transaction.
        let cached_order = self
            .spatial_timeline
            .simple_order
            .as_ref()
            .filter(|(ids, _)| {
                ids.len() == final_rows.len()
                    && ids
                        .iter()
                        .zip(&final_rows)
                        .all(|(id, row)| *id == row.runtime_id)
            });
        let order_reused = cached_order.is_some();
        let order = if let Some((_, order)) = cached_order {
            order.clone()
        } else {
            let mut order = (0..final_rows.len()).collect::<Vec<_>>();
            order.sort_unstable_by(|&a, &b| {
                #[cfg(test)]
                SPATIAL_SORT_COMPARISONS.with(|n| n.set(n.get() + 1));
                final_rows[a].entity_id.cmp(&final_rows[b].entity_id)
            });
            order
        };
        let mut linear_check = None;
        let evaluated = order
            .iter()
            .map(|&index| {
                let row = &final_rows[index];
                evaluate_validated_node_with_linear_check(
                    &row.entity_id,
                    &row.node,
                    row.node.transform.as_ref().expect("validated local"),
                    None,
                    &mut linear_check,
                )
            })
            .collect::<Result<Vec<_>, WorldMutationError>>()?;
        self.charge_spatial_budget(updates.len(), 0);
        let already_dirty = self
            .dirty_records
            .iter()
            .filter(|r| r.dirty_type == DirtyType::RenderState)
            .map(|r| r.entity_id.clone())
            .collect();
        let mut changes = Vec::with_capacity(final_rows.len());
        // There is no fallible work after this point. Publish each validated
        // independent row directly; no second large input Vec is necessary.
        for (index, state) in order.iter().copied().zip(evaluated) {
            let row = &final_rows[index];
            let local = row.node.transform.as_ref().expect("validated World3D row");
            let table = self
                .archetypes
                .get_mut(&row.location.archetype_id)
                .expect("exclusive validated table");
            let old = table
                .transform3d(row.location.row)
                .expect("exclusive validated transform");
            if old == local {
                continue;
            }
            let mask = transform_field_mask(old, local);
            assert!(
                table.replace_transform3d(row.location.row, local.clone()),
                "value writes preserve row and signature"
            );
            self.dirty_records.push(DirtyRecord {
                entity_id: row.entity_id.clone(),
                dirty_type: DirtyType::Transform,
            });
            self.publish_spatial_state(
                SpatialPublishInput {
                    entity_id: &row.entity_id,
                    runtime_id: row.runtime_id,
                    local: local.clone(),
                    parent: None,
                    state,
                    mask,
                    explicit: true,
                },
                &already_dirty,
                &mut changes,
            );
        }
        if changes.is_empty() {
            self.propagate_spatial();
        } else {
            self.spatial_timeline.merge_delta(changes.len(), changes);
        }
        if order_reused {
            // Keep the identical successful permutation and its backing.
        } else if final_rows.len() <= 16384 {
            self.spatial_timeline.simple_order =
                Some((final_rows.iter().map(|row| row.runtime_id).collect(), order));
        } else {
            self.spatial_timeline.simple_order = None;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod local_validation_tests {
    use super::*;

    #[test]
    fn shared_linear_validation_matches_scalar_errors_and_never_hides_bad_translation() {
        use crate::spatial_contract::{canonical_quaternion, ContractResult};
        let scalar = |input: &ProjectRuntimeTransform3D| -> ContractResult<CanonicalTransform3D> {
            let value = CanonicalTransform3D {
                translation: input.translation,
                rotation_xyzw: canonical_quaternion(input.rotation_xyzw)?,
                scale: input.scale,
            };
            value.validate()?;
            Ok(value)
        };
        let mut previous = None;
        for i in 0..128 {
            let q = [0., (i as f32 * 0.05).sin(), 0., (i as f32 * 0.05).cos()];
            for scale in [[1.; 3], [-2., 3., 0.5], [f32::MIN_POSITIVE; 3], [0.; 3]] {
                let good = ProjectRuntimeTransform3D {
                    translation: [i as f32, -0., 1.],
                    rotation_xyzw: q,
                    scale,
                };
                for input in [
                    good.clone(),
                    good.clone(),
                    ProjectRuntimeTransform3D {
                        translation: [f32::NAN, 0., 0.],
                        ..good.clone()
                    },
                    ProjectRuntimeTransform3D {
                        translation: [0., f32::INFINITY, 0.],
                        ..good.clone()
                    },
                    ProjectRuntimeTransform3D {
                        rotation_xyzw: [0.; 4],
                        ..good.clone()
                    },
                    good,
                ] {
                    let expected = scalar(&input);
                    let actual = canonical_local(&input, &mut previous);
                    match (expected, actual) {
                        (Ok(a), Ok(b)) => assert_eq!(a, b),
                        (Err(a), Err(b)) => {
                            assert_eq!(a.code, b.code);
                            assert_eq!(a.field_path, b.field_path);
                            assert_eq!(a.message, b.message);
                        }
                        (a, b) => panic!("scalar/shared mismatch: {a:?} / {b:?}"),
                    }
                }
            }
        }
    }
}
