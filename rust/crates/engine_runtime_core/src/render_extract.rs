//! Historical module name for the World -> Render projection path.
//!
//! Architecture docs now describe this as `RenderProjection` /
//! `RenderProjectionAdapter<SpriteRenderer2D>`. The file name is kept for
//! compatibility with existing call sites, not as a separate bridge layer.

use crate::ids::SourceEntityId;
use crate::render_command::{
    RenderCommand, RenderCommandId, RenderCommandPayload, RenderCommandQueue, RenderCommandType,
    ThreadLocalCommandBuffer,
};
use crate::render_projection_adapter::{RenderProjectionAdapter, SpriteRenderer2DAdapter};
use crate::render_state::{RenderProxyDescriptor, RenderSceneState};
use crate::world::{DirtyRecord, DirtyType, World};

#[cfg(test)]
std::thread_local! { static SPATIAL_SEEN_HASHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
#[derive(Default)]
struct SeenHashBuilder(std::collections::hash_map::RandomState);
#[cfg(test)]
struct SeenHasher(std::collections::hash_map::DefaultHasher);
#[cfg(test)]
impl std::hash::BuildHasher for SeenHashBuilder {
    type Hasher = SeenHasher;
    fn build_hasher(&self) -> Self::Hasher {
        SeenHasher(std::hash::BuildHasher::build_hasher(&self.0))
    }
}
#[cfg(test)]
impl std::hash::Hasher for SeenHasher {
    fn finish(&self) -> u64 {
        SPATIAL_SEEN_HASHES.with(|n| n.set(n.get() + 1));
        std::hash::Hasher::finish(&self.0)
    }
    fn write(&mut self, bytes: &[u8]) {
        std::hash::Hasher::write(&mut self.0, bytes);
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderExtractContext {
    next_command_id: u64,
    main_spatial_view: Option<crate::render_state::RenderViewId>,
    spatial_view_version: u64,
}

impl RenderExtractContext {
    pub fn new() -> Self {
        Self {
            next_command_id: 1,
            main_spatial_view: None,
            spatial_view_version: 0,
        }
    }

    pub fn extract_world_dirty(
        &mut self,
        frame_index: u64,
        world: &mut World,
        scene: &RenderSceneState,
    ) -> RenderCommandQueue {
        let mut dirty_records = world.take_dirty_records();
        let queue = self.extract_records(frame_index, world, scene, &mut dirty_records);
        world.recycle_dirty_record_storage(dirty_records);
        queue
    }

    pub fn extract_dirty_records(
        &mut self,
        frame_index: u64,
        world: &World,
        scene: &RenderSceneState,
        dirty_records: Vec<DirtyRecord>,
    ) -> RenderCommandQueue {
        let mut dirty_records = dirty_records;
        self.extract_records(frame_index, world, scene, &mut dirty_records)
    }

    fn extract_records(
        &mut self,
        frame_index: u64,
        world: &World,
        scene: &RenderSceneState,
        dirty_records: &mut Vec<DirtyRecord>,
    ) -> RenderCommandQueue {
        // Existing proxies still require removal, and legacy/dynamic records
        // retain their original behavior. Re-prove current archetypes each call
        // so a newly drawable row can never reuse an earlier empty result.
        if scene.proxies_len() == 0
            && dirty_records.iter().all(|record| {
                matches!(
                    record.dirty_type,
                    DirtyType::Transform | DirtyType::RenderState
                )
            })
            && world.only_spatial_rows_without_render_payload()
        {
            dirty_records.clear();
            return RenderCommandQueue::new(frame_index);
        }
        let mut buffer = ThreadLocalCommandBuffer::new();
        // Borrow only this input Vec. No identity or current-state proof survives
        // extraction, and outbound commands acquire their own IDs before clear.
        #[cfg(not(test))]
        let mut spatial_seen = std::collections::HashMap::with_capacity(dirty_records.len());
        #[cfg(test)]
        let mut spatial_seen = std::collections::HashMap::with_capacity_and_hasher(
            dirty_records.len(),
            SeenHashBuilder::default(),
        );
        for dirty_record in dirty_records.iter() {
            let candidate = matches!(
                dirty_record.dirty_type,
                DirtyType::Transform | DirtyType::RenderState
            );
            let entry = if candidate {
                match spatial_seen.entry(&dirty_record.entity_id) {
                    std::collections::hash_map::Entry::Occupied(_) => continue,
                    std::collections::hash_map::Entry::Vacant(entry) => Some(entry),
                }
            } else {
                None
            };
            if candidate
                && scene.proxy_for_source(&dirty_record.entity_id).is_none()
                && world.spatial_without_render_payload(&dirty_record.entity_id)
            {
                entry.expect("candidate entry").insert(());
                continue;
            }
            if candidate && world.transform3d(&dirty_record.entity_id).is_some() {
                entry.expect("candidate entry").insert(());
            }
            if let Some(command) =
                self.command_for_dirty(frame_index, world, scene, dirty_record.clone())
            {
                buffer.push(command);
            }
        }
        drop(spatial_seen);
        dirty_records.clear();
        let mut queue = RenderCommandQueue::new(frame_index);
        queue.collect(vec![buffer]);
        queue
    }

    pub fn project_main_camera(
        &mut self,
        world: &World,
        scene: &mut RenderSceneState,
        extent: [u32; 2],
    ) -> Result<Option<crate::render_state::RenderViewId>, crate::render_contract::ContractError>
    {
        use crate::render_contract::*;
        use crate::render_projection_adapter::Camera3DAdapter;
        use crate::render_state::{
            RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState, Viewport,
        };
        let Some(id) = world.main_camera() else {
            if let Some(old) = self.main_spatial_view.take() {
                scene.remove_view(old);
            }
            if scene.proxies().any(|p| {
                matches!(
                    p.payload,
                    crate::render_state::RenderProxyPayload::Mesh3D(_)
                )
            }) {
                return Err(ContractError::new(
                    ContractErrorCode::InvalidProjection,
                    "mainCamera.missing",
                ));
            }
            return Ok(None);
        };
        let fail = || ContractError::new(ContractErrorCode::InvalidProjection, "mainCamera");
        let valid = world.camera3d(id).filter(|c| c.enabled).zip(
            world
                .spatial_presentation(id)
                .filter(|s| s.enabled && s.rigid),
        );
        let Some((camera, state)) = valid else {
            if let Some(old) = self.main_spatial_view.take() {
                scene.remove_view(old);
            }
            return Err(fail());
        };
        let mut projected = Camera3DAdapter {
            runtime_entity_id: world.runtime_id_for_source(id).ok_or_else(fail)?,
            world: state,
            extent,
        }
        .project(id, camera);
        projected.coordinate_origin = world.spatial_presentation_origin(id).ok_or_else(fail)?;
        projected.lights = crate::render_projection_adapter::project_lights(world, &projected)?;
        let view_id = RenderViewId(projected.view.view_id);
        if let Some(old) = self.main_spatial_view.filter(|old| *old != view_id) {
            scene.remove_view(old);
        }
        if let Some(existing) = scene.view(view_id) {
            if existing
                .spatial
                .as_ref()
                .is_none_or(|v| v.view.source_camera_entity_id.as_deref() != Some(id.as_str()))
            {
                return Err(ContractError::new(
                    ContractErrorCode::InvalidProjection,
                    "viewId.collision",
                ));
            }
        }
        if let Some(previous) = scene.view(view_id).and_then(|v| v.spatial.as_ref()) {
            projected.view.version = previous.view.version;
            if projected != *previous {
                self.spatial_view_version =
                    self.spatial_view_version.max(previous.view.version) + 1;
                projected.view.version = self.spatial_view_version;
            }
        } else {
            self.spatial_view_version += 1;
            projected.view.version = self.spatial_view_version;
        }
        let evaluated = projected.executable()?;
        let mut view =
            RenderViewState::new(view_id, RenderViewKind::Game, RenderTargetKind::Window);
        view.version = projected.view.version;
        view.source_entity_id = Some(id.clone());
        view.viewport = Viewport {
            x: 0,
            y: 0,
            width: extent[0],
            height: extent[1],
        };
        view.clear_color = camera.clear_color;
        if let Some(e) = evaluated {
            view.view_matrix = e.view_matrix.map(|v| v.to_f32());
            view.projection_matrix = e.projection_matrix.map(|v| v.to_f32());
        }
        view.spatial = Some(projected);
        scene.register_view(view);
        self.main_spatial_view = Some(view_id);
        Ok(Some(view_id))
    }

    /// Existing view registry consumer seam for an auxiliary camera/offscreen view.
    /// Project gameplay continues to use typed intents, never RenderScene internals.
    pub fn project_camera_view(
        &mut self,
        world: &World,
        scene: &mut RenderSceneState,
        id: &SourceEntityId,
        viewport: crate::render_state::Viewport,
        kind: crate::render_state::RenderViewKind,
        target: crate::render_state::RenderTargetKind,
    ) -> Result<crate::render_state::RenderViewId, crate::render_contract::ContractError> {
        use crate::render_contract::*;
        use crate::render_projection_adapter::Camera3DAdapter;
        use crate::render_state::{RenderViewId, RenderViewState};
        let fail = || ContractError::new(ContractErrorCode::InvalidProjection, "auxiliaryCamera");
        let camera = world.camera3d(id).filter(|c| c.enabled).ok_or_else(fail)?;
        let state = world
            .spatial_presentation(id)
            .filter(|s| s.enabled && s.rigid)
            .ok_or_else(fail)?;
        let mut projected = Camera3DAdapter {
            runtime_entity_id: world.runtime_id_for_source(id).ok_or_else(fail)?,
            world: state,
            extent: [viewport.width, viewport.height],
        }
        .project(id, camera);
        projected.coordinate_origin = world.spatial_presentation_origin(id).ok_or_else(fail)?;
        projected.lights = crate::render_projection_adapter::project_lights(world, &projected)?;
        projected.view.viewport = [viewport.x, viewport.y, viewport.width, viewport.height];
        projected.view.view_kind = (&kind).into();
        projected.view.target = (&target).into();
        let view_id = RenderViewId(projected.view.view_id);
        if let Some(old) = scene.view(view_id) {
            if old.source_entity_id.as_ref() != Some(id) {
                return Err(ContractError::new(
                    ContractErrorCode::InvalidProjection,
                    "viewId.collision",
                ));
            }
            projected.view.version = old.version;
            if old.spatial.as_ref() != Some(&projected) {
                projected.view.version += 1;
            }
        } else {
            projected.view.version = 1;
        }
        let executable = projected.executable()?;
        let mut view = RenderViewState::new(view_id, kind, target);
        view.source_entity_id = Some(id.clone());
        view.version = projected.view.version;
        view.viewport = viewport;
        view.clear_color = camera.clear_color;
        if let Some(e) = executable {
            view.view_matrix = e.view_matrix.map(|v| v.to_f32());
            view.projection_matrix = e.projection_matrix.map(|v| v.to_f32());
        }
        view.spatial = Some(projected);
        scene.register_view(view);
        Ok(view_id)
    }

    fn command_for_dirty(
        &mut self,
        frame_index: u64,
        world: &World,
        scene: &RenderSceneState,
        dirty_record: DirtyRecord,
    ) -> Option<RenderCommand> {
        let proxy_id = scene.proxy_for_source(&dirty_record.entity_id);
        let runtime_entity_id = world
            .runtime_id_for_source(&dirty_record.entity_id)
            .or_else(|| {
                // Despawn releases the World mapping before RenderProjection consumes its dirty record.
                proxy_id
                    .and_then(|id| scene.proxy(id))
                    .map(|proxy| proxy.common.runtime_entity_id)
            })?;
        if world.transform3d(&dirty_record.entity_id).is_some() {
            if !matches!(
                dirty_record.dirty_type,
                DirtyType::Transform | DirtyType::RenderState
            ) {
                return None;
            }
            return self.render_state_command(
                frame_index,
                world,
                dirty_record.entity_id,
                runtime_entity_id,
                proxy_id,
                scene,
            );
        }
        let command_id = self.next_command_id();
        match dirty_record.dirty_type {
            DirtyType::Transform => {
                let transform = world.transform(&dirty_record.entity_id)?.clone();
                let Some(proxy_id) = proxy_id else {
                    let descriptor = render_proxy_descriptor(world, &dirty_record.entity_id)?;
                    return Some(RenderCommand::new(
                        command_id,
                        frame_index,
                        dirty_record.entity_id,
                        runtime_entity_id,
                        None,
                        RenderCommandType::AddProxy,
                        RenderCommandPayload::AddProxy {
                            transform,
                            descriptor,
                        },
                        DirtyType::Transform,
                    ));
                };
                Some(RenderCommand::new(
                    command_id,
                    frame_index,
                    dirty_record.entity_id,
                    runtime_entity_id,
                    Some(proxy_id),
                    RenderCommandType::UpdateTransform,
                    RenderCommandPayload::UpdateTransform { transform },
                    DirtyType::Transform,
                ))
            }
            DirtyType::RenderState => self.render_state_command(
                frame_index,
                world,
                dirty_record.entity_id,
                runtime_entity_id,
                proxy_id,
                scene,
            ),
            DirtyType::DynamicData => Some(RenderCommand::new(
                command_id,
                frame_index,
                dirty_record.entity_id,
                runtime_entity_id,
                proxy_id,
                RenderCommandType::UpdateDynamicData,
                RenderCommandPayload::UpdateDynamicData,
                DirtyType::DynamicData,
            )),
            DirtyType::InstanceData => Some(RenderCommand::new(
                command_id,
                frame_index,
                dirty_record.entity_id,
                runtime_entity_id,
                proxy_id,
                RenderCommandType::UpdateInstanceData,
                RenderCommandPayload::UpdateInstanceData,
                DirtyType::InstanceData,
            )),
            DirtyType::Physics2D => None,
        }
    }

    fn render_state_command(
        &mut self,
        frame_index: u64,
        world: &World,
        source_entity_id: SourceEntityId,
        runtime_entity_id: crate::ids::RuntimeEntityId,
        proxy_id: Option<crate::render_state::RenderProxyId>,
        scene: &RenderSceneState,
    ) -> Option<RenderCommand> {
        let command_id = self.next_command_id();
        let descriptor = render_proxy_descriptor(world, &source_entity_id);
        match (proxy_id, descriptor) {
            (None, Some(descriptor)) => {
                let transform = match world.transform(&source_entity_id) {
                    Some(t) => t.clone(),
                    None if world.mesh_renderer3d(&source_entity_id).is_some() => {
                        crate::components::Transform::identity()
                    }
                    None => return None,
                };
                Some(RenderCommand::new(
                    command_id,
                    frame_index,
                    source_entity_id,
                    runtime_entity_id,
                    None,
                    RenderCommandType::AddProxy,
                    RenderCommandPayload::AddProxy {
                        transform,
                        descriptor,
                    },
                    DirtyType::RenderState,
                ))
            }
            (Some(proxy_id), Some(descriptor)) => Some(RenderCommand::new(
                command_id,
                frame_index,
                source_entity_id,
                runtime_entity_id,
                Some(proxy_id),
                RenderCommandType::UpdateRenderState,
                RenderCommandPayload::UpdateRenderState {
                    visible: descriptor.visible,
                    layer: descriptor.layer.clone(),
                    payload_kind: descriptor.payload_kind(),
                    descriptor: Some(descriptor),
                },
                DirtyType::RenderState,
            )),
            (Some(proxy_id), None) => Some(RenderCommand::new(
                command_id,
                frame_index,
                source_entity_id,
                // A source ID may already belong to a new World incarnation.
                // Normalize removal against the old proxy's actual owner.
                scene.proxy(proxy_id)?.common.runtime_entity_id,
                Some(proxy_id),
                RenderCommandType::RemoveProxy,
                RenderCommandPayload::RemoveProxy,
                DirtyType::RenderState,
            )),
            (None, None) => None,
        }
    }

    fn next_command_id(&mut self) -> RenderCommandId {
        let command_id = RenderCommandId(self.next_command_id);
        self.next_command_id += 1;
        command_id
    }
}

fn render_proxy_descriptor(
    world: &World,
    source_entity_id: &SourceEntityId,
) -> Option<RenderProxyDescriptor> {
    if let Some(mesh) = world.mesh_renderer3d(source_entity_id) {
        let mut projection = crate::render_projection_adapter::MeshRenderer3DAdapter {
            runtime_entity_id: world.runtime_id_for_source(source_entity_id)?,
            world: world.spatial_presentation(source_entity_id)?,
        }
        .project(source_entity_id, mesh);
        projection.coordinate_origin = world.spatial_presentation_origin(source_entity_id)?;
        projection.deformation = world.character_skin(source_entity_id);
        let read = |ty: &str| match world.component_value(
            source_entity_id,
            &crate::components::ComponentTypeId::new(ty),
        ) {
            Some(crate::archetype::ComponentValue::Dynamic { value, .. }) => {
                Some(crate::character_animation::dynamic_json(&value))
            }
            _ => None,
        };
        let lod = read("engine.model_lod3d").and_then(|v| serde_json::from_value(v).ok());
        let hlod = read("engine.hlod3d").and_then(|v| serde_json::from_value(v).ok());
        let vegetation = read("engine.vegetation3d").and_then(|v| serde_json::from_value(v).ok());
        if lod.is_some() || hlod.is_some() || vegetation.is_some() {
            projection.presentation = Some(std::sync::Arc::new(
                crate::model_lod3d::MeshPresentation3D {
                    lod,
                    hlod,
                    vegetation,
                    time_seconds: world.presentation_time_seconds,
                    ..Default::default()
                },
            ));
        }

        return Some(RenderProxyDescriptor {
            visible: projection.world.enabled
                && projection.world.authored_visible
                && projection.world.inherited_visible,
            layer: String::new(),
            payload: crate::render_state::RenderProxyPayload::Mesh3D(Box::new(projection)),
        });
    }
    if let Some(sprite) = world.sprite_renderer2d(source_entity_id) {
        // The adapter constructs only the lossless legacy subset. Asset validity
        // remains owned by the existing pipeline, including missing/empty keys.
        let projection = SpriteRenderer2DAdapter.project(source_entity_id, sprite);
        return Some(
            projection
                .to_legacy()
                .expect("SpriteRenderer2D adapter must produce a legacy-compatible projection")
                .payload,
        );
    }
    world
        .renderable(source_entity_id)
        .cloned()
        .map(RenderProxyDescriptor::from_renderable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Hierarchy, Renderable, SpriteRenderer2D, Transform};
    use crate::ids::EntityId;
    use crate::math::Vec3;
    use crate::render_command::apply_batch;
    use crate::render_state::{
        RenderPayloadKind, RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
    };

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    fn transform(x: f32) -> Transform {
        Transform {
            local_position: Vec3 { x, y: 0.0, z: 0.0 },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn renderable(mesh: &str) -> Renderable {
        Renderable {
            mesh_ref: Some(mesh.to_string()),
            material_ref: None,
            visible: true,
            layer: "default".to_string(),
        }
    }

    fn sprite(sprite_ref: &str) -> SpriteRenderer2D {
        SpriteRenderer2D {
            sprite_ref: Some(sprite_ref.to_string()),
            sorting_layer: 2,
            order_in_layer: 7,
            sort_z: 0.5,
            ..SpriteRenderer2D::default()
        }
    }

    fn world_with_renderable() -> World {
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-a"),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(transform(1.0)),
            Some(renderable("mesh-a")),
        );
        world
    }

    #[test]
    fn spatial_empty_payload_still_removes_old_proxy_and_can_become_drawable_again() {
        let crate::spatial_contract::RuntimeSceneDocument::Spatial(scene_doc) =
            crate::spatial_contract::decode_runtime_scene(include_bytes!(
                "../tests/fixtures/three_d_a0/runtime-scene-v2.json"
            ))
            .unwrap()
        else {
            panic!("spatial fixture")
        };
        let mut world = crate::scene_loader::load_scene_into_world(&scene_doc.into())
            .value
            .unwrap();
        let mut scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let id = EntityId::from("cube");
        let mesh = world.mesh_renderer3d(&id).unwrap().clone();
        let commands = extract
            .extract_world_dirty(1, &mut world, &scene)
            .normalize_merge(&scene);
        assert!(apply_batch(&mut scene, &commands).is_empty());
        assert!(scene.proxy_for_source(&id).is_some());
        world
            .try_remove_component_value(&id, &crate::components::ComponentTypeId::mesh_renderer3d())
            .unwrap();
        let commands = extract
            .extract_world_dirty(2, &mut world, &scene)
            .normalize_merge(&scene);
        assert!(commands
            .iter()
            .any(|c| c.command_type == RenderCommandType::RemoveProxy));
        assert!(apply_batch(&mut scene, &commands).is_empty());
        assert!(scene.proxy_for_source(&id).is_none());
        world
            .try_insert_component_value(
                id.clone(),
                crate::components::ComponentTypeId::mesh_renderer3d(),
                crate::archetype::ComponentValue::MeshRenderer3D(mesh),
            )
            .unwrap();
        let commands = extract
            .extract_world_dirty(3, &mut world, &scene)
            .normalize_merge(&scene);
        assert!(apply_batch(&mut scene, &commands).is_empty());
        assert!(scene.proxy_for_source(&id).is_some());
        let old_runtime = world.runtime_id_for_source(&id).unwrap();
        let spatial = world.transform3d(&id).unwrap().clone();
        world.try_despawn_entity(&id).unwrap();
        world.spawn_with_components(
            id.clone(),
            "new cube",
            "actor",
            true,
            hierarchy(),
            None,
            None,
        );
        world.insert_component_value(
            id.clone(),
            crate::archetype::ComponentValue::Transform3D(spatial),
        );
        assert_ne!(world.runtime_id_for_source(&id), Some(old_runtime));
        let commands = extract
            .extract_world_dirty(4, &mut world, &scene)
            .normalize_merge(&scene);
        assert!(apply_batch(&mut scene, &commands).is_empty());
        assert!(scene.proxy_for_source(&id).is_none());
    }

    #[test]
    fn non_drawable_spatial_dirty_uses_one_row_lookup_per_entity_at_scale() {
        use crate::spatial_contract::{
            decode_runtime_scene, RuntimeEntityDocument, RuntimeSceneDocument, RuntimeSpatial,
        };
        let mut observed = Vec::new();
        for size in [1, 100, 1000, 10000] {
            let RuntimeSceneDocument::Spatial(mut scene) = decode_runtime_scene(include_bytes!(
                "../tests/fixtures/three_d_a0/runtime-scene-v2.json"
            ))
            .unwrap() else {
                panic!("spatial fixture")
            };
            let template = scene.entities[0].clone();
            scene.entities = (0..size)
                .map(|i| {
                    let mut entity = template.clone();
                    if let RuntimeEntityDocument::Spatial(ref mut entity) = entity {
                        entity.id = format!("empty-{i}");
                        let RuntimeSpatial::World3D { mesh_renderer, .. } = &mut entity.spatial;
                        *mesh_renderer = None;
                    }
                    entity
                })
                .collect();
            scene.main_camera = None;
            let mut world = crate::scene_loader::load_scene_into_world(&scene.into())
                .value
                .unwrap();
            world.take_dirty_records();
            let records = (0..size)
                .flat_map(|i| {
                    [DirtyType::Transform, DirtyType::RenderState].map(|dirty_type| DirtyRecord {
                        entity_id: format!("empty-{i}").into(),
                        dirty_type,
                    })
                })
                .collect::<Vec<_>>();
            let count = records.len();
            crate::world::mutation_work::SOURCE_LOOKUPS.with(|n| n.set(0));
            SPATIAL_SEEN_HASHES.with(|n| n.set(0));
            let mut queue = RenderExtractContext::new().extract_dirty_records(
                1,
                &world,
                &RenderSceneState::new(),
                records,
            );
            let hashes = SPATIAL_SEEN_HASHES.with(|n| n.get());
            assert!(
                hashes == 0,
                "empty spatial projection must not hash individual IDs at {size}: {hashes}"
            );
            let lookups = crate::world::mutation_work::SOURCE_LOOKUPS.with(|n| n.get());
            assert!(queue.normalize_merge(&RenderSceneState::new()).is_empty());
            observed.push((size, count, lookups));
        }
        eprintln!("non-drawable projection row lookups: {observed:?}");
        assert!(
            observed
                .iter()
                .all(|(entities, records, lookups)| *records == 2 * entities && *lookups == 0),
            "{observed:?}"
        );
    }

    #[test]
    fn borrowed_spatial_seen_keeps_legacy_duplicates_and_outbound_identity_ownership() {
        let mut world = world_with_renderable();
        world.take_dirty_records();
        let mut extract = RenderExtractContext::new();
        let mut scene = RenderSceneState::new();
        let id = EntityId::from("entity-a");
        let mut input = [
            DirtyType::Transform,
            DirtyType::RenderState,
            DirtyType::Transform,
        ]
        .into_iter()
        .map(|dirty_type| DirtyRecord {
            entity_id: id.clone(),
            dirty_type,
        })
        .collect::<Vec<_>>();
        let capacity = input.capacity();
        let mut queue = extract.extract_records(1, &world, &scene, &mut input);
        assert_eq!(
            queue.pending_commands.len(),
            3,
            "legacy 2D notifications must not enter spatial dedup"
        );
        assert!(input.is_empty());
        assert_eq!(input.capacity(), capacity);
        let merged = queue.normalize_merge(&scene);
        assert!(apply_batch(&mut scene, &merged).is_empty());
        world.despawn_entity(&id).unwrap();
        let records = vec![
            DirtyRecord {
                entity_id: id.clone(),
                dirty_type: DirtyType::RenderState
            };
            2
        ];
        let mut removals = extract.extract_dirty_records(2, &world, &scene, records);
        assert_eq!(
            removals.pending_commands.len(),
            2,
            "a stale ID with an old proxy is not a spatial identity proof"
        );
        assert!(removals
            .pending_commands
            .iter()
            .all(|command| command.command_type == RenderCommandType::RemoveProxy));
        drop((world, input));
        assert_eq!(
            removals.pending_commands[0].source_entity_id.as_str(),
            "entity-a"
        );
        let merged = removals.normalize_merge(&scene);
        assert!(apply_batch(&mut scene, &merged).is_empty());
        assert!(scene.proxy_for_source(&id).is_none());
    }

    #[test]
    fn projection_reuses_consumed_backing_without_recycling_public_readers() {
        let mut world = world_with_renderable();
        world.insert_transform("entity-a".into(), transform(1.5));
        let mut extract = RenderExtractContext::new();
        let scene = RenderSceneState::new();
        let capacity = world.dirty_record_capacity();
        assert!(capacity > 0);
        extract.extract_world_dirty(1, &mut world, &scene);
        assert!(world.dirty_records().is_empty());
        assert!(world.dirty_record_capacity() >= capacity);
        world.insert_transform("entity-a".into(), transform(2.));
        let retained = world.take_dirty_records();
        let before = retained.clone();
        world.insert_transform("entity-a".into(), transform(3.));
        extract.extract_world_dirty(2, &mut world, &scene);
        assert_eq!(retained, before);
        assert!(world.dirty_records().is_empty());
        assert!(world.dirty_record_capacity() > 0);
    }

    #[test]
    fn despawn_sprite_removes_presented_proxy_after_world_identity_is_released() {
        let mut world = World::new();
        let id = EntityId::from("entity-sprite");
        world.spawn_with_components(
            id.clone(),
            "Sprite",
            "actor",
            true,
            hierarchy(),
            Some(transform(1.0)),
            None,
        );
        world.insert_sprite_renderer2d(id.clone(), sprite("sprite-a"));
        let mut scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let commands = extract
            .extract_world_dirty(1, &mut world, &scene)
            .normalize_merge(&scene);
        assert!(apply_batch(&mut scene, &commands).is_empty());
        assert!(scene.proxy_for_source(&id).is_some());
        world.try_despawn_entity(&id).unwrap();
        assert!(world.runtime_id_for_source(&id).is_none());
        let commands = extract
            .extract_world_dirty(2, &mut world, &scene)
            .normalize_merge(&scene);
        assert!(apply_batch(&mut scene, &commands).is_empty());
        assert!(
            scene.proxy_for_source(&id).is_none(),
            "despawned sprite must disappear from the presented scene"
        );
    }

    #[test]
    fn spawn_renderable_entity_extracts_add_proxy() {
        let mut world = world_with_renderable();
        world.insert_transform(EntityId::from("entity-a"), transform(2.0));
        world.insert_renderable(EntityId::from("entity-a"), renderable("mesh-a"));
        let scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let mut queue = extract.extract_world_dirty(1, &mut world, &scene);
        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command_type, RenderCommandType::AddProxy);
        assert!(world.dirty_records().is_empty());
    }

    #[test]
    fn sprite_renderer2d_extracts_sprite_add_proxy() {
        let mut world = World::new();
        let id = EntityId::from("entity-sprite");
        world.spawn_with_components(
            id.clone(),
            "Sprite",
            "actor",
            true,
            hierarchy(),
            Some(transform(1.0)),
            None,
        );
        world.insert_sprite_renderer2d(id.clone(), sprite("sprite-a"));
        let scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let mut queue = extract.extract_world_dirty(1, &mut world, &scene);
        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command_type, RenderCommandType::AddProxy);
        let mut scene = RenderSceneState::new();
        let diagnostics = apply_batch(&mut scene, &merged);
        assert!(diagnostics.is_empty());
        let proxy_id = scene.proxy_for_source(&id).unwrap();
        let proxy = scene.proxy(proxy_id).unwrap();
        assert_eq!(proxy.payload.kind(), RenderPayloadKind::Sprite);
        match &proxy.payload {
            crate::render_state::RenderProxyPayload::Sprite(payload) => {
                assert_eq!(payload.sprite_ref.as_deref(), Some("sprite-a"));
                assert_eq!(payload.sorting_layer, 2);
                assert_eq!(payload.order_in_layer, 7);
            }
            other => panic!("expected sprite payload, got {other:?}"),
        }
    }

    #[test]
    fn transform_write_extracts_update_transform() {
        let mut world = world_with_renderable();
        let mut scene = RenderSceneState::new();
        let runtime_id = world
            .runtime_id_for_source(&EntityId::from("entity-a"))
            .unwrap();
        let proxy = crate::render_state::RenderProxy::new(
            crate::render_state::RenderProxyId(0),
            runtime_id,
            EntityId::from("entity-a"),
            transform(1.0),
            renderable("mesh-a"),
        );
        scene.insert_proxy(proxy);
        world.insert_transform(EntityId::from("entity-a"), transform(4.0));
        let mut extract = RenderExtractContext::new();

        let mut queue = extract.extract_world_dirty(1, &mut world, &scene);
        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command_type, RenderCommandType::UpdateTransform);
    }

    #[test]
    fn remove_renderable_extracts_remove_proxy() {
        let mut world = world_with_renderable();
        let mut scene = RenderSceneState::new();
        let runtime_id = world
            .runtime_id_for_source(&EntityId::from("entity-a"))
            .unwrap();
        scene.insert_proxy(crate::render_state::RenderProxy::new(
            crate::render_state::RenderProxyId(0),
            runtime_id,
            EntityId::from("entity-a"),
            transform(1.0),
            renderable("mesh-a"),
        ));
        world.remove_renderable(&EntityId::from("entity-a"));
        let mut extract = RenderExtractContext::new();

        let mut queue = extract.extract_world_dirty(1, &mut world, &scene);
        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command_type, RenderCommandType::RemoveProxy);
    }

    #[test]
    fn remove_sprite_renderer2d_extracts_remove_proxy() {
        let mut world = World::new();
        let id = EntityId::from("entity-sprite");
        world.spawn_with_components(
            id.clone(),
            "Sprite",
            "actor",
            true,
            hierarchy(),
            Some(transform(1.0)),
            None,
        );
        world.insert_sprite_renderer2d(id.clone(), sprite("sprite-a"));
        let mut scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let mut queue = extract.extract_world_dirty(1, &mut world, &scene);
        let merged = queue.normalize_merge(&scene);
        apply_batch(&mut scene, &merged);

        world.remove_sprite_renderer2d(&id);
        let mut queue = extract.extract_world_dirty(2, &mut world, &scene);
        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command_type, RenderCommandType::RemoveProxy);
    }

    #[test]
    fn dirty_records_are_consumed_once() {
        let mut world = world_with_renderable();
        world.insert_transform(EntityId::from("entity-a"), transform(2.0));
        let scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let first = extract.extract_world_dirty(1, &mut world, &scene);
        let second = extract.extract_world_dirty(2, &mut world, &scene);

        assert_eq!(first.pending_commands.len(), 1);
        assert!(second.pending_commands.is_empty());
    }

    #[test]
    fn game_and_scene_view_share_proxies_but_not_view_state() {
        let world = world_with_renderable();
        let mut scene = RenderSceneState::new();
        let runtime_id = world
            .runtime_id_for_source(&EntityId::from("entity-a"))
            .unwrap();
        scene.insert_proxy(crate::render_state::RenderProxy::new(
            crate::render_state::RenderProxyId(0),
            runtime_id,
            EntityId::from("entity-a"),
            transform(1.0),
            renderable("mesh-a"),
        ));
        scene.register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        ));
        scene.register_view(RenderViewState::new(
            RenderViewId(2),
            RenderViewKind::SceneView,
            RenderTargetKind::ViewportTexture,
        ));

        let game_data = scene.build_frame_view_data(RenderViewId(1)).unwrap();
        let scene_data = scene.build_frame_view_data(RenderViewId(2)).unwrap();

        assert_eq!(game_data.visible_proxy_ids, scene_data.visible_proxy_ids);
        assert_ne!(
            scene.view(RenderViewId(1)).unwrap().view_kind,
            scene.view(RenderViewId(2)).unwrap().view_kind
        );
    }

    #[test]
    fn frame_view_data_is_per_view_and_not_stored_as_scene_truth() {
        let mut scene = RenderSceneState::new();
        scene.register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        ));

        let first = scene.build_frame_view_data(RenderViewId(1)).unwrap();
        let second = scene.build_frame_view_data(RenderViewId(1)).unwrap();

        assert_eq!(first.view_id, second.view_id);
        assert!(scene.view(RenderViewId(1)).is_some());
        assert_eq!(scene.proxies_len(), 0);
    }

    #[test]
    fn extracted_commands_apply_to_render_scene_state() {
        let mut world = world_with_renderable();
        world.insert_renderable(EntityId::from("entity-a"), renderable("mesh-a"));
        let mut scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let mut queue = extract.extract_world_dirty(1, &mut world, &scene);
        let merged = queue.normalize_merge(&scene);

        let diagnostics = apply_batch(&mut scene, &merged);

        assert!(diagnostics.is_empty());
        assert_eq!(scene.proxies_len(), 1);
    }
}
