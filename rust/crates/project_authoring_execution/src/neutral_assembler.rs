use crate::game_project_compiler::{CompilerSourceView, GameProjectCompilerError};
use crate::{
    load_engine_builtin_font_pack, ProjectAssemblyArtifactCache, ProjectAssemblyProducerReport,
    ProjectFontCookModule,
};
use engine_runtime::animator2d::{
    Animator2DParameterKind, Animator2DPlayback, Animator2DTransitionTiming,
    CookedAnimator2DCondition, CookedAnimator2DParameter, CookedAnimator2DRegistry,
    CookedAnimator2DState, CookedAnimator2DTransition, CookedAnimatorController2D,
    CookedSpriteAnimationClip2D, CookedSpriteAnimationFrame2D, RuntimeAnimator2D,
};
use engine_runtime::aui::{
    AuiAssetRef, AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect, AuiStyle,
    AUI_DOCUMENT_SCHEMA_VERSION,
};
use engine_runtime::project_runtime_module::{
    project_runtime_aot_digest, ProjectRuntimeAotDigestSource, EMPTY_PROJECT_RUNTIME_AOT_DIGEST,
    EMPTY_PROJECT_RUNTIME_MODULE_ID,
};
use engine_runtime::runtime_package::RuntimeAuiManifest;
use engine_runtime::runtime_package::{
    CookedTextureAsset, RuntimeAssetRef, RuntimeEntity, RuntimeProjectComponent,
    RuntimeProjectInfo, RuntimeProjectModuleRef, RuntimeScene, RuntimeSpriteRenderer2D,
    RuntimeTransform, Vector3, RUNTIME_ENTITY_SCHEMA_VERSION, RUNTIME_SCENE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildInput, RuntimePackageSourceAsset, RuntimePackageSourceJson,
    RuntimePackageSourcePrefab, RuntimePackageSourceTexture,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

#[path = "animator_description.rs"]
mod animator_description;

pub(crate) fn assemble(
    source: &CompilerSourceView,
    artifact_cache_root: Option<&Path>,
) -> Result<
    (
        RuntimePackageBuildInput,
        String,
        Vec<ProjectAssemblyProducerReport>,
    ),
    GameProjectCompilerError,
> {
    let expanded = animator_description::expand(source)?;
    let source = &expanded;
    let manifest = source.project_manifest_value()?;
    let project_id = manifest
        .get("projectId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let project_name = manifest
        .get("projectName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&project_id)
        .to_string();
    let version = manifest
        .get("engineVersion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("0.1.0")
        .to_string();
    let module = manifest.get("runtimeModule");
    let module_id = module
        .and_then(|value| value.get("moduleId"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("engine.empty.runtime");
    let interface_version = module
        .and_then(|value| value.get("interfaceVersion"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("project-runtime-module.v2");
    let cargo_manifest = module
        .and_then(|value| value.get("cargoManifest"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let cargo_package = module
        .and_then(|value| value.get("cargoPackage"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let player_binary = module
        .and_then(|value| value.get("playerBinary"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let module_digest = if module_id == EMPTY_PROJECT_RUNTIME_MODULE_ID {
        EMPTY_PROJECT_RUNTIME_AOT_DIGEST.to_string()
    } else {
        let module_sources = source
            .paths()
            .filter(|path| {
                *path == cargo_manifest
                    || *path == "RuntimeModule/Cargo.lock"
                    || (path.starts_with("RuntimeModule/src/") && path.ends_with(".rs"))
            })
            .filter_map(|path| source.bytes(path).map(|bytes| (path, bytes)))
            .map(|(relative_path, bytes)| ProjectRuntimeAotDigestSource {
                relative_path,
                bytes,
            });
        project_runtime_aot_digest(
            module_id,
            interface_version,
            cargo_manifest,
            cargo_package,
            player_binary,
            module_sources,
        )
        .map_err(|error| assembly_error("runtime_module_digest_failed", error.to_string()))?
    };
    let project = RuntimeProjectInfo::new(
        project_id,
        project_name,
        version,
        RuntimeProjectModuleRef::new(module_id, interface_version, module_digest),
    );
    let mut input = RuntimePackageBuildInput::new(project);
    let cache = artifact_cache_root
        .map(ProjectAssemblyArtifactCache::open)
        .transpose()
        .map_err(|error| assembly_error("artifact_cache_open_failed", error.to_string()))?;
    let mut producer_reports = Vec::new();
    let animator2d_registry = cook_animator2d_registry(source)?;

    let mut prefab_documents = BTreeMap::new();
    for path in source
        .paths()
        .filter(|path| path.starts_with("Prefabs/") && path.ends_with(".prefab.json"))
    {
        let document = parse_json(path, source.bytes(path).unwrap())?;
        let prefab_id = document
            .get("prefabId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(path)
            .to_string();
        prefab_documents.insert(prefab_id.clone(), document.clone());
        input.prefabs.push(RuntimePackageSourcePrefab {
            prefab_id,
            document,
        });
    }

    for path in source.paths() {
        if path.starts_with("Scenes/") && path.ends_with(".scene.json") {
            let scene = parse_scene(
                path,
                source.bytes(path).unwrap(),
                &prefab_documents,
                &animator2d_registry,
            )?;
            input.assets.push(RuntimePackageSourceAsset::new(
                scene.id.clone(),
                scene.name.clone(),
                "scene",
                path,
                format!("scenes/{}.json", scene.id),
            ));
            input.scenes.push(scene);
        } else if path.starts_with("Prefabs/") && path.ends_with(".prefab.json") {
            continue;
        } else if path.starts_with("AUI/") && path.ends_with(".json") {
            let source_document = parse_json(path, source.bytes(path).unwrap())?;
            let (id, document) = cook_aui_document(path, source_document)?;
            input
                .aui_documents
                .push(RuntimePackageSourceJson { id, document });
        } else if path.starts_with("Input/") && path.ends_with(".json") {
            let document = parse_json(path, source.bytes(path).unwrap())?;
            let id = document
                .get("asset_id")
                .or_else(|| document.get("assetId"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(path)
                .to_string();
            input
                .input_mappings
                .push(RuntimePackageSourceJson { id, document });
        } else if path == "Rules/rule-manifest.json" {
            input.rule_manifest = Some(
                serde_json::from_slice(source.bytes(path).unwrap())
                    .map_err(|error| assembly_error("rule_manifest_invalid", error.to_string()))?,
            );
        } else if path.starts_with("Assets/") && path.ends_with(".asset") {
            append_asset(
                path,
                source,
                &mut input,
                cache.as_ref(),
                &mut producer_reports,
            )?;
        }
    }

    if let Some(observation_path) = manifest
        .get("observationContract")
        .and_then(serde_json::Value::as_str)
    {
        if let Some(bytes) = source.bytes(observation_path) {
            input.observation_contract = Some(serde_json::from_slice(bytes).map_err(|error| {
                assembly_error("observation_contract_invalid", error.to_string())
            })?);
        }
    }
    if !input.aui_documents.is_empty() {
        input.aui_manifest = Some(RuntimeAuiManifest {
            schema_version: "runtime-aui-manifest.v1".to_string(),
            documents: input
                .aui_documents
                .iter()
                .map(
                    |document| engine_runtime::runtime_package::RuntimeAuiManifestEntry {
                        document_id: document.id.clone(),
                        path: source
                            .paths()
                            .find(|path| {
                                path.starts_with("AUI/")
                                    && path.ends_with(".json")
                                    && source
                                        .bytes(path)
                                        .and_then(|bytes| {
                                            serde_json::from_slice::<serde_json::Value>(bytes).ok()
                                        })
                                        .and_then(|value| {
                                            value
                                                .get("documentId")
                                                .and_then(serde_json::Value::as_str)
                                                .map(|id| id == document.id)
                                        })
                                        .unwrap_or(false)
                            })
                            .unwrap_or("AUI/unknown.json")
                            .to_string(),
                        canvas_count: 1,
                        node_count: document
                            .document
                            .get("nodes")
                            .and_then(serde_json::Value::as_array)
                            .map(Vec::len)
                            .unwrap_or_default(),
                        binding_count: 0,
                        action_count: 0,
                        asset_refs: Vec::new(),
                    },
                )
                .collect(),
        });
    }
    if crate::font_cook::source_has_project_font_profile(source) {
        let (cook, report) = ProjectFontCookModule::cook_for_runtime_package_from_source_view(
            source,
            &input.aui_documents,
            cache.as_ref(),
            None,
        )
        .map_err(|failure| {
            let message = failure
                .diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                .unwrap_or_else(|| "Project font cook failed.".to_string());
            assembly_error("font_cook_failed", message)
        })?;
        if let Some(atlas) = cook.legacy_atlas {
            input.font_atlases.push(atlas);
        }
        if let Some(bundle) = cook.font_bundle {
            input.font_bundles.push(bundle);
        }
        producer_reports.push(report);
    } else {
        input.font_bundles.push(
            load_engine_builtin_font_pack()
                .map_err(|error| assembly_error("builtin_font_pack_invalid", error.to_string()))?,
        );
    }
    input.animator2d_registry = animator2d_registry;
    input
        .assets
        .extend(crate::particle_effect_cook::cook(source)?);
    let digest = input
        .assembly_input_digest()
        .map_err(|error| assembly_error("assembly_digest_failed", error.to_string()))?
        .0
        .prefixed_value();
    Ok((input, digest, producer_reports))
}

fn cook_animator2d_registry(
    source: &CompilerSourceView,
) -> Result<CookedAnimator2DRegistry, GameProjectCompilerError> {
    let mut clips = Vec::new();
    let mut controllers = Vec::new();
    for path in source
        .paths()
        .filter(|path| path.starts_with("Animations/") && path.ends_with(".json"))
    {
        let document = parse_json(path, source.bytes(path).unwrap())?;
        if path.ends_with(".sprite-animation-clip-2d.json") {
            let playback = match required_string(&document, "playback", path)? {
                "loop" => Animator2DPlayback::Loop,
                "once" => Animator2DPlayback::Once,
                value => {
                    return Err(assembly_error(
                        "animator2d.playback_invalid",
                        format!("{path}: unsupported playback '{value}'."),
                    ));
                }
            };
            let frames = document
                .get("frames")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| {
                    assembly_error(
                        "animator2d.clip_frames_invalid",
                        format!("{path}: frames must be an array."),
                    )
                })?
                .iter()
                .map(|frame| {
                    Ok(CookedSpriteAnimationFrame2D {
                        sprite_asset_id: required_string(frame, "spriteRef", path)?.to_string(),
                        duration_ticks: required_u32(frame, "durationTicks", path)?,
                    })
                })
                .collect::<Result<Vec<_>, GameProjectCompilerError>>()?;
            clips.push(CookedSpriteAnimationClip2D {
                id: required_string(&document, "assetId", path)?.to_string(),
                playback,
                frames,
            });
        } else if path.ends_with(".animator-controller-2d.json") {
            controllers.push((path.to_string(), document));
        }
    }
    clips.sort_by(|left, right| left.id.cmp(&right.id));
    let clip_indices = clips
        .iter()
        .enumerate()
        .map(|(index, clip)| (clip.id.clone(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let mut cooked_controllers = controllers
        .into_iter()
        .map(|(path, document)| cook_animator2d_controller(&path, document, &clip_indices))
        .collect::<Result<Vec<_>, GameProjectCompilerError>>()?;
    cooked_controllers.sort_by(|left, right| left.id.cmp(&right.id));
    CookedAnimator2DRegistry::from_parts(clips, cooked_controllers).map_err(|diagnostics| {
        let diagnostic = diagnostics.first();
        assembly_error(
            "animator2d.registry_invalid",
            diagnostic
                .map(|entry| format!("{}: {}", entry.code, entry.message))
                .unwrap_or_else(|| "Animator2D registry is invalid.".to_string()),
        )
    })
}

fn cook_animator2d_controller(
    path: &str,
    document: serde_json::Value,
    clip_indices: &BTreeMap<String, u32>,
) -> Result<CookedAnimatorController2D, GameProjectCompilerError> {
    let mut parameters = document
        .get("parameters")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    parameters.sort_by_key(|value| {
        value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let parameter_indices = parameters
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            value
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(|id| (id.to_string(), index as u32))
        })
        .collect::<BTreeMap<_, _>>();
    let parameters = parameters
        .iter()
        .map(|value| {
            let kind = match required_string(value, "kind", path)? {
                "bool" => Animator2DParameterKind::Bool,
                "trigger" => Animator2DParameterKind::Trigger,
                other => {
                    return Err(assembly_error(
                        "animator2d.parameter_kind_invalid",
                        format!("{path}: unsupported parameter kind '{other}'."),
                    ));
                }
            };
            Ok(CookedAnimator2DParameter {
                id: required_string(value, "id", path)?.to_string(),
                kind,
                default_bool: value
                    .get("defaultBool")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect::<Result<Vec<_>, GameProjectCompilerError>>()?;

    let mut states = document
        .get("states")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .ok_or_else(|| {
            assembly_error(
                "animator2d.states_invalid",
                format!("{path}: states must be an array."),
            )
        })?;
    states.sort_by_key(|value| {
        value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let state_indices = states
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            value
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(|id| (id.to_string(), index as u32))
        })
        .collect::<BTreeMap<_, _>>();
    let states = states
        .iter()
        .map(|value| {
            let clip_ref = required_string(value, "clipRef", path)?;
            let clip_index = clip_indices.get(clip_ref).copied().ok_or_else(|| {
                assembly_error(
                    "animator2d.clip_missing",
                    format!("{path}: missing clip '{clip_ref}'."),
                )
            })?;
            Ok(CookedAnimator2DState {
                id: required_string(value, "id", path)?.to_string(),
                clip_index,
                speed_permille: value
                    .get("speedPermille")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1000) as u32,
            })
        })
        .collect::<Result<Vec<_>, GameProjectCompilerError>>()?;

    let mut transitions = document
        .get("transitions")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    transitions.sort_by_key(|value| {
        value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let transitions = transitions
        .iter()
        .map(|value| {
            let from = required_string(value, "from", path)?;
            let to = required_string(value, "to", path)?;
            let timing = match required_string(value, "when", path)? {
                "immediate" => Animator2DTransitionTiming::Immediate,
                "clip_end" => Animator2DTransitionTiming::ClipEnd,
                other => {
                    return Err(assembly_error(
                        "animator2d.transition_timing_invalid",
                        format!("{path}: unsupported transition timing '{other}'."),
                    ));
                }
            };
            let conditions = value
                .get("conditions")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|condition| {
                    let parameter = required_string(condition, "parameter", path)?;
                    let parameter_index =
                        parameter_indices.get(parameter).copied().ok_or_else(|| {
                            assembly_error(
                                "animator2d.parameter_missing",
                                format!("{path}: missing parameter '{parameter}'."),
                            )
                        })?;
                    match required_string(condition, "op", path)? {
                        "triggered" => Ok(CookedAnimator2DCondition::Triggered { parameter_index }),
                        "equals" => Ok(CookedAnimator2DCondition::BoolEquals {
                            parameter_index,
                            value: condition
                                .get("value")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false),
                        }),
                        other => Err(assembly_error(
                            "animator2d.condition_invalid",
                            format!("{path}: unsupported condition '{other}'."),
                        )),
                    }
                })
                .collect::<Result<Vec<_>, GameProjectCompilerError>>()?;
            Ok(CookedAnimator2DTransition {
                id: required_string(value, "id", path)?.to_string(),
                from_state_index: state_indices.get(from).copied().ok_or_else(|| {
                    assembly_error(
                        "animator2d.transition_target_invalid",
                        format!("{path}: missing state '{from}'."),
                    )
                })?,
                to_state_index: state_indices.get(to).copied().ok_or_else(|| {
                    assembly_error(
                        "animator2d.transition_target_invalid",
                        format!("{path}: missing state '{to}'."),
                    )
                })?,
                timing,
                priority: value
                    .get("priority")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or_default() as i32,
                conditions,
            })
        })
        .collect::<Result<Vec<_>, GameProjectCompilerError>>()?;

    let entry_state = required_string(&document, "entryStateId", path)?;
    Ok(CookedAnimatorController2D {
        id: required_string(&document, "assetId", path)?.to_string(),
        entry_state_index: state_indices.get(entry_state).copied().ok_or_else(|| {
            assembly_error(
                "animator2d.entry_state_invalid",
                format!("{path}: missing entry state '{entry_state}'."),
            )
        })?,
        parameters,
        states,
        transitions,
    })
}

fn required_string<'a>(
    value: &'a serde_json::Value,
    field: &str,
    path: &str,
) -> Result<&'a str, GameProjectCompilerError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            assembly_error(
                "animator2d.field_missing",
                format!("{path}: missing string field '{field}'."),
            )
        })
}

fn required_u32(
    value: &serde_json::Value,
    field: &str,
    path: &str,
) -> Result<u32, GameProjectCompilerError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            assembly_error(
                "animator2d.field_invalid",
                format!("{path}: '{field}' must be a positive u32."),
            )
        })
}

fn optional_sprite_color(
    value: &serde_json::Value,
) -> Result<Option<[f32; 4]>, GameProjectCompilerError> {
    let Some(raw) = value.get("color") else {
        return Ok(None);
    };
    let Some(raw) = raw.as_str() else {
        return Err(assembly_error(
            "sprite_renderer2d.color_invalid",
            "SpriteRenderer2D color must be a #RRGGBB or #RRGGBBAA string.",
        ));
    };
    let digits = raw.strip_prefix('#').ok_or_else(|| {
        assembly_error(
            "sprite_renderer2d.color_invalid",
            "SpriteRenderer2D color must start with '#'.",
        )
    })?;
    if digits.len() != 6 && digits.len() != 8 {
        return Err(assembly_error(
            "sprite_renderer2d.color_invalid",
            "SpriteRenderer2D color must contain 6 or 8 hexadecimal digits.",
        ));
    }
    let mut rgba = [255u8; 4];
    for (index, slot) in rgba.iter_mut().enumerate() {
        if index == 3 && digits.len() == 6 {
            break;
        }
        *slot = u8::from_str_radix(&digits[index * 2..index * 2 + 2], 16).map_err(|_| {
            assembly_error(
                "sprite_renderer2d.color_invalid",
                "SpriteRenderer2D color contains non-hexadecimal digits.",
            )
        })?;
    }
    Ok(Some(rgba.map(|channel| f32::from(channel) / 255.0)))
}

fn parse_scene(
    path: &str,
    bytes: &[u8],
    prefabs: &BTreeMap<String, serde_json::Value>,
    animator2d_registry: &CookedAnimator2DRegistry,
) -> Result<RuntimeScene, GameProjectCompilerError> {
    let document = parse_json(path, bytes)?;
    let source_entities = document
        .get("entities")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            assembly_error("scene_entities_missing", "Scene entities must be an array.")
        })?;
    let mut entities = Vec::new();
    for entity in source_entities {
        if let Some(instance) = prefab_instance(entity) {
            entities.extend(bake_prefab_instance(
                entity,
                instance,
                prefabs,
                animator2d_registry,
            )?);
        } else {
            entities.push(parse_entity(entity, animator2d_registry)?);
        }
    }
    Ok(RuntimeScene {
        schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
        id: document
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(path)
            .to_string(),
        name: document
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(path)
            .to_string(),
        gravity: number(document.get("gravity")),
        background: document
            .get("background")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("#000000")
            .to_string(),
        sky_color: document
            .get("skyColor")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("#000000")
            .to_string(),
        entities,
    })
}

fn prefab_instance(entity: &serde_json::Value) -> Option<&serde_json::Value> {
    entity
        .get("components")?
        .as_array()?
        .iter()
        .find(|component| {
            component
                .get("componentType")
                .and_then(serde_json::Value::as_str)
                == Some("engine.prefab_instance")
        })
        .and_then(|component| component.get("data"))
}

fn bake_prefab_instance(
    scene_entity: &serde_json::Value,
    instance: &serde_json::Value,
    prefabs: &BTreeMap<String, serde_json::Value>,
    animator2d_registry: &CookedAnimator2DRegistry,
) -> Result<Vec<RuntimeEntity>, GameProjectCompilerError> {
    let prefab_id = instance
        .get("source")
        .and_then(|source| source.get("id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            assembly_error(
                "prefab_reference_invalid",
                "Prefab instance source.id is missing.",
            )
        })?;
    let prefab = prefabs.get(prefab_id).ok_or_else(|| {
        assembly_error(
            "prefab_missing",
            format!("Prefab instance references missing prefab '{prefab_id}'."),
        )
    })?;
    let root_source_id = prefab
        .get("rootEntityId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            assembly_error(
                "prefab_root_missing",
                format!("Prefab '{prefab_id}' has no rootEntityId."),
            )
        })?;
    let scene_entity_id = scene_entity
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            assembly_error(
                "prefab_scene_entity_id_missing",
                "Prefab scene entity has no id.",
            )
        })?;
    let prefab_entities = prefab
        .get("entities")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            assembly_error(
                "prefab_entities_missing",
                format!("Prefab '{prefab_id}' entities must be an array."),
            )
        })?;
    let overrides = instance
        .get("overrides")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut baked = Vec::new();
    for source_entity in prefab_entities {
        let source_id = source_entity
            .get("sourceEntityId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                assembly_error(
                    "prefab_entity_id_missing",
                    format!("Prefab '{prefab_id}' contains an entity without sourceEntityId."),
                )
            })?;
        let mut value = source_entity.clone();
        let object = value.as_object_mut().ok_or_else(|| {
            assembly_error("prefab_entity_invalid", "Prefab entity must be an object.")
        })?;
        object.insert(
            "id".to_string(),
            serde_json::Value::String(prefab_runtime_id(
                scene_entity_id,
                root_source_id,
                source_id,
            )),
        );
        object.insert(
            "kind".to_string(),
            serde_json::Value::String("prefab".to_string()),
        );
        if source_id == root_source_id {
            for field in ["transform", "name", "siblingOrder", "parentId", "enabled"] {
                if let Some(source_value) = scene_entity.get(field) {
                    object.insert(field.to_string(), source_value.clone());
                }
            }
        } else if let Some(parent_source) = source_entity
            .get("parentSourceEntityId")
            .and_then(serde_json::Value::as_str)
        {
            object.insert(
                "parentId".to_string(),
                serde_json::Value::String(prefab_runtime_id(
                    scene_entity_id,
                    root_source_id,
                    parent_source,
                )),
            );
        }
        apply_prefab_overrides(&mut value, source_id, overrides);
        baked.push(parse_entity(&value, animator2d_registry)?);
    }
    Ok(baked)
}

fn prefab_runtime_id(scene_entity_id: &str, root_source_id: &str, source_id: &str) -> String {
    if source_id == root_source_id {
        scene_entity_id.to_string()
    } else {
        format!("{scene_entity_id}__{source_id}")
    }
}

fn apply_prefab_overrides(
    entity: &mut serde_json::Value,
    source_id: &str,
    overrides: &[serde_json::Value],
) {
    for value in overrides.iter().filter(|value| {
        value
            .get("targetSourceEntityId")
            .and_then(serde_json::Value::as_str)
            == Some(source_id)
    }) {
        let Some(component_type) = value
            .get("componentType")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(field_path) = value.get("fieldPath").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(replacement) = value.get("value") else {
            continue;
        };
        let Some(component) = entity
            .get_mut("components")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|components| {
                components.iter_mut().find(|component| {
                    component
                        .get("componentType")
                        .and_then(serde_json::Value::as_str)
                        == Some(component_type)
                })
            })
        else {
            continue;
        };
        if let Some(data) = component.get_mut("data") {
            set_json_path(data, field_path, replacement.clone());
        }
    }
}

fn set_json_path(target: &mut serde_json::Value, path: &str, replacement: serde_json::Value) {
    let mut segments = path.split('.').peekable();
    let mut current = target;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            if let Some(object) = current.as_object_mut() {
                object.insert(segment.to_string(), replacement);
            }
            return;
        }
        let Some(next) = current.get_mut(segment) else {
            return;
        };
        current = next;
    }
}

