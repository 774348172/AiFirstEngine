use crate::archetype::ComponentValue;
use crate::components::{ComponentTypeId, Hierarchy};
use crate::ids::{EntityId, RuntimeEntityId, SourceEntityId};
use crate::runtime_instance::RuntimeInstanceId;
use crate::runtime_instance_loader::RuntimeInstanceLoader;
use crate::runtime_package::{RuntimeAssetRef, RuntimePackage};
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameplayCommandId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub enum GameplayCommand {
    SpawnEntity {
        entity_id: EntityId,
        name: String,
        kind: String,
        enabled: bool,
        parent_id: Option<EntityId>,
        components: Vec<ComponentValue>,
    },
    DespawnEntity {
        entity_id: EntityId,
    },
    AddComponent {
        entity_id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
    },
    RemoveComponent {
        entity_id: EntityId,
        component_type: ComponentTypeId,
    },
    SetParent {
        entity_id: EntityId,
        parent_id: Option<EntityId>,
        keep_world_transform: bool,
    },
    InstantiatePrefab {
        prefab_ref: RuntimeAssetRef,
        parent_entity: Option<SourceEntityId>,
        target_scene_instance: Option<RuntimeInstanceId>,
    },
    DespawnPrefabInstance {
        instance_id: RuntimeInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayCommandApplyRecord {
    pub command_id: GameplayCommandId,
    pub operation: &'static str,
    pub entity_id: EntityId,
    pub result: &'static str,
    pub error_code: Option<&'static str>,
    pub instance_id: Option<RuntimeInstanceId>,
    pub prefab_ref_id: Option<String>,
    pub created_entity_count: usize,
    pub root_entity_id: Option<RuntimeEntityId>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameplayCommandBuffer {
    commands: Vec<(GameplayCommandId, GameplayCommand)>,
    next_id: u64,
}

impl GameplayCommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn commands(&self) -> &[(GameplayCommandId, GameplayCommand)] {
        &self.commands
    }

    pub fn push(&mut self, command: GameplayCommand) -> GameplayCommandId {
        let command_id = GameplayCommandId(self.next_id);
        self.next_id += 1;
        self.commands.push((command_id, command));
        command_id
    }

    pub fn drain(&mut self) -> Vec<(GameplayCommandId, GameplayCommand)> {
        self.commands.drain(..).collect()
    }
}

pub fn apply_gameplay_commands(
    world: &mut World,
    commands: Vec<(GameplayCommandId, GameplayCommand)>,
) -> Vec<GameplayCommandApplyRecord> {
    commands
        .into_iter()
        .map(|(command_id, command)| apply_gameplay_command(world, command_id, command, None))
        .collect()
}

pub struct RuntimeCommandContext<'a> {
    pub package: &'a RuntimePackage,
    pub instance_loader: &'a mut RuntimeInstanceLoader,
}

pub fn apply_gameplay_commands_with_runtime(
    world: &mut World,
    commands: Vec<(GameplayCommandId, GameplayCommand)>,
    runtime_context: RuntimeCommandContext<'_>,
) -> Vec<GameplayCommandApplyRecord> {
    let mut context = runtime_context;
    commands
        .into_iter()
        .map(|(command_id, command)| {
            apply_gameplay_command(world, command_id, command, Some(&mut context))
        })
        .collect()
}

fn apply_gameplay_command(
    world: &mut World,
    command_id: GameplayCommandId,
    command: GameplayCommand,
    runtime_context: Option<&mut RuntimeCommandContext<'_>>,
) -> GameplayCommandApplyRecord {
    match command {
        GameplayCommand::SpawnEntity {
            entity_id,
            name,
            kind,
            enabled,
            parent_id,
            components,
        } => {
            if let Err(error) = world.try_spawn_entity(
                entity_id.clone(),
                name,
                kind,
                enabled,
                Hierarchy {
                    parent_id,
                    sibling_order: 0,
                },
            ) {
                return failed_world_mutation_record(
                    command_id,
                    "spawn_entity",
                    entity_id,
                    error.code,
                );
            }
            for value in components {
                let component_type = value.component_type();
                if let Err(error) =
                    world.try_insert_component_value(entity_id.clone(), component_type, value)
                {
                    return failed_world_mutation_record(
                        command_id,
                        "spawn_entity",
                        entity_id,
                        error.code,
                    );
                }
            }
            GameplayCommandApplyRecord {
                command_id,
                operation: "spawn_entity",
                entity_id,
                result: "ok",
                error_code: None,
                instance_id: None,
                prefab_ref_id: None,
                created_entity_count: 1,
                root_entity_id: None,
            }
        }
        GameplayCommand::DespawnEntity { entity_id } => {
            let result = world.try_despawn_entity(&entity_id);
            GameplayCommandApplyRecord {
                command_id,
                operation: "despawn_entity",
                entity_id,
                result: if result.is_ok() { "ok" } else { "failed" },
                error_code: result.err().map(|error| error.code),
                instance_id: None,
                prefab_ref_id: None,
                created_entity_count: 0,
                root_entity_id: None,
            }
        }
        GameplayCommand::AddComponent {
            entity_id,
            component_type,
            value,
        } => {
            if value.component_type() != component_type {
                return GameplayCommandApplyRecord {
                    command_id,
                    operation: "add_component",
                    entity_id,
                    result: "failed",
                    error_code: Some("world.component.type_mismatch"),
                    instance_id: None,
                    prefab_ref_id: None,
                    created_entity_count: 0,
                    root_entity_id: None,
                };
            }
            if let Err(error) =
                world.try_insert_component_value(entity_id.clone(), component_type, value)
            {
                return failed_world_mutation_record(
                    command_id,
                    "add_component",
                    entity_id,
                    error.code,
                );
            }
            GameplayCommandApplyRecord {
                command_id,
                operation: "add_component",
                entity_id,
                result: "ok",
                error_code: None,
                instance_id: None,
                prefab_ref_id: None,
                created_entity_count: 0,
                root_entity_id: None,
            }
        }
        GameplayCommand::RemoveComponent {
            entity_id,
            component_type,
        } => {
            let result = world.try_remove_component_value(&entity_id, &component_type);
            GameplayCommandApplyRecord {
                command_id,
                operation: "remove_component",
                entity_id,
                result: if result.is_ok() { "ok" } else { "failed" },
                error_code: result.err().map(|error| error.code),
                instance_id: None,
                prefab_ref_id: None,
                created_entity_count: 0,
                root_entity_id: None,
            }
        }
        GameplayCommand::SetParent {
            entity_id,
            parent_id,
            keep_world_transform: _,
        } => {
            if let Err(error) = world.try_set_parent(entity_id.clone(), parent_id) {
                return failed_world_mutation_record(
                    command_id,
                    "set_parent",
                    entity_id,
                    error.code,
                );
            }
            GameplayCommandApplyRecord {
                command_id,
                operation: "set_parent",
                entity_id,
                result: "ok",
                error_code: None,
                instance_id: None,
                prefab_ref_id: None,
                created_entity_count: 0,
                root_entity_id: None,
            }
        }
        GameplayCommand::InstantiatePrefab {
            prefab_ref,
            parent_entity,
            target_scene_instance,
        } => {
            let prefab_ref_id = prefab_ref.id.clone();
            let Some(runtime_context) = runtime_context else {
                return GameplayCommandApplyRecord {
                    command_id,
                    operation: "instantiate_prefab",
                    entity_id: EntityId::from(format!("prefab:{prefab_ref_id}")),
                    result: "failed",
                    error_code: Some("missing_runtime_context"),
                    instance_id: None,
                    prefab_ref_id: Some(prefab_ref_id),
                    created_entity_count: 0,
                    root_entity_id: None,
                };
            };
            let (instance, report) = runtime_context
                .instance_loader
                .instantiate_prefab_from_package(
                    runtime_context.package,
                    prefab_ref,
                    parent_entity,
                    target_scene_instance,
                    world,
                );
            let root_entity_id = instance.as_ref().and_then(|instance| instance.root_entity);
            GameplayCommandApplyRecord {
                command_id,
                operation: "instantiate_prefab",
                entity_id: root_entity_id
                    .map(|root| EntityId::from(root.to_string()))
                    .unwrap_or_else(|| EntityId::from(format!("prefab:{prefab_ref_id}"))),
                result: if report.has_errors() { "failed" } else { "ok" },
                error_code: report.has_errors().then_some("runtime_instance_error"),
                instance_id: report.instance_id,
                prefab_ref_id: Some(prefab_ref_id),
                created_entity_count: report.created_entity_count,
                root_entity_id,
            }
        }
        GameplayCommand::DespawnPrefabInstance { instance_id } => {
            let Some(runtime_context) = runtime_context else {
                return GameplayCommandApplyRecord {
                    command_id,
                    operation: "despawn_prefab_instance",
                    entity_id: EntityId::from(format!("prefab-instance:{}", instance_id)),
                    result: "failed",
                    error_code: Some("missing_runtime_context"),
                    instance_id: Some(instance_id),
                    prefab_ref_id: None,
                    created_entity_count: 0,
                    root_entity_id: None,
                };
            };
            let report = runtime_context
                .instance_loader
                .despawn_prefab_instance(instance_id, world);
            GameplayCommandApplyRecord {
                command_id,
                operation: "despawn_prefab_instance",
                entity_id: EntityId::from(format!("prefab-instance:{}", instance_id)),
                result: if report.has_errors() { "failed" } else { "ok" },
                error_code: report.has_errors().then_some("runtime_instance_error"),
                instance_id: Some(instance_id),
                prefab_ref_id: Some(report.asset_ref),
                created_entity_count: report.created_entity_count,
                root_entity_id: None,
            }
        }
    }
}

fn failed_world_mutation_record(
    command_id: GameplayCommandId,
    operation: &'static str,
    entity_id: EntityId,
    error_code: &'static str,
) -> GameplayCommandApplyRecord {
    GameplayCommandApplyRecord {
        command_id,
        operation,
        entity_id,
        result: "failed",
        error_code: Some(error_code),
        instance_id: None,
        prefab_ref_id: None,
        created_entity_count: 0,
        root_entity_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_value::RuntimeValue;

    #[test]
    fn command_buffer_spawn_entity_applies_after_rule_execution() {
        let mut world = World::new();
        let mut buffer = GameplayCommandBuffer::new();
        let component_type = ComponentTypeId::from("project.marker");
        buffer.push(GameplayCommand::SpawnEntity {
            entity_id: EntityId::from("entity-created"),
            name: "Created".to_string(),
            kind: "actor".to_string(),
            enabled: true,
            parent_id: None,
            components: vec![ComponentValue::Dynamic {
                component_type: component_type.clone(),
                value: RuntimeValue::I64(1),
            }],
        });

        assert!(world.entity(&EntityId::from("entity-created")).is_none());
        let records = apply_gameplay_commands(&mut world, buffer.drain());

        assert_eq!(records[0].operation, "spawn_entity");
        assert!(world.entity(&EntityId::from("entity-created")).is_some());
        assert_eq!(
            world.component_value(&EntityId::from("entity-created"), &component_type),
            Some(ComponentValue::Dynamic {
                component_type,
                value: RuntimeValue::I64(1),
            })
        );
    }

    #[test]
    fn instantiate_prefab_command_requires_runtime_context() {
        let mut world = World::new();
        let records = apply_gameplay_commands(
            &mut world,
            vec![(
                GameplayCommandId(0),
                GameplayCommand::InstantiatePrefab {
                    prefab_ref: RuntimeAssetRef {
                        id: "prefab-ship".to_string(),
                        asset_type: "prefab".to_string(),
                        guid: None,
                        sub_asset: None,
                    },
                    parent_entity: None,
                    target_scene_instance: None,
                },
            )],
        );

        assert_eq!(records[0].operation, "instantiate_prefab");
        assert_eq!(records[0].result, "failed");
        assert_eq!(records[0].error_code, Some("missing_runtime_context"));
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn despawn_prefab_instance_command_requires_runtime_context() {
        let mut world = World::new();
        let records = apply_gameplay_commands(
            &mut world,
            vec![(
                GameplayCommandId(0),
                GameplayCommand::DespawnPrefabInstance {
                    instance_id: RuntimeInstanceId(1),
                },
            )],
        );

        assert_eq!(records[0].operation, "despawn_prefab_instance");
        assert_eq!(records[0].result, "failed");
        assert_eq!(records[0].error_code, Some("missing_runtime_context"));
        assert_eq!(records[0].instance_id, Some(RuntimeInstanceId(1)));
    }

    #[test]
    fn invalid_world_commands_return_stable_failures_without_unwind() {
        let mut world = World::new();
        let entity = EntityId::from("entity-a");
        world
            .try_spawn_entity(
                entity.clone(),
                "A",
                "actor",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
            )
            .unwrap();
        let commands = vec![
            (
                GameplayCommandId(0),
                GameplayCommand::SpawnEntity {
                    entity_id: entity.clone(),
                    name: "Duplicate".to_string(),
                    kind: "actor".to_string(),
                    enabled: true,
                    parent_id: None,
                    components: Vec::new(),
                },
            ),
            (
                GameplayCommandId(1),
                GameplayCommand::AddComponent {
                    entity_id: EntityId::from("missing"),
                    component_type: ComponentTypeId::from("project.value"),
                    value: ComponentValue::Dynamic {
                        component_type: ComponentTypeId::from("project.value"),
                        value: RuntimeValue::I64(1),
                    },
                },
            ),
            (
                GameplayCommandId(2),
                GameplayCommand::RemoveComponent {
                    entity_id: entity.clone(),
                    component_type: ComponentTypeId::from("project.missing"),
                },
            ),
            (
                GameplayCommandId(3),
                GameplayCommand::SetParent {
                    entity_id: entity.clone(),
                    parent_id: Some(entity.clone()),
                    keep_world_transform: false,
                },
            ),
            (
                GameplayCommandId(4),
                GameplayCommand::DespawnEntity {
                    entity_id: EntityId::from("missing"),
                },
            ),
        ];

        let records = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            apply_gameplay_commands(&mut world, commands)
        }))
        .expect("invalid commands must not unwind");

        assert_eq!(records.len(), 5);
        assert_eq!(records[0].error_code, Some("world.entity.duplicate_id"));
        assert_eq!(records[1].error_code, Some("world.entity.missing"));
        assert_eq!(records[2].error_code, Some("world.component.missing"));
        assert_eq!(records[3].error_code, Some("world.parent.self"));
        assert_eq!(records[4].error_code, Some("world.entity.missing"));
        assert_eq!(world.entity_count(), 1);
    }
}
