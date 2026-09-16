//! Authoring-only expansion into the existing Animator2D contracts.
use super::{assembly_error, CompilerSourceView, GameProjectCompilerError};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Description {
    schema: String,
    asset_id: String,
    entity: Option<String>,
    default: String,
    #[serde(default, deserialize_with = "unique_map")]
    parameters: BTreeMap<String, bool>,
    #[serde(deserialize_with = "unique_map")]
    animations: BTreeMap<String, Animation>,
    #[serde(default)]
    rules: Vec<Rule>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Animation {
    frames: Vec<String>,
    fps: Option<u32>,
    duration_ticks: Option<u32>,
    #[serde(rename = "loop", default)]
    looping: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    when: String,
    play: String,
}

fn unique_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an object with unique names")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, T>()? {
                if result.insert(key.clone(), value).is_some() {
                    return Err(serde::de::Error::custom(format!("duplicate name '{key}'")));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}

fn failure(path: &str, field: &str, message: impl std::fmt::Display) -> GameProjectCompilerError {
    assembly_error(
        "animator2d.description_invalid",
        format!("{path} [{field}]: {message}"),
    )
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

pub(super) fn expand(
    source: &CompilerSourceView,
) -> Result<CompilerSourceView, GameProjectCompilerError> {
    let mut expanded = source.clone();
    let textures: BTreeSet<String> = source
        .paths()
        .filter(|p| p.starts_with("Assets/") && p.ends_with(".asset"))
        .filter_map(|p| serde_json::from_slice::<Value>(source.bytes(p)?).ok())
        .filter(|v| v["schemaVersion"].as_str() == Some("texture-asset.v1"))
        .filter_map(|v| v["assetId"].as_str().map(str::to_owned))
        .collect();
    let mut identities = BTreeSet::new();
    for path in source
        .paths()
        .filter(|p| p.starts_with("Animations/") && p.ends_with(".animator-description-2d.json"))
    {
        let desc: Description = serde_json::from_slice(source.bytes(path).unwrap())
            .map_err(|e| failure(path, "description", e))?;
        if desc.schema != "animator-description-2d.v1"
            || !valid_name(&desc.asset_id)
            || !identities.insert(desc.asset_id.clone())
        {
            return Err(failure(
                path,
                "assetId/schema",
                "expected supported schema and unique stable assetId",
            ));
        }
        if !desc.animations.contains_key(&desc.default) {
            return Err(failure(path, "default", "must name an existing animation"));
        }
        if desc.animations.len() > 32 || desc.parameters.len() > 8 || desc.rules.len() > 32 {
            return Err(failure(
                path,
                "budget",
                "maximum 32 animations, 8 boolean parameters, 32 rules",
            ));
        }
        let names: Vec<_> = desc.parameters.keys().collect();
        if names.iter().any(|n| !valid_name(n)) {
            return Err(failure(path, "parameters", "invalid parameter name"));
        }
        let mut predicates = Vec::new();
        for (i, rule) in desc.rules.iter().enumerate() {
            if !desc.animations.contains_key(&rule.play) {
                return Err(failure(
                    path,
                    &format!("rules[{i}].play"),
                    "unknown animation",
                ));
            }
            let mut terms = Vec::new();
            for term in rule.when.split("&&") {
                let term = term.trim();
                let (name, value) = term
                    .strip_prefix('!')
                    .map(|n| (n.trim(), false))
                    .unwrap_or((term, true));
                let index = names
                    .iter()
                    .position(|n| n.as_str() == name)
                    .ok_or_else(|| {
                        failure(
                            path,
                            &format!("rules[{i}].when"),
                            format!("unknown boolean '{name}'; only ! and && are supported"),
                        )
                    })?;
                if terms.iter().any(|(old, _)| *old == index) {
                    return Err(failure(
                        path,
                        "rules.when",
                        "duplicate or conflicting condition",
                    ));
                }
                terms.push((index, value));
            }
            predicates.push(terms);
        }
        let mut states = Vec::new();
        for (name, animation) in &desc.animations {
            if !valid_name(name) || animation.frames.is_empty() {
                return Err(failure(
                    path,
                    &format!("animations.{name}"),
                    "valid name and nonempty frames required",
                ));
            }
            let duration = match (animation.fps, animation.duration_ticks) {
                (Some(fps), None) if fps > 0 && fps <= 60 && 60 % fps == 0 => 60 / fps,
                (None, Some(ticks)) if ticks > 0 => ticks,
                (None, None) if animation.frames.len() == 1 => 1,
                _ => {
                    return Err(failure(
                        path,
                        &format!("animations.{name}.fps/durationTicks"),
                        "use fps dividing 60, or a positive integer durationTicks; not both",
                    ))
                }
            };
            for frame in &animation.frames {
                if !textures.contains(frame) {
                    return Err(failure(
                        path,
                        &format!("animations.{name}.frames"),
                        format!("missing texture asset '{frame}'"),
                    ));
                }
            }
            let clip = format!("{}.{}", desc.asset_id, name);
            states.push(json!({"id":name,"clipRef":clip}));
            insert(
                &mut expanded,
                format!("{path}.{name}.sprite-animation-clip-2d.json"),
                json!({"schema":"sprite-animation-clip-2d.v1","assetId":clip,"playback":if animation.looping {"loop"} else {"once"},"frames":animation.frames.iter().map(|s| json!({"spriteRef":s,"durationTicks":duration})).collect::<Vec<_>>()}),
                path,
            )?;
        }
        // Enumerate the bounded boolean domain only while cooking. Mutually exclusive
        // transitions avoid self-transition resets and preserve first-match semantics.
        let mut transitions = Vec::new();
        for bits in 0..(1usize << names.len()) {
            let target = desc
                .rules
                .iter()
                .zip(&predicates)
                .find(|(_, terms)| {
                    terms
                        .iter()
                        .all(|(i, value)| (bits & (1 << i) != 0) == *value)
                })
                .map(|(r, _)| &r.play)
                .unwrap_or(&desc.default);
            let conditions: Vec<_> = names
                .iter()
                .enumerate()
                .map(|(i, n)| json!({"parameter":n,"op":"equals","value":bits & (1 << i) != 0}))
                .collect();
            for from in desc.animations.keys().filter(|name| *name != target) {
                transitions.push(json!({"id":format!("select.{bits:03}.{from}"),"from":from,"to":target,"when":"immediate","conditions":conditions}));
            }
        }
        insert(
            &mut expanded,
            format!("{path}.animator-controller-2d.json"),
            json!({"schema":"animator-controller-2d.v1","assetId":desc.asset_id,"entryStateId":desc.default,"states":states,"parameters":desc.parameters.iter().map(|(id,value)| json!({"id":id,"kind":"bool","defaultBool":value})).collect::<Vec<_>>(),"transitions":transitions}),
            path,
        )?;
        if let Some(entity) = &desc.entity {
            let scene_paths: Vec<_> = expanded
                .paths()
                .filter(|p| p.starts_with("Scenes/") && p.ends_with(".scene.json"))
                .map(str::to_owned)
                .collect();
            let mut count = 0;
            for scene_path in scene_paths {
                let mut scene: Value = serde_json::from_slice(expanded.bytes(&scene_path).unwrap())
                    .map_err(|e| failure(path, "entity", e))?;
                if let Some(entities) = scene["entities"].as_array_mut() {
                    for target in entities
                        .iter_mut()
                        .filter(|v| v["id"].as_str() == Some(entity))
                    {
                        count += 1;
                        if target.get("components").is_none() {
                            target["components"] = json!([]);
                        }
                        let components = target["components"]
                            .as_array_mut()
                            .ok_or_else(|| failure(path, "entity.components", "expected array"))?;
                        if components.iter().any(|c| {
                            c["componentType"]
                                .as_str()
                                .is_some_and(|t| t.eq_ignore_ascii_case("Animator2D"))
                        }) {
                            return Err(failure(
                                path,
                                "entity",
                                "Animator2D already bound; choose one authoring source",
                            ));
                        }
                        components.push(json!({"componentType":"Animator2D","data":{"controllerRef":desc.asset_id,"enabled":true,"initialBools":desc.parameters}}));
                    }
                }
                expanded
                    .files
                    .insert(scene_path, serde_json::to_vec(&scene).unwrap());
            }
            if count != 1 {
                return Err(failure(
                    path,
                    "entity",
                    format!("'{entity}' must resolve to exactly one scene entity (found {count})"),
                ));
            }
        }
        expanded.files.remove(path);
    }
    Ok(expanded)
}
fn insert(
    source: &mut CompilerSourceView,
    path: String,
    value: Value,
    origin: &str,
) -> Result<(), GameProjectCompilerError> {
    if source.files.contains_key(&path) {
        return Err(failure(
            origin,
            "generatedPath",
            "collides with authored file",
        ));
    }
    source
        .files
        .insert(path, serde_json::to_vec(&value).unwrap());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(text: &str) -> CompilerSourceView {
        CompilerSourceView {
            files: BTreeMap::from([
                (
                    "Animations/robot.animator-description-2d.json".into(),
                    text.as_bytes().to_vec(),
                ),
                (
                    "Assets/frame.asset".into(),
                    br#"{"schemaVersion":"texture-asset.v1","assetId":"frame"}"#.to_vec(),
                ),
            ]),
        }
    }
    const VALID: &str = r#"{"schema":"animator-description-2d.v1","assetId":"robot","default":"idle","parameters":{"moving":false},"animations":{"idle":{"frames":["frame"],"loop":true},"walk":{"frames":["frame","frame"],"fps":10,"loop":true}},"rules":[{"when":"moving","play":"walk"}]}"#;
    #[test]
    fn animator2d_description_generates_mutually_exclusive_rules_and_stable_identity() {
        let input = source(VALID);
        let expanded = expand(&input).unwrap();
        let a = super::super::cook_animator2d_registry(&expanded).unwrap();
        assert_eq!(a.controllers[0].id, "robot");
        assert_eq!(a.controllers[0].transitions.len(), 2);
        assert_eq!(a.clips[1].frames[0].duration_ticks, 6);
        let mut renamed = input.clone();
        let bytes = renamed
            .files
            .remove("Animations/robot.animator-description-2d.json")
            .unwrap();
        renamed.files.insert(
            "Animations/moved.animator-description-2d.json".into(),
            bytes,
        );
        assert_eq!(
            a,
            super::super::cook_animator2d_registry(&expand(&renamed).unwrap()).unwrap()
        );
        assert_eq!(input, source(VALID));
    }
    #[test]
    fn animator2d_description_rejects_invalid_inputs_instead_of_defaulting() {
        for broken in [
            VALID.replace("\"fps\":10", "\"fps\":7"),
            VALID.replace("\"fps\":10", "\"fps\":\"fast\""),
            VALID.replace("\"default\":\"idle\"", "\"default\":\"missing\""),
            VALID.replace("\"when\":\"moving\"", "\"when\":\"unknown\""),
            VALID.replace("\"play\":\"walk\"", "\"play\":\"missing\""),
            VALID.replace("[\"frame\",\"frame\"]", "[]"),
            VALID.replace("[\"frame\",\"frame\"]", "[\"missing\"]"),
            VALID.replace("\"moving\":false", "\"moving\":false,\"moving\":true"),
            VALID.replace(
                "\"animations\":{",
                "\"animations\":{\"idle\":{\"frames\":[\"frame\"]},",
            ),
        ] {
            assert!(
                expand(&source(&broken)).is_err(),
                "accepted invalid description: {broken}"
            );
        }
    }

    #[test]
    fn animator2d_description_first_matching_rule_holds_progress_and_binds_entity() {
        use engine_runtime::{
            animator2d::*,
            archetype::ComponentValue,
            components::{ComponentTypeId, Hierarchy, SpriteRenderer2D},
            ids::EntityId,
            world::World,
        };
        let text = VALID.replace("\"moving\":false","\"moving\":false,\"grounded\":true")
            .replace("\"when\":\"moving\",\"play\":\"walk\"","\"when\":\"moving && grounded\",\"play\":\"walk\"},{\"when\":\"moving\",\"play\":\"idle\"");
        let mut value: Value = serde_json::from_str(&text).unwrap();
        value["entity"] = json!("robot-entity");
        let mut input = source(&value.to_string());
        input.files.insert(
            "Scenes/main.scene.json".into(),
            br#"{"entities":[{"id":"robot-entity","components":[]}]}"#.to_vec(),
        );
        let expanded = expand(&input).unwrap();
        let scene: Value =
            serde_json::from_slice(expanded.bytes("Scenes/main.scene.json").unwrap()).unwrap();
        assert_eq!(
            scene["entities"][0]["components"][0]["data"]["controllerRef"],
            "robot"
        );
        let registry = super::super::cook_animator2d_registry(&expanded).unwrap();
        let mut world = World::new();
        let entity = EntityId::from("robot-entity");
        world
            .try_spawn_entity(
                entity.clone(),
                "Robot",
                "actor",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
            )
            .unwrap();
        world
            .try_insert_sprite_renderer2d(entity.clone(), SpriteRenderer2D::default())
            .unwrap();
        world
            .try_insert_component_value(
                entity.clone(),
                ComponentTypeId::animator2d(),
                ComponentValue::Animator2D(RuntimeAnimator2D {
                    controller_id: "robot".into(),
                    controller_index: 0,
                    registry_digest: registry.registry_digest.clone(),
                    enabled: true,
                    initial_bools: BTreeMap::new(),
                }),
            )
            .unwrap();
        let mut module = Animator2DModule::load(registry).unwrap();
        module.apply([Animator2DCommand::SetBool {
            entity_id: entity.clone(),
            parameter_id: "moving".into(),
            value: true,
        }]);
        for tick in 1..=7 {
            let report = module.tick(&mut world, tick, Animator2DReportLevel::Trace);
            assert_eq!(report.trace[0].state_id, "walk");
            assert_eq!(report.trace[0].frame_index, if tick == 7 { 1 } else { 0 });
        }
        module.apply([Animator2DCommand::SetBool {
            entity_id: entity,
            parameter_id: "grounded".into(),
            value: false,
        }]);
        assert_eq!(
            module
                .tick(&mut world, 8, Animator2DReportLevel::Trace)
                .trace[0]
                .state_id,
            "idle"
        );
        let mut twice = input.clone();
        twice.files.insert(
            "Animations/second.animator-description-2d.json".into(),
            value
                .to_string()
                .replace("\"assetId\":\"robot\"", "\"assetId\":\"second\"")
                .into_bytes(),
        );
        assert!(expand(&twice).is_err());
    }
}