fn parse_entity(
    value: &serde_json::Value,
    animator2d_registry: &CookedAnimator2DRegistry,
) -> Result<RuntimeEntity, GameProjectCompilerError> {
    let transform = value.get("transform").map(parse_transform).transpose()?;
    let mut sprite = None;
    let mut animator2d = None;
    let mut components = Vec::new();
    if let Some(items) = value
        .get("components")
        .and_then(serde_json::Value::as_array)
    {
        for component in items {
            let component_type = component
                .get("componentType")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let data = component
                .get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if component_type.eq_ignore_ascii_case("spriterenderer2d") {
                let sprite_ref = data.get("spriteRef").and_then(parse_asset_ref);
                sprite = Some(RuntimeSpriteRenderer2D {
                    sprite_ref,
                    material_ref: data.get("materialRef").and_then(parse_asset_ref),
                    color: optional_sprite_color(&data)?,
                    flip_x: data.get("flipX").and_then(serde_json::Value::as_bool),
                    flip_y: data.get("flipY").and_then(serde_json::Value::as_bool),
                    sorting_layer: data
                        .get("sortingLayer")
                        .and_then(serde_json::Value::as_i64)
                        .map(|v| v as i16),
                    order_in_layer: data
                        .get("orderInLayer")
                        .and_then(serde_json::Value::as_i64)
                        .map(|v| v as i32),
                    sort_z: data
                        .get("sortZ")
                        .and_then(serde_json::Value::as_f64)
                        .map(|v| v as f32),
                    visible: data.get("visible").and_then(serde_json::Value::as_bool),
                });
            } else if component_type.eq_ignore_ascii_case("animator2d") {
                let controller_id = required_string(&data, "controllerRef", "Scene Animator2D")?;
                let controller_index = animator2d_registry
                    .controller_index(controller_id)
                    .ok_or_else(|| {
                        assembly_error(
                            "animator2d.controller_missing",
                            format!("Animator2D controller is not available: {controller_id}."),
                        )
                    })?;
                let initial_bools = data
                    .get("initialBools")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|error| {
                        assembly_error("animator2d.initial_bools_invalid", error.to_string())
                    })?
                    .unwrap_or_default();
                animator2d = Some(RuntimeAnimator2D {
                    controller_id: controller_id.to_string(),
                    controller_index,
                    registry_digest: animator2d_registry.registry_digest.clone(),
                    enabled: data
                        .get("enabled")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true),
                    initial_bools,
                });
            } else {
                components.push(RuntimeProjectComponent {
                    component_type: component_type.to_string(),
                    data,
                });
            }
        }
    }
    Ok(RuntimeEntity {
        schema_version: RUNTIME_ENTITY_SCHEMA_VERSION.to_string(),
        id: value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        kind: value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("entity")
            .to_string(),
        enabled: value
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        parent_id: value
            .get("parentId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        sibling_order: value
            .get("siblingOrder")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
        transform,
        mesh: None,
        sprite_renderer2d: sprite,
        animator2d,
        components,
    })
}

