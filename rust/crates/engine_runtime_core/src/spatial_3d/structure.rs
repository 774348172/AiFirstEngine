//! Structural spatial changes reuse World's final-overlay transaction boundary.
use super::*;
#[derive(Debug, Clone)]
pub(crate) enum SpatialStructureRequest {
    Reparent {
        entity: EntityId,
        parent: Option<EntityId>,
        preserve_world: bool,
    },
    Anchor {
        entity: EntityId,
        anchor: [f64; 3],
        preserve_world: bool,
    },
}
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedSpatialStructure {
    parents: BTreeMap<EntityId, Option<EntityId>>,
    anchors: BTreeMap<EntityId, [f64; 3]>,
    pub writes: Vec<(EntityId, ComponentValue)>,
}
impl World {
    pub(crate) fn prepare_spatial_structure(
        &self,
        writes: &[(EntityId, ComponentValue)],
        requests: &[SpatialStructureRequest],
    ) -> Result<PreparedSpatialStructure, WorldMutationError> {
        if requests.is_empty() {
            return Ok(PreparedSpatialStructure::default());
        }
        let mut plan = PreparedSpatialStructure::default();
        let mut keep = BTreeMap::new();
        for request in requests {
            let (id, preserve) = match request {
                SpatialStructureRequest::Reparent {
                    entity,
                    parent,
                    preserve_world,
                } => {
                    if self.transform3d(entity).is_none()
                        || parent
                            .as_ref()
                            .is_some_and(|p| self.transform3d(p).is_none())
                    {
                        return Err(failure(
                            entity,
                            "three_d.domain_conflict",
                            "Reparent requires live World3D child and parent",
                        ));
                    }
                    plan.parents.insert(entity.clone(), parent.clone());
                    (entity, *preserve_world)
                }
                SpatialStructureRequest::Anchor {
                    entity,
                    anchor,
                    preserve_world,
                } => {
                    if self.transform3d(entity).is_none()
                        || anchor.iter().any(|v| !v.is_finite() || v.abs() > 1e12)
                    {
                        return Err(failure(
                            entity,
                            "three_d.anchor_invalid",
                            "Anchor must be finite within the frozen domain",
                        ));
                    }
                    plan.anchors.insert(entity.clone(), *anchor);
                    (entity, *preserve_world)
                }
            };
            keep.insert(id.clone(), preserve);
        }
        let parent_of = |id: &EntityId| {
            plan.parents
                .get(id)
                .cloned()
                .unwrap_or_else(|| self.hierarchy(id).and_then(|h| h.parent_id))
        };
        let mut ordered = Vec::new();
        for id in keep.keys() {
            let mut visited = BTreeSet::from([id.clone()]);
            let mut depth = 0;
            let mut parent = parent_of(id);
            while let Some(p) = parent {
                if !visited.insert(p.clone()) {
                    return Err(failure(
                        id,
                        "three_d.parent_cycle",
                        "Final hierarchy contains a cycle",
                    ));
                }
                depth += 1;
                parent = parent_of(&p);
            }
            if plan.anchors.contains_key(id) && parent_of(id).is_some() {
                return Err(failure(
                    id,
                    "three_d.anchor_requires_root",
                    "Anchors belong to hierarchy roots",
                ));
            }
            ordered.push((depth, id.clone()));
        }
        ordered.sort();
        let mut values = writes
            .iter()
            .map(|(id, v)| ((id.clone(), v.component_type()), v.clone()))
            .collect::<BTreeMap<_, _>>();
        // Source position is captured after ordinary final writes, before structural migration.
        let base_nodes = |id: &EntityId| {
            let mut n = self.spatial_node(id)?;
            if let Some(ComponentValue::Transform3D(t)) =
                values.get(&(id.clone(), ComponentTypeId::transform3d()))
            {
                n.transform = Some(t.clone());
            }
            Some(n)
        };
        let mut source = BTreeMap::new();
        for (_, id) in &ordered {
            evaluate(id, &base_nodes, &mut source, &mut BTreeSet::new())?;
        }
        for (_, id) in ordered {
            if !keep[&id] {
                continue;
            }
            let nodes = |key: &EntityId| {
                let mut n = self.spatial_node(key)?;
                if let Some(ComponentValue::Transform3D(t)) =
                    values.get(&(key.clone(), ComponentTypeId::transform3d()))
                {
                    n.transform = Some(t.clone());
                }
                if let Some(parent) = plan.parents.get(key) {
                    n.parent = parent.clone();
                }
                if let Some(anchor) = plan.anchors.get(key) {
                    n.anchor = *anchor;
                }
                Some(n)
            };
            let parent = parent_of(&id)
                .map(|p| evaluate(&p, &nodes, &mut BTreeMap::new(), &mut BTreeSet::new()))
                .transpose()?;
            let anchor = parent.as_ref().map_or_else(
                || {
                    plan.anchors
                        .get(&id)
                        .copied()
                        .or_else(|| self.spatial_anchors.get(&id).copied())
                        .unwrap_or([0.; 3])
                },
                |p| p.anchor,
            );
            let old = &source[&id];
            let position = crate::spatial_contract::WorldPosition::new(
                old.anchor,
                [
                    old.world_matrix[12],
                    old.world_matrix[13],
                    old.world_matrix[14],
                ],
            )
            .and_then(|p| {
                p.relative_to(
                    crate::spatial_contract::WorldPosition::new(anchor, [0.; 3])?,
                    crate::spatial_contract::CoordinateDomain::new(1e12, 32768., 32768., 0.005)?,
                )
            })
            .map_err(|e| failure(&id, "three_d.reparent_coordinate_invalid", e.to_string()))?;
            let local = if plan.anchors.contains_key(&id) && !plan.parents.contains_key(&id) {
                // Changing a root's coordinate anchor changes only its local
                // translation. Preserve exact authored/final linear values:
                // decomposing a rounded rotation matrix can invent scale and
                // make a previously rigid camera fail its unit-scale contract.
                let mut local = match values.get(&(id.clone(), ComponentTypeId::transform3d())) {
                    Some(ComponentValue::Transform3D(t)) => t.clone(),
                    _ => self
                        .transform3d(&id)
                        .expect("validated spatial root")
                        .clone(),
                };
                local.translation = position;
                local
            } else {
                let mut target = old.world_matrix;
                target[12..15].copy_from_slice(&position);
                let local = multiply_matrix(
                    parent
                        .as_ref()
                        .map_or(Ok(IDENTITY_MATRIX), |p| inverse(&id, p.world_matrix))?,
                    target,
                );
                decompose(&id, local)?
            };
            values.insert(
                (id.clone(), ComponentTypeId::transform3d()),
                ComponentValue::Transform3D(local.clone()),
            );
            plan.writes.push((id, ComponentValue::Transform3D(local)));
        }
        Ok(plan)
    }
    pub(crate) fn validate_spatial_structure(
        &self,
        writes: &[(EntityId, ComponentValue)],
        camera: Option<&EntityId>,
        plan: &PreparedSpatialStructure,
    ) -> Result<(), WorldMutationError> {
        self.validate_spatial_overlay(writes, camera, &plan.parents, &plan.anchors, false)
    }
    pub(crate) fn commit_spatial_structure(&mut self, plan: PreparedSpatialStructure) {
        let mut reset = BTreeSet::new();
        for (id, parent) in plan.parents {
            if self.hierarchy(&id).and_then(|h| h.parent_id) != parent {
                self.spatial_dirty
                    .entry(id.clone())
                    .or_default()
                    .merge(SpatialFieldMask::HIERARCHY);
                self.set_parent(id.clone(), parent);
                reset.insert(id);
            }
        }
        for (id, anchor) in plan.anchors {
            if self.spatial_anchors.get(&id).copied().unwrap_or([0.; 3]) != anchor {
                self.spatial_anchors.insert(id.clone(), anchor);
                self.spatial_dirty
                    .entry(id.clone())
                    .or_default()
                    .merge(SpatialFieldMask::HIERARCHY);
                self.dirty_records.push(DirtyRecord {
                    entity_id: id.clone(),
                    dirty_type: DirtyType::Transform,
                });
                reset.insert(id);
            }
        }
        self.commit_deferred_components(plan.writes, None);
        if !reset.is_empty() {
            self.finish_spatial_tick();
        }
        for id in reset {
            self.reset_spatial_history(&id, None);
        }
    }
}
fn inverse(id: &EntityId, m: [f32; 16]) -> Result<[f32; 16], WorldMutationError> {
    let a = m.map(f64::from);
    let c = [
        a[5] * a[10] - a[9] * a[6],
        a[9] * a[2] - a[1] * a[10],
        a[1] * a[6] - a[5] * a[2],
        a[8] * a[6] - a[4] * a[10],
        a[0] * a[10] - a[8] * a[2],
        a[4] * a[2] - a[0] * a[6],
        a[4] * a[9] - a[8] * a[5],
        a[8] * a[1] - a[0] * a[9],
        a[0] * a[5] - a[4] * a[1],
    ];
    let d = a[0] * c[0] + a[4] * c[1] + a[8] * c[2];
    let mut out = IDENTITY_MATRIX;
    for column in 0..3 {
        for row in 0..3 {
            out[column * 4 + row] = (c[column * 3 + row] / d) as f32;
        }
    }
    for row in 0..3 {
        out[12 + row] = (-(f64::from(out[row]) * a[12]
            + f64::from(out[4 + row]) * a[13]
            + f64::from(out[8 + row]) * a[14])) as f32;
    }
    if !valid_affine(out) {
        return Err(failure(
            id,
            "three_d.reparent_singular",
            "Parent inverse is not representable",
        ));
    }
    Ok(out)
}
pub(super) fn decompose(
    id: &EntityId,
    m: [f32; 16],
) -> Result<CanonicalTransform3D, WorldMutationError> {
    let mut scale = std::array::from_fn::<_, 3, _>(|column| {
        (0..3)
            .map(|row| f64::from(m[column * 4 + row]).powi(2))
            .sum::<f64>()
            .sqrt()
    });
    let determinant = f64::from(m[0])
        * (f64::from(m[5]) * f64::from(m[10]) - f64::from(m[9]) * f64::from(m[6]))
        - f64::from(m[4])
            * (f64::from(m[1]) * f64::from(m[10]) - f64::from(m[9]) * f64::from(m[2]))
        + f64::from(m[8]) * (f64::from(m[1]) * f64::from(m[6]) - f64::from(m[5]) * f64::from(m[2]));
    if determinant < 0. {
        scale[0] = -scale[0];
    }
    let r = std::array::from_fn::<_, 9, _>(|i| f64::from(m[(i / 3) * 4 + i % 3]) / scale[i / 3]);
    for a in 0..3 {
        for b in a + 1..3 {
            if (0..3)
                .map(|i| r[a * 3 + i] * r[b * 3 + i])
                .sum::<f64>()
                .abs()
                > 1e-5
            {
                return Err(failure(
                    id,
                    "three_d.reparent_shear",
                    "Preserved world pose cannot be represented by local TRS",
                ));
            }
        }
    }
    let trace = r[0] + r[4] + r[8];
    let q;
    if trace > 0. {
        let s = (trace + 1.).sqrt() * 2.;
        q = [
            (r[5] - r[7]) / s,
            (r[6] - r[2]) / s,
            (r[1] - r[3]) / s,
            s / 4.,
        ];
    } else if r[0] > r[4] && r[0] > r[8] {
        let s = (1. + r[0] - r[4] - r[8]).sqrt() * 2.;
        q = [
            s / 4.,
            (r[3] + r[1]) / s,
            (r[6] + r[2]) / s,
            (r[5] - r[7]) / s,
        ];
    } else if r[4] > r[8] {
        let s = (1. + r[4] - r[0] - r[8]).sqrt() * 2.;
        q = [
            (r[3] + r[1]) / s,
            s / 4.,
            (r[7] + r[5]) / s,
            (r[6] - r[2]) / s,
        ];
    } else {
        let s = (1. + r[8] - r[0] - r[4]).sqrt() * 2.;
        q = [
            (r[6] + r[2]) / s,
            (r[7] + r[5]) / s,
            s / 4.,
            (r[1] - r[3]) / s,
        ];
    }
    let result = crate::spatial_contract::Transform3D {
        schema_version: "transform3d.v1".into(),
        translation: [m[12], m[13], m[14]],
        rotation: crate::spatial_contract::Rotation3D::Quaternion {
            xyzw: q.map(|v| v as f32),
        },
        scale: scale.map(|v| v as f32),
    }
    .canonicalize()
    .map_err(|e| failure(id, "three_d.reparent_transform_invalid", e.to_string()))?;
    let rebuilt = matrix(&result);
    if rebuilt
        .iter()
        .zip(m)
        .any(|(a, b)| (*a - b).abs() > 1e-4 * b.abs().max(1.))
    {
        return Err(failure(
            id,
            "three_d.reparent_shear",
            "TRS decomposition exceeds representation tolerance",
        ));
    }
    Ok(result)
}
