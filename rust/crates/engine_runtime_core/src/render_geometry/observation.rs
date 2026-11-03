//! Requested engine facts, derived from the same World/Prepare/View owners as rendering.
use super::*;
use crate::ids::EntityId;
use crate::project_observation::{
    ProjectObservationType as Type, ProjectObservationValue as Value,
};
use crate::render_state::RenderSceneState;
use crate::world::World;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialObservationRequest {
    pub schema_version: String,
    pub level: DiagnosticLevel,
    pub entities: Vec<String>,
}
impl SpatialObservationRequest {
    pub fn validate(&self) -> bool {
        let limit = match self.level {
            DiagnosticLevel::Off => 0,
            DiagnosticLevel::Summary => 64,
            DiagnosticLevel::Trace => 4096,
        };
        self.schema_version == "spatial-observation-request.v1"
            && self.entities.len() <= limit
            && self
                .entities
                .iter()
                .all(|id| !id.is_empty() && id.len() <= 128)
            && self
                .entities
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.entities.len()
    }
}

pub const SPATIAL_OBSERVATION_PREFIX: &str = "engine.spatial/";

/// A slash-delimited entity id avoids ambiguity with dotted project observation paths.
pub fn spatial_observation_path(path: &str) -> Option<(&str, &str, Type)> {
    let (id, field) = path
        .strip_prefix(SPATIAL_OBSERVATION_PREFIX)?
        .split_once('/')?;
    if id.is_empty() || id.len() > 128 || id.contains('/') {
        return None;
    }
    let ty = match field {
        "visible" | "inView" | "resourceReady" | "mainCamera" => Type::Bool,
        value
            if value.strip_prefix("material/").is_some_and(|slot| {
                !slot.is_empty() && slot.len() <= 128 && !slot.contains('/')
            }) =>
        {
            Type::String
        }
        "x" | "y" | "z" => Type::Number,
        _ => return None,
    };
    Some((id, field, ty))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpatialResourceState {
    Prepared,
    Deferred,
    Failed,
}
fn resource_failure(error: Option<ContractError>) -> SpatialResourceState {
    if error.is_some_and(|e| e.code == ContractErrorCode::ResourceNotReady) {
        SpatialResourceState::Deferred
    } else {
        SpatialResourceState::Failed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialEntityObservation {
    pub entity_id: String,
    pub runtime_index: u32,
    pub generation: u32,
    pub local: crate::spatial_contract::CanonicalTransform3D,
    pub authored_visible: bool,
    pub inherited_visible: bool,
    pub mesh: Option<crate::spatial_contract::SpatialAssetRef>,
    pub world_matrix: [f32; 16],
    pub world_version: u64,
    pub visible: bool,
    pub main_camera: bool,
    pub materials: BTreeMap<String, String>,
    pub bounds: Option<([f32; 3], [f32; 3])>,
    pub in_view: Option<bool>,
    pub resource_ready: Option<bool>,
    pub resource_state: Option<SpatialResourceState>,
    pub culling: Vec<ViewVisibility>,
    pub culling_summary: Vec<ViewCullingSummary>,
    pub resources: Vec<SpatialResourceObservation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialResourceObservation {
    pub slot: String,
    pub geometry: Provenance,
    pub material: Provenance,
    pub textures: BTreeMap<String, RenderAssetReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum SpatialFrameStage {
    Committed,
    Presented,
    Captured,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialObservation {
    pub schema_version: String,
    pub frame_stage: SpatialFrameStage,
    pub matches_projection: bool,
    pub committed_frame: u64,
    pub projection: Option<Projection>,
    pub pose: Option<CameraPose>,
    pub suspended: bool,
    pub view: Option<ExecutableRenderView>,
    pub entities: Vec<SpatialEntityObservation>,
    pub diagnostics: Vec<SpatialObservationDiagnostic>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialObservationDiagnostic {
    pub code: ContractErrorCode,
    pub field_path: String,
    pub stage: ProducerStage,
    pub provenance: Provenance,
    pub expected: String,
    pub actual: String,
    pub next_action: String,
    pub retryability: Retryability,
}

impl SpatialObservation {
    fn record(&mut self, error: ContractError, entity: Option<&str>) {
        if self.diagnostics.len() == 16 {
            self.truncated = true;
            return;
        }
        let mut owner = ContractDiagnostics::new(DiagnosticLevel::Summary);
        let mut provenance = Provenance::unspecified(error.stage);
        provenance.source_entity_id = entity.map(str::to_owned);
        owner.record(error, RenderDomain::World3d, &provenance, || {
            DiagnosticDetails {
                actual: error.to_string(),
                dependency_chain: Vec::new(),
            }
        });
        for item in owner.entries {
            self.diagnostics.push(SpatialObservationDiagnostic {
                code: item.code,
                field_path: item.field.into(),
                stage: item.stage,
                provenance: item.provenance,
                expected: item.expected.into(),
                actual: item.actual,
                next_action: item.next_action.into(),
                retryability: item.retryability,
            });
        }
    }
    pub fn actual_value(&self, path: &str) -> Option<Value> {
        if !self.matches_projection {
            return None;
        }
        let (id, field, _) = spatial_observation_path(path)?;
        let entity = self.entities.iter().find(|e| e.entity_id == id)?;
        Some(match field {
            "x" => Value::Number(f64::from(entity.world_matrix[12])),
            "y" => Value::Number(f64::from(entity.world_matrix[13])),
            "z" => Value::Number(f64::from(entity.world_matrix[14])),
            "visible" => Value::Bool(entity.visible),
            "mainCamera" => Value::Bool(entity.main_camera),
            "inView" => Value::Bool(entity.in_view?),
            "resourceReady" => Value::Bool(entity.resource_ready?),
            field if field.starts_with("material/") => Value::String(
                entity
                    .materials
                    .get(field.strip_prefix("material/")?)?
                    .clone(),
            ),
            _ => return None,
        })
    }
}

pub(crate) struct SpatialObservationContext<'a> {
    pub world: &'a World,
    pub scene: &'a RenderSceneState,
    pub target: Option<&'a crate::runtime_renderer::RenderTarget>,
}

impl ProductionGeometrySnapshot {
    pub(crate) fn observe(
        &self,
        frame: u64,
        context: SpatialObservationContext<'_>,
        manager: &RenderResourceManager,
        ids: &[String],
        level: DiagnosticLevel,
    ) -> Option<SpatialObservation> {
        // No detail allocation or closure is entered in normal runtime Off mode.
        if level == DiagnosticLevel::Off {
            return None;
        }
        let SpatialObservationContext {
            world,
            scene,
            target,
        } = context;
        let limit = if level == DiagnosticLevel::Summary {
            64
        } else {
            4096
        };
        let camera = scene
            .views()
            .filter(|v| v.view_kind == crate::render_state::RenderViewKind::Game)
            .find_map(|v| v.spatial.as_ref());
        let mut report = SpatialObservation {
            schema_version: "spatial-observation.v1".into(),
            frame_stage: SpatialFrameStage::Committed,
            matches_projection: true,
            projection: camera.map(|c| c.view.projection.clone()),
            pose: camera.map(|c| c.pose),
            suspended: camera.is_some_and(|c| c.suspended),
            committed_frame: frame,
            view: camera.and_then(|c| c.executable().ok().flatten()),
            entities: Vec::new(),
            diagnostics: Vec::new(),
            truncated: ids.len() > limit,
        };
        if world
            .dirty_records()
            .iter()
            .any(|r| world.transform3d(&r.entity_id).is_some())
        {
            report.matches_projection = false;
            report.record(
                ContractError::new(
                    ContractErrorCode::InvalidBinding,
                    "committedFrame.pendingWorldChanges",
                ),
                None,
            );
        }
        if camera.is_some_and(|c| {
            world.main_camera().map(|id| id.as_str()) != c.view.source_camera_entity_id.as_deref()
                || world
                    .main_camera()
                    .and_then(|id| world.spatial_world(id))
                    .is_none_or(|state| state.pose != c.pose)
        }) {
            report.matches_projection = false;
            report.record(
                ContractError::new(
                    ContractErrorCode::InvalidProjection,
                    "committedFrame.camera",
                ),
                None,
            );
        }
        if let Some(error) = self.error {
            report.record(error, None);
        }
        let output_camera = match (camera, target) {
            (Some(camera), Some(target)) if !camera.suspended => {
                self.observation_camera_for_target(camera, target)
            }
            (Some(camera), None) if !camera.suspended => Ok(Some((
                camera.clone(),
                [camera.view.viewport[2], camera.view.viewport[3]],
            ))),
            _ => Ok(None),
        };
        let output_camera = match output_camera {
            Ok(camera) => camera,
            Err(error) => {
                report.matches_projection = false;
                report.record(error, None);
                None
            }
        };
        if target.is_some() {
            report.view = output_camera.as_ref().and_then(|(camera, extent)| {
                camera.executable().ok().flatten().map(|mut view| {
                    view.target_extent = *extent;
                    view
                })
            });
        }
        let mut items = 0;
        for name in ids.iter().take(limit) {
            let id = EntityId::new(name);
            let (Some(state), Some(runtime)) =
                (world.spatial_world(&id), world.runtime_id_for_source(&id))
            else {
                report.record(
                    ContractError::new(ContractErrorCode::InvalidBinding, "entityId"),
                    Some(name),
                );
                continue;
            };
            let mut sample = SpatialEntityObservation {
                entity_id: name.clone(),
                runtime_index: runtime.index,
                generation: runtime.generation,
                local: world
                    .transform3d(&id)
                    .expect("evaluated spatial transform")
                    .clone(),
                authored_visible: state.authored_visible,
                inherited_visible: state.inherited_visible && state.enabled,
                mesh: world.mesh_renderer3d(&id).map(|m| m.mesh.clone()),
                culling: Vec::new(),
                culling_summary: Vec::new(),
                resources: Vec::new(),
                world_matrix: state.world_matrix,
                world_version: state.version,
                visible: state.enabled && state.authored_visible && state.inherited_visible,
                main_camera: world.main_camera() == Some(&id),
                materials: BTreeMap::new(),
                bounds: None,
                in_view: None,
                resource_ready: None,
                resource_state: None,
            };
            if let Some((projection, prepared)) = self
                .meshes
                .iter()
                .find(|(p, _)| p.runtime_entity_id == runtime)
            {
                if projection.world != *state
                    || world.mesh_renderer3d(&id) != Some(&projection.component)
                {
                    report.matches_projection = false;
                    report.record(
                        ContractError::new(
                            ContractErrorCode::InvalidBinding,
                            "committedFrame.entity",
                        ),
                        Some(name),
                    );
                }
                if items + prepared.geometry.len() > 4096 {
                    report.truncated = true;
                    break;
                }
                items += prepared.geometry.len();
                sample.materials = projection
                    .component
                    .materials
                    .iter()
                    .map(|(slot, asset)| (slot.clone(), asset.0.id.clone()))
                    .collect();
                let mut min = [f32::INFINITY; 3];
                let mut max = [f32::NEG_INFINITY; 3];
                for geometry in &prepared.geometry {
                    sample.resources.push(SpatialResourceObservation {
                        slot: geometry.submesh.material_slot.clone(),
                        geometry: geometry.geometry_source.as_ref().clone(),
                        material: geometry.material_source.as_ref().clone(),
                        textures: geometry.material.textures.clone(),
                    });
                    for bits in 0..8 {
                        let p: [f32; 3] = std::array::from_fn(|i| {
                            if bits & (1 << i) == 0 {
                                geometry.bounds.min[i]
                            } else {
                                geometry.bounds.max[i]
                            }
                        });
                        let m = state.world_matrix;
                        for i in 0..3 {
                            let v = m[i] * p[0] + m[4 + i] * p[1] + m[8 + i] * p[2] + m[12 + i];
                            min[i] = min[i].min(v);
                            max[i] = max[i].max(v);
                        }
                    }
                }
                if !prepared.geometry.is_empty() && min.iter().chain(&max).all(|v| v.is_finite()) {
                    sample.bounds = Some((min, max));
                }
                let residency = prepared.geometry.iter().try_for_each(|g| {
                    g.geometry_binding.validate_residency(manager)?;
                    g.material_binding.validate_residency(manager)
                });
                sample.resource_ready = Some(residency.is_ok());
                sample.resource_state = Some(match residency {
                    Ok(()) => SpatialResourceState::Prepared,
                    Err(error) => {
                        report.record(error, Some(name));
                        resource_failure(Some(error))
                    }
                });
                if let Some((camera, extent)) = &output_camera {
                    let result = prepared
                        .geometry
                        .iter()
                        .map(|g| {
                            let mut instance = projection.geometry_instance(g)?;
                            if target.is_some() {
                                let matrix = &mut instance.world_transform;
                                let relative = super::production::relative_position(
                                    [matrix[12], matrix[13], matrix[14]],
                                    projection.coordinate_origin,
                                    camera.origin,
                                )?;
                                matrix[12..15].copy_from_slice(&relative);
                            }
                            Ok(instance)
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .and_then(|instances| {
                            GeometryFrame::build(
                                "spatial-observation",
                                *extent,
                                &[GeometryViewInput {
                                    view: &camera.view,
                                    pose: camera.pose,
                                }],
                                &instances,
                                manager,
                                if level == DiagnosticLevel::Trace {
                                    DiagnosticLevel::Trace
                                } else {
                                    DiagnosticLevel::Summary
                                },
                            )
                            .map_err(|e| e.error)
                        });
                    match result {
                        Ok(frame) => {
                            sample.in_view = Some(frame.views.iter().any(|v| !v.draws.is_empty()));
                            sample.culling_summary = frame
                                .views
                                .iter()
                                .filter_map(|v| v.summary.clone())
                                .collect();
                            sample.culling =
                                frame.views.into_iter().flat_map(|v| v.trace).collect();
                        }
                        Err(error) => {
                            report.record(error, Some(name));
                        }
                    }
                }
            } else if world.mesh_renderer3d(&id).is_some() {
                sample.resource_ready = Some(false);
                sample.resource_state = Some(resource_failure(self.error));
            }
            report.entities.push(sample);
        }
        Some(report)
    }
}