fn parse_transform(
    value: &serde_json::Value,
) -> Result<RuntimeTransform, GameProjectCompilerError> {
    Ok(RuntimeTransform {
        local_position: vector(value.get("localPosition")),
        local_rotation: vector(value.get("localRotation")),
        local_scale: vector(value.get("localScale")),
    })
}

fn vector(value: Option<&serde_json::Value>) -> Vector3 {
    Vector3 {
        x: number(value.and_then(|value| value.get("x"))),
        y: number(value.and_then(|value| value.get("y"))),
        z: number(value.and_then(|value| value.get("z"))),
    }
}

fn number(value: Option<&serde_json::Value>) -> f32 {
    value.and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32
}

fn parse_asset_ref(value: &serde_json::Value) -> Option<RuntimeAssetRef> {
    Some(RuntimeAssetRef {
        id: value.get("id")?.as_str()?.to_string(),
        asset_type: value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("asset")
            .to_string(),
        guid: value
            .get("guid")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        sub_asset: value
            .get("subAsset")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    })
}

fn append_asset(
    path: &str,
    source: &CompilerSourceView,
    input: &mut RuntimePackageBuildInput,
    cache: Option<&ProjectAssemblyArtifactCache>,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
) -> Result<(), GameProjectCompilerError> {
    let document = parse_json(path, source.bytes(path).unwrap())?;
    let asset_id = document
        .get("assetId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(path)
        .to_string();
    let asset_type = document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("asset")
        .strip_suffix("-asset.v1")
        .unwrap_or("asset")
        .to_string();
    let source_path = document
        .get("sourceImage")
        .and_then(serde_json::Value::as_str);
    let runtime_uri = format!("cooked/{asset_id}.asset");
    let mut asset = RuntimePackageSourceAsset::new(
        asset_id.clone(),
        asset_id.clone(),
        asset_type.clone(),
        path,
        runtime_uri,
    );
    asset.asset_guid = document
        .get("assetGuid")
        .or_else(|| document.get("asset_guid"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    asset.name = document
        .get("displayName")
        .or_else(|| document.get("display_name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&asset_id)
        .to_string();
    if asset_type == "audio" {
        if document.get("sourceImage").is_some() {
            return Err(assembly_error(
                "audio_source_image_unsupported",
                format!("{path}: audio assets cannot contain sourceImage; use sourceAudio."),
            ));
        }
        let audio_path = document
            .get("sourceAudio")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                assembly_error(
                    "audio.source_missing",
                    format!("{path}: sourceAudio is required."),
                )
            })?;
        crate::ProjectRelativePath::parse(audio_path).map_err(|error| {
            assembly_error("audio.source_invalid", format!("{path}: {error:?}"))
        })?;
        let bytes = source.bytes(audio_path).ok_or_else(|| {
            assembly_error(
                "audio.source_missing",
                format!("{path}: audio source {audio_path} is missing from the snapshot."),
            )
        })?;
        engine_runtime::audio::decode_audio_wav(bytes).map_err(|message| {
            assembly_error(
                "audio.decode_failed",
                format!("{path} -> {audio_path}: {message}"),
            )
        })?;
        asset.runtime_uri = format!("cooked/audio/{asset_id}.wav");
        asset.runtime_payload = Some(bytes.to_vec());
        asset.hash = Some(format!("sha256:{:x}", Sha256::digest(bytes)));
    } else if matches!(asset_type.as_str(), "mesh" | "material")
        && document.get("runtimeData").is_some()
    {
        // Ordinary asset metadata stays outside its typed runtime bytes.
        let data = document["runtimeData"].clone();
        let invalid = |message| {
            assembly_error(
                "particle_resource.invalid",
                format!("{path}: runtimeData: {message}"),
            )
        };
        if asset_type == "mesh" {
            let mesh: engine_runtime::runtime_particles::ParticleMeshData =
                serde_json::from_value(data.clone()).map_err(|e| invalid(e.to_string()))?;
            mesh.validate().map_err(&invalid)?;
        } else {
            let material: engine_runtime::runtime_particles::ParticleMaterialData =
                serde_json::from_value(data.clone()).map_err(|e| invalid(e.to_string()))?;
            material.validate().map_err(&invalid)?;
            if let Some(reference) = material.texture {
                let resolved = source
                    .files
                    .iter()
                    .filter(|(p, _)| p.ends_with(".asset"))
                    .filter_map(|(_, bytes)| {
                        serde_json::from_slice::<serde_json::Value>(bytes).ok()
                    })
                    .filter(|v| v["assetId"].as_str() == Some(reference.id.as_str()))
                    .collect::<Vec<_>>();
                if resolved.len() != 1 {
                    return Err(invalid(format!(
                        "texture id must resolve uniquely: {}",
                        reference.id
                    )));
                }
                let texture = &resolved[0];
                let guid = texture
                    .get("assetGuid")
                    .or_else(|| texture.get("asset_guid"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&reference.id);
                if texture["schemaVersion"].as_str() != Some("texture-asset.v1")
                    || reference.guid.as_deref().is_some_and(|g| g != guid)
                {
                    return Err(invalid(format!(
                        "texture id/guid/type mismatch: {}",
                        reference.id
                    )));
                }
                asset.dependencies.push(guid.to_string());
            }
        }
        let bytes = serde_json::to_vec(&data).map_err(|e| invalid(e.to_string()))?;
        asset.hash = Some(format!("sha256:{:x}", Sha256::digest(&bytes)));
        asset.runtime_payload = Some(bytes);
    } else if asset_type != "texture" {
        asset.runtime_payload = source.bytes(path).map(<[u8]>::to_vec);
        asset.hash = source
            .bytes(path)
            .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)));
    }
    if let Some(image_path) = source_path {
        if let Some(bytes) = source.bytes(image_path) {
            asset.hash = Some(format!("sha256:{:x}", Sha256::digest(bytes)));
            if image_path.ends_with(".png") {
                let texture = cached_texture(
                    &asset_id,
                    image_path,
                    bytes,
                    document.get("importer"),
                    cache,
                    reports,
                )?;
                input.texture_payloads.push(texture);
            }
        }
    }
    input.assets.push(asset);
    Ok(())
}

fn cached_texture(
    asset_id: &str,
    source_path: &str,
    bytes: &[u8],
    importer: Option<&serde_json::Value>,
    cache: Option<&ProjectAssemblyArtifactCache>,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
) -> Result<RuntimePackageSourceTexture, GameProjectCompilerError> {
    use crate::ProjectAssemblyArtifactCacheStatus;
    use engine_runtime::canonical_digest::sha256_prefixed;
    const RECIPE: &str = "texture-png.v1";
    let dependency = sha256_prefixed(
        &serde_json::to_vec(&(asset_id, sha256_prefixed(bytes), importer)).unwrap(),
    );
    let key = sha256_prefixed(format!("{RECIPE}|{dependency}").as_bytes());
    let mut report = ProjectAssemblyProducerReport::uncached("texture-cook", 0);
    report.producer_recipe_version = RECIPE.into();
    report.recipe_key = Some(key.clone());
    let started = std::time::Instant::now();
    if let Some(cache) = cache {
        let lookup =
            cache.lookup_json::<(CookedTextureAsset, Vec<u8>)>("texture-cook", &key, RECIPE);
        report.cache_status = lookup.status;
        report.miss_reason = lookup.reason;
        report.artifact_path = lookup.artifact_path.map(|path| path.display().to_string());
        if let (Some((metadata, rgba8)), Some(envelope)) = (lookup.artifact, lookup.envelope) {
            let output = sha256_prefixed(&serde_json::to_vec(&(&metadata, &rgba8)).unwrap());
            if envelope.dependency_digest == dependency && envelope.output_digest == output {
                report.output_digest = Some(output);
                report.duration_ms = started.elapsed().as_millis() as u64;
                reports.push(report);
                return Ok(RuntimePackageSourceTexture { metadata, rgba8 });
            }
            cache
                .quarantine("texture-cook", &key)
                .map_err(|error| assembly_error("texture_cache_invalid", error.to_string()))?;
            report.cache_status = ProjectAssemblyArtifactCacheStatus::Invalid;
            report.miss_reason = Some("dependency_or_output_digest_mismatch".into());
        }
    }
    let texture = cook_texture(asset_id, source_path, bytes, importer).ok_or_else(|| {
        assembly_error(
            "texture_decode_failed",
            format!("Cannot decode {source_path}"),
        )
    })?;
    let payload = (&texture.metadata, &texture.rgba8);
    let output = sha256_prefixed(&serde_json::to_vec(&payload).unwrap());
    report.output_digest = Some(output.clone());
    if let Some(cache) = cache {
        cache
            .publish_json("texture-cook", RECIPE, &key, &dependency, &output, &payload)
            .map_err(|error| assembly_error("texture_cache_publish_failed", error.to_string()))?;
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    reports.push(report);
    Ok(texture)
}

fn cook_texture(
    asset_id: &str,
    _source_path: &str,
    bytes: &[u8],
    importer: Option<&serde_json::Value>,
) -> Option<RuntimePackageSourceTexture> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    let decoded = &buffer[..info.buffer_size()];
    let rgba8 = match info.color_type {
        png::ColorType::Rgba => decoded.to_vec(),
        png::ColorType::Rgb => decoded
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::Grayscale => decoded
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => decoded
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        _ => return None,
    };
    let importer = importer.cloned().unwrap_or_default();
    let metadata = CookedTextureAsset {
        schema_version: "cooked-texture.v1".to_string(),
        asset_id: asset_id.to_string(),
        cooked_asset_id: format!("{asset_id}.cooked"),
        source_hash: format!("sha256:{:x}", Sha256::digest(bytes)),
        width: info.width,
        height: info.height,
        format: if importer
            .get("colorSpace")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("srgb")
            .eq_ignore_ascii_case("srgb")
        {
            "rgba8UnormSrgb".to_string()
        } else {
            "rgba8Unorm".to_string()
        },
        color_space: importer
            .get("colorSpace")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("srgb")
            .to_string(),
        mip_count: 1,
        byte_length: rgba8.len(),
        pixel_data_path: format!("cooked/textures/{asset_id}.rgba8"),
        sampler: importer
            .get("sampler")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("linearClamp")
            .to_string(),
    };
    Some(RuntimePackageSourceTexture { metadata, rgba8 })
}

fn parse_json(path: &str, bytes: &[u8]) -> Result<serde_json::Value, GameProjectCompilerError> {
    serde_json::from_slice(bytes)
        .map_err(|error| assembly_error("source_json_invalid", format!("{path}: {error}")))
}

fn cook_aui_document(
    path: &str,
    source: serde_json::Value,
) -> Result<(String, serde_json::Value), GameProjectCompilerError> {
    if let Ok(mut document) = serde_json::from_value::<AuiDocument>(source.clone()) {
        document.schema_version = AUI_DOCUMENT_SCHEMA_VERSION.to_string();
        let id = document.document_id.clone();
        let value = serde_json::to_value(document)
            .map_err(|error| assembly_error("aui_document_cook_failed", error.to_string()))?;
        return Ok((id, value));
    }
    let document_id = source
        .get("documentId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            assembly_error(
                "aui_document_id_missing",
                format!("{path}: AUI documentId is missing."),
            )
        })?
        .to_string();
    let root = source.get("root").ok_or_else(|| {
        assembly_error(
            "aui_document_root_missing",
            format!("{path}: AUI root is missing."),
        )
    })?;
    let root_id = root
        .get("nodeId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("root")
        .to_string();
    let mut nodes = Vec::new();
    cook_aui_node(root, None, &mut nodes);
    let document = AuiDocument::new(
        document_id.clone(),
        vec![AuiCanvas::screen_overlay(
            format!("{document_id}.canvas"),
            1280.0,
            720.0,
            root_id,
        )],
        nodes,
    );
    let value = serde_json::to_value(document)
        .map_err(|error| assembly_error("aui_document_cook_failed", error.to_string()))?;
    Ok((document_id, value))
}

