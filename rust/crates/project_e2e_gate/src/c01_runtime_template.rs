use engine_runtime::archetype::ComponentValue;
use engine_runtime::aui::{
    AuiBindingValue, AuiSnapshotSource, ProjectUiStateProducerContext, ProjectUiStateSnapshot,
    ProjectUiStateSnapshotOutput, ProjectUiStateSnapshotProducer,
};
use engine_runtime::component_value::RuntimeValue;
use engine_runtime::components::ComponentTypeId;
use engine_runtime::field_path::FieldPath;
use engine_runtime::ids::EntityId;
use engine_runtime::logic_executor::{ExecutorKind, LogicContext, LogicResult};
use engine_runtime::project_runtime_module::{
    project_runtime_aot_digest, LinkedProjectRuntimeSet, ProjectRuntimeAotDigestSource,
    ProjectRuntimeError, ProjectRuntimeModule, ProjectRuntimeModuleDescriptor,
    ProjectRuntimeRegistration, PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
};
use engine_runtime::query::QuerySpec;
use engine_runtime::runtime_package::RuntimeAssetRef;
use engine_runtime::world::World;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

pub const MODULE_ID: &str = "project.c01.runtime";
const RULE_ID: &str = "project.rule.c01.tick";
const RULE_ARTIFACT: &str = "__ARTIFACT_ID__";

pub struct C01RuntimeModule;

impl ProjectRuntimeModule for C01RuntimeModule {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
        static DESCRIPTOR: OnceLock<ProjectRuntimeModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| {
            let digest = project_runtime_aot_digest(
                MODULE_ID,
                PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
                "RuntimeModule/Cargo.toml",
                "c01_project_runtime",
                "c01_project_player",
                [
                    ProjectRuntimeAotDigestSource {
                        relative_path: "RuntimeModule/Cargo.toml",
                        bytes: include_bytes!("../Cargo.toml"),
                    },
                    ProjectRuntimeAotDigestSource {
                        relative_path: "RuntimeModule/src/lib.rs",
                        bytes: include_bytes!("lib.rs"),
                    },
                ],
            )
            .expect("C-01 project source must have a canonical digest");
            ProjectRuntimeModuleDescriptor::new(MODULE_ID, digest)
        })
    }

    fn install(
        &self,
        registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeError> {
        registration.register_rust_aot_rule(RULE_ID, RULE_ARTIFACT, c01_tick)?;
        registration.set_ui_state_producer_factory(|| Box::new(C01UiProducer))
    }
}
pub fn linked_set() -> Result<LinkedProjectRuntimeSet, ProjectRuntimeError> {
    LinkedProjectRuntimeSet::singleton(Arc::new(C01RuntimeModule))
}

