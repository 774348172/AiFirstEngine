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
use crate::render_state::{RenderProxyDescriptor, RenderSceneState};
use crate::world::{DirtyRecord, DirtyType, World};

#[derive(Debug, Clone, Default)]
pub struct RenderExtractContext {
    next_command_id: u64,
}

impl RenderExtractContext {
    pub fn new() -> Self {
        Self { next_command_id: 1 }
    }

    pub fn extract_world_dirty(
        &mut self,
        frame_index: u64,
        world: &mut World,
        scene: &RenderSceneState,
    ) -> RenderCommandQueue {
        let dirty_records = world.take_dirty_records();
        self.extract_dirty_records(frame_index, world, scene, dirty_records)
    }

    pub fn extract_dirty_records(
        &mut self,
        frame_index: u64,
        world: &World,
        scene: &RenderSceneState,
        dirty_records: Vec<DirtyRecord>,
    ) -> RenderCommandQueue {
        let mut buffer = ThreadLocalCommandBuffer::new();
        for dirty_record in dirty_records {
            if let Some(command) = self.command_for_dirty(frame_index, world, scene, dirty_record) {
                buffer.push(command);
            }
        }
        let mut queue = RenderCommandQueue::new(frame_index);
        queue.collect(vec![buffer]);
        queue
    }

    fn command_for_dirty(
        &mut self,
        frame_index: u64,
        world: &World,
        scene: &RenderSceneState,
        dirty_record: DirtyRecord,
    ) -> Option<RenderCommand> {
        let runtime_entity_id = world.runtime_id_for_source(&dirty_record.entity_id)?;
        let proxy_id = scene.proxy_for_source(&dirty_record.entity_id);
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
    ) -> Option<RenderCommand> {
        let command_id = self.next_command_id();
        let descriptor = render_proxy_descriptor(world, &source_entity_id);
        match (proxy_id, descriptor) {
            (None, Some(descriptor)) => {
                let transform = world.transform(&source_entity_id)?.clone();
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
                runtime_entity_id,
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
    if let Some(sprite) = world.sprite_renderer2d(source_entity_id).cloned() {
        return Some(RenderProxyDescriptor::from_sprite_renderer2d(sprite));
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