fn cook_aui_node(source: &serde_json::Value, parent: Option<&str>, output: &mut Vec<AuiNode>) {
    let node_id = source
        .get("nodeId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("node")
        .to_string();
    let node_type = source
        .get("nodeType")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("panel");
    let kind = match node_type {
        "image" | "image-row" => AuiNodeKind::Image,
        "text" => AuiNodeKind::Text,
        "button" => AuiNodeKind::Button,
        "progress-bar" => AuiNodeKind::ProgressBar,
        "toggle" => AuiNodeKind::Toggle,
        "slider" => AuiNodeKind::Slider,
        "list" => AuiNodeKind::List,
        "scroll-view" => AuiNodeKind::ScrollView,
        "input-field" => AuiNodeKind::InputField,
        _ => AuiNodeKind::Panel,
    };
    let child_ids = source
        .get("children")
        .and_then(serde_json::Value::as_array)
        .map(|children| {
            children
                .iter()
                .filter_map(|child| child.get("nodeId").and_then(serde_json::Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut node = AuiNode::new(node_id.clone(), kind, legacy_aui_rect(source, node_type));
    node.visible = source
        .get("visible")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    node.style = Some(AuiStyle {
        color: source
            .get("color")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| (node_type == "canvas").then(|| "#00000000".to_string())),
        text_color: source
            .get("textColor")
            .or_else(|| source.get("text_color"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| (kind == AuiNodeKind::Text).then(|| "#ffffff".to_string())),
        font_size: source
            .get("fontSize")
            .or_else(|| source.get("font_size"))
            .and_then(serde_json::Value::as_f64)
            .map(|v| v as f32)
            .or_else(|| (kind == AuiNodeKind::Text).then_some(24.0)),
        font: None,
    });
    node.parent = parent.map(str::to_string);
    node.children = child_ids;
    node.name = source
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&node_id)
        .to_string();
    node.text = source
        .get("text")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    node.progress_value = source
        .get("value")
        .and_then(serde_json::Value::as_f64)
        .map(|value| value as f32);
    node.image = source
        .get("imageRef")
        .and_then(|value| value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(AuiAssetRef::new);
    output.push(node);
    if let Some(children) = source.get("children").and_then(serde_json::Value::as_array) {
        for child in children {
            cook_aui_node(child, Some(&node_id), output);
        }
    }
}

// Preserve legacy presentation defaults, but leave all gameplay bindings in project data.
fn legacy_aui_rect(source: &serde_json::Value, node_type: &str) -> AuiRect {
    if let Some(rect) = source.get("rect") {
        let number = |key, fallback| {
            rect.get(key)
                .and_then(serde_json::Value::as_f64)
                .map(|v| v as f32)
                .unwrap_or(fallback)
        };
        return AuiRect::fixed_position(
            number("x", 0.0),
            number("y", 0.0),
            number("width", 240.0),
            number("height", 48.0),
        );
    }
    let anchor = source.get("anchor").and_then(serde_json::Value::as_str);
    match node_type {
        "canvas" => AuiRect::stretch_full(),
        "text" => match anchor {
            Some("top-right") => AuiRect::fixed_position(980.0, 24.0, 260.0, 36.0),
            Some("bottom-left") => AuiRect::fixed_position(24.0, 650.0, 260.0, 36.0),
            Some("bottom-right") => AuiRect::fixed_position(980.0, 650.0, 260.0, 36.0),
            _ => AuiRect::fixed_position(24.0, 24.0, 280.0, 36.0),
        },
        "image" | "image-row" => match anchor {
            Some("top-right") => AuiRect::fixed_position(1112.0, 24.0, 120.0, 32.0),
            Some("bottom-left") => AuiRect::fixed_position(24.0, 640.0, 120.0, 32.0),
            Some("bottom-right") => AuiRect::fixed_position(1112.0, 640.0, 120.0, 32.0),
            _ => AuiRect::fixed_position(24.0, 72.0, 120.0, 32.0),
        },
        "progress-bar" => AuiRect::fixed_position(24.0, 66.0, 220.0, 18.0),
        _ => AuiRect::fixed_position(0.0, 0.0, 240.0, 48.0),
    }
}

fn assembly_error(code: &'static str, message: impl Into<String>) -> GameProjectCompilerError {
    GameProjectCompilerError::new_for_assembly(code, message.into())
}

#[cfg(test)]
mod particle_resource_tests {
    use super::*;
    #[test]
    fn particle_resource_author_metadata_cooks_real_typed_bytes_and_dependencies() {
        let mut snapshot = CompilerSourceView { files: BTreeMap::from([
            ("Assets/m.asset".into(), br#"{"schemaVersion":"material-asset.v1","assetId":"m","assetGuid":"gm","runtimeData":{"baseColor":[1,0.5,0.25,1],"texture":{"id":"t","type":"texture","guid":"gt"}}}"#.to_vec()),
            ("Assets/t.asset".into(), br#"{"schemaVersion":"texture-asset.v1","assetId":"t","assetGuid":"gt"}"#.to_vec()),
            ("Assets/mesh.asset".into(), br#"{"schemaVersion":"mesh-asset.v1","assetId":"mesh","runtimeData":{"positions":[[0,0,0],[1,0,0],[0,1,0]],"uvs":[[0,0],[1,0],[0,1]],"indices":[0,1,2]}}"#.to_vec()),
        ]) };
        let cook = |snapshot: &CompilerSourceView, path| {
            let mut input =
                RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty("p", "P", "1"));
            append_asset(path, snapshot, &mut input, None, &mut Vec::new()).map(|_| input)
        };
        let first = cook(&snapshot, "Assets/m.asset").unwrap();
        let material: engine_runtime::runtime_particles::ParticleMaterialData =
            serde_json::from_slice(first.assets[0].runtime_payload.as_ref().unwrap()).unwrap();
        assert_eq!(material.base_color, [1.0, 0.5, 0.25, 1.0]);
        assert_eq!(first.assets[0].dependencies, vec!["gt"]);
        let mesh_input = cook(&snapshot, "Assets/mesh.asset").unwrap();
        let mesh: engine_runtime::runtime_particles::ParticleMeshData =
            serde_json::from_slice(mesh_input.assets[0].runtime_payload.as_ref().unwrap()).unwrap();
        assert_eq!(mesh.indices, vec![0, 1, 2]);
        let mut changed: serde_json::Value =
            serde_json::from_slice(&snapshot.files["Assets/m.asset"]).unwrap();
        changed["runtimeData"]["baseColor"][0] = serde_json::json!(0.25);
        snapshot.files.insert(
            "Assets/m.asset".into(),
            serde_json::to_vec(&changed).unwrap(),
        );
        assert_ne!(
            first.assembly_input_digest().unwrap(),
            cook(&snapshot, "Assets/m.asset")
                .unwrap()
                .assembly_input_digest()
                .unwrap()
        );
        changed["runtimeData"]["texture"]["guid"] = serde_json::json!("wrong");
        snapshot.files.insert(
            "Assets/m.asset".into(),
            serde_json::to_vec(&changed).unwrap(),
        );
        assert!(cook(&snapshot, "Assets/m.asset")
            .unwrap_err()
            .message()
            .contains("mismatch"));
        changed["runtimeData"] = serde_json::json!({"baseColor":[1,1,1,1],"shader":"custom"});
        snapshot.files.insert(
            "Assets/m.asset".into(),
            serde_json::to_vec(&changed).unwrap(),
        );
        assert!(cook(&snapshot, "Assets/m.asset")
            .unwrap_err()
            .message()
            .contains("unknown field"));
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;

    fn wav() -> Vec<u8> {
        b"RIFF\x2c\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x08\0\0\0\0\x80\0\0\0\x40\xff\x7f".to_vec()
    }

    fn source() -> CompilerSourceView {
        CompilerSourceView { files: BTreeMap::from([
            ("Assets/clip.asset".into(), br#"{"schemaVersion":"audio-asset.v1","assetId":"clip","assetGuid":"guid-clip","sourceAudio":"Assets/Audio/clip.wav"}"#.to_vec()),
            ("Assets/Audio/clip.wav".into(), wav()),
        ]) }
    }

    fn cook(
        source: &CompilerSourceView,
    ) -> Result<RuntimePackageBuildInput, GameProjectCompilerError> {
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "audio-test",
            "Audio",
            "1",
        ));
        append_asset(
            "Assets/clip.asset",
            source,
            &mut input,
            None,
            &mut Vec::new(),
        )?;
        Ok(input)
    }

    #[test]
    fn audio_asset_cooks_snapshot_wav_bytes_and_tracks_content_digest() {
        let mut snapshot = source();
        let first = cook(&snapshot).unwrap();
        let asset = &first.assets[0];
        assert_eq!(asset.asset_type, "audio");
        assert_eq!(asset.asset_guid.as_deref(), Some("guid-clip"));
        assert_eq!(asset.runtime_uri, "cooked/audio/clip.wav");
        assert_eq!(asset.runtime_payload.as_deref(), Some(wav().as_slice()));
        assert_eq!(
            asset.hash.as_deref(),
            Some(format!("sha256:{:x}", Sha256::digest(wav())).as_str())
        );
        snapshot.files.get_mut("Assets/Audio/clip.wav").unwrap()[48] = 1;
        let second = cook(&snapshot).unwrap();
        assert_ne!(
            first.assembly_input_digest().unwrap(),
            second.assembly_input_digest().unwrap()
        );
        assert_eq!(
            first.assets[0].runtime_payload.as_deref(),
            Some(wav().as_slice())
        );
    }

    #[test]
    fn audio_asset_rejects_missing_outside_and_fake_sources() {
        let mut missing = source();
        missing.files.remove("Assets/Audio/clip.wav");
        assert!(cook(&missing).is_err());
        let mut fake = source();
        fake.files
            .insert("Assets/Audio/clip.wav".into(), b"{}".to_vec());
        assert!(cook(&fake).is_err());
        let mut outside = source();
        outside.files.insert("Assets/clip.asset".into(), br#"{"schemaVersion":"audio-asset.v1","assetId":"clip","sourceAudio":"../outside.wav"}"#.to_vec());
        outside.files.insert("../outside.wav".into(), wav());
        assert!(cook(&outside).is_err());
    }

    #[test]
    fn audio_asset_rejects_source_image_before_texture_cooking() {
        let mut snapshot = source();
        snapshot.files.insert("Assets/clip.asset".into(), br#"{"schemaVersion":"audio-asset.v1","assetId":"clip","sourceAudio":"Assets/Audio/clip.wav","sourceImage":"Assets/Textures/clip.png"}"#.to_vec());
        snapshot
            .files
            .insert("Assets/Textures/clip.png".into(), b"not-a-png".to_vec());
        let error = cook(&snapshot).unwrap_err();
        assert_eq!(
            error.code(),
            "game_project_compiler.audio_source_image_unsupported"
        );
        assert!(error.message().contains("sourceImage"));
    }
}

#[cfg(test)]
mod legacy_aui_tests {
    use super::*;
    use engine_runtime::aui::AuiComputedRect;
    use serde_json::json;

    #[test]
    fn legacy_aui_hud_children_do_not_cover_the_game_view() {
        let source = json!({"documentId":"test.hud", "root":{
        "nodeId":"root", "nodeType":"canvas", "children":[
            {"nodeId":"icon", "nodeType":"image-row", "anchor":"top-right",
             "imageRef":{"id":"icon.texture"}},
            {"nodeId":"meter", "nodeType":"progress-bar", "value":0.5},
            {"nodeId":"label", "nodeType":"text", "text":"STATUS"}
        ]}});
        let (_, cooked) = cook_aui_document("AUI/test.json", source).unwrap();
        let doc: AuiDocument = serde_json::from_value(cooked).unwrap();
        let parent = AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let icon = doc.nodes.iter().find(|n| n.node_id == "icon").unwrap();
        let rect = icon.rect.resolve(parent);
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (1112.0, 24.0, 120.0, 32.0)
        );
        for node in doc.nodes.iter().filter(|n| n.node_id != "root") {
            let rect = node.rect.resolve(parent);
            assert!(rect.width * rect.height < parent.width * parent.height / 4.0);
            assert!(
                node.binding_refs.is_empty(),
                "Compiler must not guess gameplay bindings"
            );
        }
    }

    #[test]
    fn legacy_aui_explicit_rect_style_and_visibility_survive_cooking() {
        let source = json!({"documentId":"explicit", "root":{
        "nodeId":"root", "nodeType":"canvas", "children":[{
            "nodeId":"label", "nodeType":"text", "text":"TEST", "visible":false,
            "rect":{"x":48,"y":90,"width":180,"height":36},
            "textColor":"#ffcc00", "fontSize":28
        }]}});
        let (_, cooked) = cook_aui_document("AUI/test.json", source).unwrap();
        let doc: AuiDocument = serde_json::from_value(cooked).unwrap();
        let node = doc.nodes.iter().find(|n| n.node_id == "label").unwrap();
        assert_eq!(node.rect, AuiRect::fixed_position(48.0, 90.0, 180.0, 36.0));
        assert!(!node.visible);
        assert_eq!(
            node.style.as_ref().unwrap().text_color.as_deref(),
            Some("#ffcc00")
        );
        assert_eq!(node.style.as_ref().unwrap().font_size, Some(28.0));
    }

    #[test]
    fn legacy_aui_repair_preserves_canonical_document() {
        let node = AuiNode::new(
            "root",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(2.0, 3.0, 40.0, 50.0),
        );
        let doc = AuiDocument::new(
            "canonical",
            vec![AuiCanvas::screen_overlay("main", 640.0, 480.0, "root")],
            vec![node],
        );
        let source = serde_json::to_value(&doc).unwrap();
        let (_, cooked) = cook_aui_document("AUI/test.json", source.clone()).unwrap();
        assert_eq!(cooked, source);
    }
}
