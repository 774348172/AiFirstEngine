//! v2 authoring is compiled alongside the existing Spine Cook, never through v1 sprite expansion.
// Keep the Compiler's existing structured, actionable error at this owner boundary.
#![allow(clippy::result_large_err)]
use super::*;
use engine_runtime::spine_animation::schema as c;
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spine374_d_optional_values_follow_nonnullable_author_schema() {
        let example: serde_json::Value = serde_json::from_str(include_str!(
            "../../engine_spine_runtime/contracts/example.animator-description-2d.json"
        ))
        .unwrap();
        assert!(serde_json::from_value::<Description>(example.clone()).is_ok());
        for path in [
            "/entity",
            "/layers/0/mask",
            "/animations/walk/fadeOutSeconds",
            "/animations/walk/onComplete",
        ] {
            let mut value = example.clone();
            match path {
                "/entity" => value["entity"] = serde_json::Value::Null,
                "/layers/0/mask" => value["layers"][0]["mask"] = serde_json::Value::Null,
                "/animations/walk/fadeOutSeconds" => {
                    value["animations"]["walk"]["fadeOutSeconds"] = serde_json::Value::Null;
                }
                _ => value["animations"]["walk"]["onComplete"] = serde_json::Value::Null,
            }
            assert!(
                serde_json::from_value::<Description>(value).is_err(),
                "author schema rejected null at {path}, but the owner accepted it"
            );
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Description {
    schema: String,
    asset_id: String,
    #[serde(default, deserialize_with = "optional_value")]
    entity: Option<String>,
    backend: String,
    skeleton: AssetRef,
    default: String,
    #[serde(default, deserialize_with = "unique")]
    parameters: BTreeMap<String, Parameter>,
    layers: Vec<Layer>,
    #[serde(default, deserialize_with = "unique")]
    masks: BTreeMap<String, Vec<String>>,
    #[serde(deserialize_with = "unique")]
    animations: BTreeMap<String, Action>,
    #[serde(default)]
    rules: Vec<Rule>,
    #[serde(default, deserialize_with = "unique")]
    sockets: BTreeMap<String, Socket>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetRef {
    asset_id: String,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Parameter {
    Bool { default: bool },
    Trigger {},
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    name: String,
    blend: String,
    #[serde(default, deserialize_with = "optional_value")]
    mask: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Action {
    clip: String,
    layer: String,
    playback: c::Playback,
    speed_permille: u16,
    mix_in_seconds: f64,
    #[serde(default, deserialize_with = "optional_value")]
    fade_out_seconds: Option<f64>,
    #[serde(default, deserialize_with = "optional_value")]
    on_complete: Option<c::Completion>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Condition {
    Bool(BoolCondition),
    Trigger(TriggerCondition),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoolCondition {
    parameter: String,
    equals: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TriggerCondition {
    triggered: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    all: Vec<Condition>,
    play: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Socket {
    bone: String,
    offset: [f32; 3],
}
// The frozen author schema allows absence, but excludes explicit null values.
fn optional_value<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
fn unique<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<BTreeMap<String, T>, D::Error> {
    struct V<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for V<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("unique object keys")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut a: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = BTreeMap::new();
            while let Some((k, v)) = a.next_entry::<String, T>()? {
                if out.insert(k, v).is_some() {
                    return Err(serde::de::Error::custom("duplicate name"));
                }
            }
            Ok(out)
        }
    }
    d.deserialize_map(V(std::marker::PhantomData))
}
pub(super) fn descriptions(source: &CompilerSourceView) -> impl Iterator<Item = (&str, &[u8])> {
    source
        .paths()
        .filter(|p| p.starts_with("Animations/") && p.ends_with(".animator-description-2d.json"))
        .filter_map(|p| {
            let b = source.bytes(p)?;
            let v: serde_json::Value = serde_json::from_slice(b).ok()?;
            (v["schema"] == "animator-description-2d.v2").then_some((p, b))
        })
}
pub(super) fn validate_headers(
    source: &CompilerSourceView,
) -> Result<(), GameProjectCompilerError> {
    for path in source
        .paths()
        .filter(|p| p.starts_with("Animations/") && p.ends_with(".animator-description-2d.json"))
    {
        let bytes = source.bytes(path).unwrap();
        let fail =
            |e: String| assembly_error("animator.description_invalid", format!("{path}: {e}"));
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| fail(e.to_string()))?;
        match value["schema"].as_str() {
            Some("animator-description-2d.v1") => {}
            Some("animator-description-2d.v2") => {
                if bytes.len() > 256 * 1024 {
                    return Err(fail("description capacity".into()));
                }
                serde_json::from_slice::<Description>(bytes).map_err(|e| fail(e.to_string()))?;
            }
            _ => return Err(fail("unsupported Animator description schema".into())),
        }
    }
    Ok(())
}
pub(super) fn is_v2(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .is_ok_and(|v| v["schema"] == "animator-description-2d.v2")
}
pub(crate) fn inspect(
    path: &str,
    bytes: &[u8],
    rate: u32,
) -> Result<Option<crate::spine_tooling::SpineControllerInspection>, String> {
    use crate::spine_tooling::*;
    if !is_v2(bytes) {
        return Ok(None);
    }
    let d: Description = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let actions = d
        .animations
        .into_iter()
        .map(|(name, a)| {
            Ok(SpineActionInspection {
                name,
                requested_mix_in_seconds: a.mix_in_seconds,
                mix_in_ticks: c::ticks(a.mix_in_seconds, rate)?,
                requested_fade_out_seconds: a.fade_out_seconds,
                fade_out_ticks: a.fade_out_seconds.map(|s| c::ticks(s, rate)).transpose()?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Some(SpineControllerInspection {
        source_path: path.into(),
        asset_id: d.asset_id,
        skeleton_asset_id: d.skeleton.asset_id,
        default_action: d.default,
        actions,
    }))
}
pub(super) fn append(
    source: &CompilerSourceView,
    input: &mut RuntimePackageBuildInput,
) -> Result<(), GameProjectCompilerError> {
    let mut bindings = Vec::new();
    for (path, bytes) in descriptions(source) {
        let fail =
            |m: String| assembly_error("animator.description_invalid", format!("{path}: {m}"));
        if bytes.len() > 256 * 1024 {
            return Err(fail("description capacity".into()));
        }
        let d: Description = serde_json::from_slice(bytes).map_err(|e| fail(e.to_string()))?;
        if d.schema != "animator-description-2d.v2"
            || d.backend != "spine"
            || !c::valid_name(&d.asset_id)
            || !c::valid_name(&d.skeleton.asset_id)
            || d.entity.as_ref().is_some_and(|n| !c::valid_name(n))
        {
            return Err(fail("invalid schema/backend/name".into()));
        }
        let spine = input
            .spine_animator
            .as_mut()
            .ok_or_else(|| fail("skeleton registry missing".into()))?;
        let registry = &mut spine.registry;
        let asset = registry
            .assets
            .iter()
            .find(|a| a.asset_id == d.skeleton.asset_id)
            .ok_or_else(|| fail("unknown skeleton".into()))?;
        let lookup = |v: &[String], name: &str| {
            v.iter()
                .position(|n| n == name)
                .ok_or_else(|| fail(format!("unknown binding: {name}")))
        };
        let parameter_names: Vec<_> = d.parameters.keys().cloned().collect();
        let layer_names: Vec<_> = d.layers.iter().map(|l| l.name.clone()).collect();
        let mask_names: Vec<_> = d.masks.keys().cloned().collect();
        let action_names: Vec<_> = d.animations.keys().cloned().collect();
        let mut layers = Vec::new();
        for l in &d.layers {
            if !matches!(l.blend.as_str(), "replace" | "additive") {
                return Err(fail("unknown layer blend".into()));
            }
            layers.push(c::Layer {
                name: l.name.clone(),
                additive: l.blend == "additive",
                mask_index: l
                    .mask
                    .as_ref()
                    .map(|m| lookup(&mask_names, m).map(|i| i as u8))
                    .transpose()?,
            });
        }
        let mut actions = Vec::new();
        for (name, a) in &d.animations {
            let layer = lookup(&layer_names, &a.layer)?;
            actions.push(c::Action {
                name: name.clone(),
                source_animation_index: lookup(&asset.animations, &a.clip)? as u16,
                layer_index: layer as u8,
                playback: a.playback,
                speed_permille: a.speed_permille,
                requested_mix_in_seconds: a.mix_in_seconds,
                mix_in_ticks: c::ticks(a.mix_in_seconds, registry.tick_rate).map_err(&fail)?,
                requested_fade_out_seconds: a.fade_out_seconds,
                fade_out_ticks: a
                    .fade_out_seconds
                    .map(|s| c::ticks(s, registry.tick_rate))
                    .transpose()
                    .map_err(&fail)?,
                on_complete: a.on_complete.unwrap_or(c::Completion::Hold),
                mask_index: layers[layer].mask_index,
            });
        }
        let mut rules = Vec::new();
        for r in &d.rules {
            let mut all = Vec::new();
            for condition in &r.all {
                let (name, value, trigger) = match condition {
                    Condition::Bool(v) => (&v.parameter, v.equals, false),
                    Condition::Trigger(v) => (&v.triggered, true, true),
                };
                if d.parameters
                    .get(name)
                    .is_none_or(|p| matches!(p, Parameter::Trigger {}) != trigger)
                {
                    return Err(fail("parameter kind mismatch".into()));
                }
                all.push((lookup(&parameter_names, name)? as u8, value));
            }
            rules.push(c::Rule {
                all,
                action_index: lookup(&action_names, &r.play)? as u16,
            });
        }
        let sockets = d
            .sockets
            .iter()
            .map(|(n, s)| {
                Ok(c::Socket {
                    name: n.clone(),
                    bone_index: lookup(&asset.bones, &s.bone)? as u16,
                    offset: s.offset,
                })
            })
            .collect::<Result<Vec<_>, GameProjectCompilerError>>()?;
        let cooked = c::CookedSpineController {
            asset_id: d.asset_id.clone(),
            skeleton_asset_id: d.skeleton.asset_id,
            default_action_index: lookup(&action_names, &d.default)? as u16,
            parameters: d
                .parameters
                .into_iter()
                .map(|(name, p)| match p {
                    Parameter::Bool { default } => c::Parameter::Bool { name, default },
                    Parameter::Trigger {} => c::Parameter::Trigger { name },
                })
                .collect(),
            layers,
            masks: d
                .masks
                .into_iter()
                .map(|(name, bones)| c::Mask { name, bones })
                .collect(),
            actions,
            rules,
            sockets,
        };
        cooked.validate(asset, registry.tick_rate).map_err(&fail)?;
        #[cfg(feature = "spine")]
        {
            let payload = input
                .assets
                .iter()
                .find(|a| a.asset_id == asset.asset_id)
                .and_then(|a| a.runtime_payload.as_deref())
                .ok_or_else(|| fail("missing skeleton payload".into()))?;
            let atlas = spine
                .payloads
                .get(&asset.atlas_path)
                .ok_or_else(|| fail("missing atlas payload".into()))?;
            let pages = asset
                .pages
                .iter()
                .map(|p| (p.name.clone(), p.texture_asset_id.clone()))
                .collect();
            let checked = engine_spine_runtime::import::validate(
                std::str::from_utf8(payload).map_err(|e| fail(e.to_string()))?,
                std::str::from_utf8(atlas).map_err(|e| fail(e.to_string()))?,
                &pages,
            )
            .map_err(|e| fail(e.message))?;
            cooked
                .compile(&checked, registry.tick_rate, asset.pixels_per_unit)
                .map_err(&fail)?;
        }
        registry.controllers.push(cooked);
        if let Some(entity) = d.entity {
            bindings.push((path.to_string(), entity, d.asset_id));
        }
    }
    let Some(spine) = input.spine_animator.as_mut() else {
        return Ok(());
    };
    spine
        .registry
        .controllers
        .sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    spine
        .registry
        .seal()
        .map_err(|e| assembly_error("animator.binding.invalid_registry", e))?;
    let mut bound = BTreeSet::new();
    for (path, entity, id) in bindings {
        if !bound.insert(entity.clone()) {
            return Err(assembly_error("animator.binding.duplicate_entity", path));
        }
        let matches = input
            .scenes
            .iter_mut()
            .flat_map(|s| s.entities.iter_mut())
            .filter(|e| e.id == entity)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(assembly_error("animator.binding.unknown_entity", path));
        }
        let e = matches.into_iter().next().unwrap();
        if e.components
            .iter()
            .any(|c| c.component_type == "SpineAnimator")
            || e.animator2d.is_some()
            || e.spatial.is_some()
        {
            return Err(assembly_error("animator.binding.entity_conflict", path));
        }
        e.components
            .push(engine_runtime::runtime_package::RuntimeProjectComponent {
                component_type: "SpineAnimator".into(),
                data: serde_json::to_value(engine_runtime::spine_animation::SpineAnimatorBinding {
                    controller_id: id.clone(),
                    controller_index: spine
                        .registry
                        .controllers
                        .iter()
                        .position(|c| c.asset_id == id)
                        .unwrap() as u16,
                    registry_digest: spine.registry.registry_digest.clone(),
                })
                .unwrap(),
            });
    }
    for e in input.scenes.iter().flat_map(|s| &s.entities) {
        if let Some(b) = engine_runtime::spine_animation::SpineAnimatorBinding::from_entity(e)
            .map_err(|e| assembly_error("animator.binding.entity_conflict", e))?
        {
            spine
                .registry
                .validate_binding(&b)
                .map_err(|e| assembly_error("animator.binding.registry_identity", e))?;
        }
    }
    Ok(())
}

#[cfg(feature = "spine")]
pub(super) fn check(
    source: &CompilerSourceView,
    declared: Vec<engine_spine_runtime::import::DeclaredSource>,
) -> Result<(), GameProjectCompilerError> {
    use engine_runtime::{canonical_digest::sha256_prefixed, spine_animation::*};
    if descriptions(source).next().is_none() {
        return Ok(());
    }
    // Reuse the same semantic compiler with leased metadata/payloads. No texture decode,
    // cache, package publication or second assembler is needed for an authoring check.
    let mut input =
        RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("check", "check", "1"));
    let mut registry = CookedSpineRegistry::empty(60);
    let mut payloads = BTreeMap::new();
    for d in declared {
        let payload = d.source.cooked_json().as_bytes().to_vec();
        let atlas = d.source.cooked_atlas().as_bytes().to_vec();
        let payload_path = format!("spine/{}/skeleton.json", d.asset_id);
        let atlas_path = format!("spine/{}/skeleton.atlas", d.asset_id);
        registry.assets.push(CookedSpineAsset {
            asset_id: d.asset_id.clone(),
            source_digest: sha256_prefixed(&payload),
            payload_path: payload_path.clone(),
            payload_digest: sha256_prefixed(&payload),
            atlas_path: atlas_path.clone(),
            atlas_digest: sha256_prefixed(&atlas),
            pages: d
                .source
                .pages
                .iter()
                .map(|p| SpinePage {
                    name: p.name.clone(),
                    texture_asset_id: p.texture_asset_id.clone(),
                    pma: p.pma,
                })
                .collect(),
            animations: d.source.animations.clone(),
            skins: d.source.skins.clone(),
            bones: d.source.bones.clone(),
            slots: d.source.slots.clone(),
            events: Vec::new(),
            limits: BTreeMap::new(),
            pixels_per_unit: d.pixels_per_unit,
        });
        let mut a = RuntimePackageSourceAsset::new(
            &d.asset_id,
            &d.asset_id,
            "spine-skeleton",
            "",
            payload_path,
        );
        a.runtime_payload = Some(payload);
        input.assets.push(a);
        payloads.insert(atlas_path, atlas);
    }
    input.spine_animator = Some(SpinePackageSource { registry, payloads });
    let prefabs = source
        .paths()
        .filter(|p| p.starts_with("Prefabs/") && p.ends_with(".prefab.json"))
        .map(|p| {
            let v = parse_json(p, source.bytes(p).unwrap())?;
            let id = v
                .get("prefabId")
                .or_else(|| v.get("id"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(p)
                .to_string();
            Ok((id, v))
        })
        .collect::<Result<BTreeMap<_, _>, GameProjectCompilerError>>()?;
    for path in source
        .paths()
        .filter(|p| p.starts_with("Scenes/") && p.ends_with(".scene.json"))
    {
        input.scenes.push(parse_scene(
            path,
            source.bytes(path).unwrap(),
            &prefabs,
            &[],
            &cook_animator2d_registry(source)?,
        )?);
    }
    append(source, &mut input)
}