fn c01_tick(context: &mut LogicContext<'_>) -> LogicResult {
    let mut result = LogicResult::applied(RULE_ID, ExecutorKind::RustAot);
    let player_ty = ComponentTypeId::from("project.c01Player");
    let session_ty = ComponentTypeId::from("project.c01Session");
    let motion_ty = ComponentTypeId::from("project.c01Motion");
    let combat_ty = ComponentTypeId::from("project.c01Combat");

    for player in context.query(QuerySpec::all([
        ComponentTypeId::transform(),
        player_ty.clone(),
    ])) {
        let fields = dynamic_fields(context, &player, &player_ty).unwrap_or_default();
        let mut hp = number(&fields, "hp").unwrap_or(3.0);
        let mut dash_remaining = number(&fields, "dashRemaining").unwrap_or(0.0);
        let mut dash_cooldown = number(&fields, "dashCooldownRemaining").unwrap_or(0.0);
        let mut fire_remaining = number(&fields, "fireRemaining").unwrap_or(0.0);
        let speed = number(&fields, "speed").unwrap_or(7.5) as f32;
        let delta_time = f64::from(context.delta_time);
        dash_remaining = (dash_remaining - delta_time).max(0.0);
        dash_cooldown = (dash_cooldown - delta_time).max(0.0);
        fire_remaining = (fire_remaining - delta_time).max(0.0);
        if context.action_pressed("action.dash") && dash_cooldown <= 0.0 {
            dash_remaining = 0.2;
            dash_cooldown = 2.0;
        }
        if let Some(axis) = context
            .action_snapshot()
            .and_then(|snapshot| snapshot.axis2("action.move"))
        {
            if let Ok(mut position) = context.read_transform_local_position(&player) {
                let multiplier = if dash_remaining > 0.0 { 2.0 } else { 1.0 };
                position.x = (position.x + axis.x * speed * multiplier * context.delta_time)
                    .clamp(-6.6, 6.6);
                position.y = (position.y + axis.y * speed * multiplier * context.delta_time)
                    .clamp(-5.0, 5.0);
                if let Ok(write) = context.write_transform_local_position(player.clone(), position)
                {
                    result.writes.push(write);
                }
            }
        }
        if context.action_pressed("action.fire") && fire_remaining <= 0.0 && hp > 0.0 {
            fire_remaining = 0.2;
            context.request_instantiate_prefab(prefab_ref("prefab-c01-bullet"), None, None);
            if let Some(session) = context
                .query(QuerySpec::all([session_ty.clone()]))
                .into_iter()
                .next()
            {
                let session_fields =
                    dynamic_fields(context, &session, &session_ty).unwrap_or_default();
                let serial = integer(&session_fields, "bulletSerial").unwrap_or(10) + 1;
                write_field(
                    context,
                    &mut result,
                    session,
                    session_ty.clone(),
                    "bulletSerial",
                    RuntimeValue::I64(serial),
                );
            }
        }
        for pair in context.collision_pairs().to_vec() {
            let other = if pair.entity_a == player {
                Some(pair.entity_b)
            } else if pair.entity_b == player {
                Some(pair.entity_a)
            } else {
                None
            };
            if let Some(other) = other {
                if team(context, &other, &combat_ty).as_deref() == Some("enemy") {
                    hp = (hp - 1.0).max(0.0);
                    context.request_despawn_entity(other);
                }
            }
        }
        for (field, value) in [
            ("hp", RuntimeValue::F64(hp)),
            ("dashRemaining", RuntimeValue::F64(dash_remaining)),
            ("dashCooldownRemaining", RuntimeValue::F64(dash_cooldown)),
            ("fireRemaining", RuntimeValue::F64(fire_remaining)),
        ] {
            write_field(
                context,
                &mut result,
                player.clone(),
                player_ty.clone(),
                field,
                value,
            );
        }
    }

    for entity in context.query(QuerySpec::all([
        ComponentTypeId::transform(),
        motion_ty.clone(),
    ])) {
        let fields = dynamic_fields(context, &entity, &motion_ty).unwrap_or_default();
        let (vx, vy) = vector2(&fields, "velocity").unwrap_or((0.0, 0.0));
        if let Ok(mut position) = context.read_transform_local_position(&entity) {
            position.x += vx * context.delta_time;
            position.y += vy * context.delta_time;
            if let Ok(write) = context.write_transform_local_position(entity.clone(), position) {
                result.writes.push(write);
            }
            if position.y.abs() > 8.0 || position.x.abs() > 8.0 {
                context.request_despawn_entity(entity);
            }
        }
    }

    for pair in context.collision_pairs().to_vec() {
        let left_team = team(context, &pair.entity_a, &combat_ty).unwrap_or_default();
        let right_team = team(context, &pair.entity_b, &combat_ty).unwrap_or_default();
        let hit = if left_team == "playerBullet" && right_team == "enemy" {
            Some((pair.entity_a, pair.entity_b))
        } else if right_team == "playerBullet" && left_team == "enemy" {
            Some((pair.entity_b, pair.entity_a))
        } else {
            None
        };
        if let Some((bullet, enemy)) = hit {
            context.request_despawn_entity(bullet);
            context.request_despawn_entity(enemy);
            for session in context.query(QuerySpec::all([session_ty.clone()])) {
                let fields = dynamic_fields(context, &session, &session_ty).unwrap_or_default();
                let score = integer(&fields, "score").unwrap_or(0) + 100;
                write_field(
                    context,
                    &mut result,
                    session,
                    session_ty.clone(),
                    "score",
                    RuntimeValue::I64(score),
                );
            }
        }
    }

    for session in context.query(QuerySpec::all([session_ty.clone()])) {
        let fields = dynamic_fields(context, &session, &session_ty).unwrap_or_default();
        let mut wave = integer(&fields, "wave").unwrap_or(1);
        let mut game_over = bool_value(&fields, "gameOver").unwrap_or(false);
        let player_hp = context
            .query(QuerySpec::all([player_ty.clone()]))
            .into_iter()
            .next()
            .and_then(|id| dynamic_fields(context, &id, &player_ty))
            .and_then(|values| number(&values, "hp"))
            .unwrap_or(0.0);
        if player_hp <= 0.0 {
            game_over = true;
        }
        let enemy_count = context
            .query(QuerySpec::all([combat_ty.clone()]))
            .into_iter()
            .filter(|id| team(context, id, &combat_ty).as_deref() == Some("enemy"))
            .count();
        if enemy_count == 0 && !game_over {
            wave += 1;
            for _ in 0..5 {
                context.request_instantiate_prefab(prefab_ref("prefab-c01-enemy"), None, None);
            }
            write_field(
                context,
                &mut result,
                session.clone(),
                session_ty.clone(),
                "spawnedWave",
                RuntimeValue::I64(wave),
            );
            let enemy_serial = integer(&fields, "enemySerial").unwrap_or(10) + 5;
            write_field(
                context,
                &mut result,
                session.clone(),
                session_ty.clone(),
                "enemySerial",
                RuntimeValue::I64(enemy_serial),
            );
        }
        if context.action_pressed("action.restart") {
            wave = 1;
            game_over = false;
            write_field(
                context,
                &mut result,
                session.clone(),
                session_ty.clone(),
                "score",
                RuntimeValue::I64(0),
            );
            for player in context.query(QuerySpec::all([player_ty.clone()])) {
                write_field(
                    context,
                    &mut result,
                    player,
                    player_ty.clone(),
                    "hp",
                    RuntimeValue::F64(3.0),
                );
            }
        }
        write_field(
            context,
            &mut result,
            session.clone(),
            session_ty.clone(),
            "wave",
            RuntimeValue::I64(wave),
        );
        write_field(
            context,
            &mut result,
            session,
            session_ty.clone(),
            "gameOver",
            RuntimeValue::Bool(game_over),
        );
    }
    result
}

fn dynamic_fields(
    context: &mut LogicContext<'_>,
    entity: &EntityId,
    ty: &ComponentTypeId,
) -> Option<BTreeMap<String, RuntimeValue>> {
    match context.read_component(entity, ty).ok()? {
        ComponentValue::Dynamic {
            value: RuntimeValue::Object(fields),
            ..
        } => Some(fields),
        _ => None,
    }
}

fn number(fields: &BTreeMap<String, RuntimeValue>, key: &str) -> Option<f64> {
    match fields.get(key)? {
        RuntimeValue::F64(v) => Some(*v),
        RuntimeValue::I64(v) => Some(*v as f64),
        _ => None,
    }
}
fn integer(fields: &BTreeMap<String, RuntimeValue>, key: &str) -> Option<i64> {
    match fields.get(key)? {
        RuntimeValue::I64(v) => Some(*v),
        RuntimeValue::F64(v) => Some(*v as i64),
        _ => None,
    }
}
fn bool_value(fields: &BTreeMap<String, RuntimeValue>, key: &str) -> Option<bool> {
    match fields.get(key)? {
        RuntimeValue::Bool(v) => Some(*v),
        _ => None,
    }
}
fn vector2(fields: &BTreeMap<String, RuntimeValue>, key: &str) -> Option<(f32, f32)> {
    let RuntimeValue::Object(value) = fields.get(key)? else {
        return None;
    };
    Some((number(value, "x")? as f32, number(value, "y")? as f32))
}
fn team(context: &mut LogicContext<'_>, entity: &EntityId, ty: &ComponentTypeId) -> Option<String> {
    let fields = dynamic_fields(context, entity, ty)?;
    match fields.get("team")? {
        RuntimeValue::String(value) => Some(value.clone()),
        _ => None,
    }
}
fn write_field(
    context: &mut LogicContext<'_>,
    result: &mut LogicResult,
    entity: EntityId,
    ty: ComponentTypeId,
    field: &str,
    value: RuntimeValue,
) {
    if let Ok(write) = context.write_component_field(
        entity,
        ty,
        &FieldPath::parse(field).expect("static field"),
        value,
    ) {
        result.writes.push(write);
    }
}

fn prefab_ref(id: &str) -> RuntimeAssetRef {
    RuntimeAssetRef {
        id: id.to_string(),
        asset_type: "prefab".to_string(),
        guid: None,
        sub_asset: None,
    }
}

struct C01UiProducer;
impl ProjectUiStateSnapshotProducer for C01UiProducer {
    fn producer_id(&self) -> &str {
        "c01_ui_state"
    }
    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput {
        let player =
            world_fields(context.world, "entity-player", "project.c01Player").unwrap_or_default();
        let session =
            world_fields(context.world, "entity-session", "project.c01Session").unwrap_or_default();
        let hp = integer(&player, "hp").unwrap_or(0);
        let score = integer(&session, "score").unwrap_or(0);
        let wave = integer(&session, "wave").unwrap_or(1);
        let dash = number(&player, "dashCooldownRemaining").unwrap_or(0.0);
        let game_over = bool_value(&session, "gameOver").unwrap_or(false);
        let snapshot = ProjectUiStateSnapshot::new(context.frame_index)
            .with_value("hud.hp", AuiBindingValue::String(format!("HP {hp}")))
            .with_value(
                "hud.score",
                AuiBindingValue::String(format!("SCORE {score:06}")),
            )
            .with_value("hud.wave", AuiBindingValue::String(format!("WAVE {wave}")))
            .with_value(
                "hud.dash",
                AuiBindingValue::String(if dash <= 0.0 {
                    "DASH READY".to_string()
                } else {
                    format!("DASH COOLDOWN {dash:.1}")
                }),
            )
            .with_value(
                "hud.game_over",
                AuiBindingValue::String(if game_over {
                    "GAME OVER - PRESS R".to_string()
                } else {
                    String::new()
                }),
            );
        ProjectUiStateSnapshotOutput::new(
            self.producer_id(),
            AuiSnapshotSource::ProjectProducer,
            snapshot,
        )
    }
}

fn world_fields(
    world: &World,
    entity: &str,
    component: &str,
) -> Option<BTreeMap<String, RuntimeValue>> {
    match world.component_value(&EntityId::from(entity), &ComponentTypeId::from(component))? {
        ComponentValue::Dynamic {
            value: RuntimeValue::Object(fields),
            ..
        } => Some(fields),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linked_descriptor_is_project_owned() {
        let linked = linked_set().unwrap();
        assert_eq!(linked.only_descriptor().unwrap().module_id, MODULE_ID);
        assert!(linked
            .only_descriptor()
            .unwrap()
            .aot_content_digest
            .starts_with("sha256:"));
    }
}
