//! Read-only reimport evidence. Matching clues never allocate or move identities.
use super::*;
use crate::three_d_contract::ImportOutputKind;
use authoring_project_context::ProjectSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelImportInspectionRequest {
    pub descriptor_path: String,
    #[serde(default = "default_limit")]
    pub max_entries: usize,
}
fn default_limit() -> usize {
    256
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportSourceObject {
    pub kind: String,
    pub source_locator: String,
    pub source_fingerprint: String,
    pub name: Option<String>,
    pub duplicate_name: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportMappingEvidence {
    pub kind: String,
    pub key: String,
    pub asset_id: Option<String>,
    pub asset_guid: Option<String>,
    pub entity_id: Option<String>,
    pub slot_name: Option<String>,
    pub source_locator: String,
    pub cooked_path: Option<String>,
    pub status: String,
    pub candidate_locators: Vec<String>,
    pub ambiguous: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportReferenceImpact {
    pub source_path: String,
    pub field_path: String,
    pub value: String,
    pub mapping_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_instance_entity_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportInspectionReport {
    pub schema_version: &'static str,
    pub expected_revision_id: String,
    pub descriptor_path: String,
    pub source_path: String,
    pub source_digest: String,
    pub buildable: bool,
    pub diagnostic: Option<Value>,
    pub required_objects: Vec<ModelImportSourceObject>,
    pub committed_mappings: Vec<ModelImportMappingEvidence>,
    /// Conservative source references, including explicit instance override paths.
    pub reference_impacts: Vec<ModelImportReferenceImpact>,
    pub truncated: bool,
    pub next_action: &'static str,
}

pub fn inspect_model_import_snapshot(
    snapshot: &ProjectSnapshot,
    request: &ModelImportInspectionRequest,
) -> std::result::Result<ModelImportInspectionReport, crate::GameProjectCompilerError> {
    let source = CompilerSourceView::from_snapshot(snapshot)?;
    inspect(&source, &snapshot.revision.revision_id, request).map_err(compiler_error)
}

fn inspect(
    source: &CompilerSourceView,
    revision: &str,
    request: &ModelImportInspectionRequest,
) -> Result<ModelImportInspectionReport> {
    let path = &request.descriptor_path;
    if !(1..=4096).contains(&request.max_entries)
        || !path.starts_with("Assets/")
        || crate::ProjectRelativePath::parse(path).is_err()
    {
        return Err(fail(
            path,
            "",
            "mapping_invalid",
            "Use an Assets descriptor path and maxEntries in 1..=4096.",
        ));
    }
    let bytes = source.bytes(path).ok_or_else(|| {
        fail(
            path,
            "",
            "model_source_missing",
            "Import descriptor is absent from the snapshot.",
        )
    })?;
    let settings: ModelImportV1 = decode(
        strict_json(bytes).map_err(|error| ImportFailure {
            source_path: path.clone(),
            error,
        })?,
        "",
    )
    .map_err(|error| ImportFailure {
        source_path: path.clone(),
        error,
    })?;
    let decoded = decode_model(source, path, &settings)?;
    let mut objects = Vec::new();
    let mut add = |kind: &str, locator: String| {
        let value = decoded.json.pointer(&locator).unwrap_or(&Value::Null);
        objects.push(ModelImportSourceObject {
            kind: kind.into(),
            source_fingerprint: assembly::fingerprint(&decoded.json, &locator)
                .expect("selected object"),
            source_locator: locator,
            name: value
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string),
            duplicate_name: false,
        });
    };
    add("prefab", format!("/scenes/{}", settings.scene));
    if decoded.character.is_some() { add("character3d",format!("/scenes/{}/character",settings.scene)); }
    for node in &decoded.nodes {
        add("node", format!("/nodes/{node}"));
    }
    let meshes: BTreeSet<_> = decoded.primitives.iter().map(|p| p.mesh_index).collect();
    for mesh in meshes {
        add("mesh", format!("/meshes/{mesh}"));
    }
    let materials: BTreeSet<_> = decoded
        .primitives
        .iter()
        .map(|p| p.material_index)
        .collect();
    let mut images = BTreeSet::new();
    for index in materials {
        add(
            "material",
            index.map_or_else(
                || "/materials/default".into(),
                |i| format!("/materials/{i}"),
            ),
        );
        if let Some(index) = index {
            if let Some(texture) = decoded
                .document
                .materials()
                .nth(index)
                .unwrap()
                .pbr_metallic_roughness()
                .base_color_texture()
            {
                images.insert(texture.texture().source().index());
            }
        }
    }
    for image in images {
        add("texture", format!("/images/{image}"));
    }
    for primitive in &decoded.primitives {
        add(
            "slot",
            format!(
                "/meshes/{}/primitives/{}",
                primitive.mesh_index, primitive.primitive_index
            ),
        );
    }
    let mut names = BTreeMap::new();
    for object in &objects {
        if let Some(name) = &object.name {
            *names
                .entry((object.kind.clone(), name.clone()))
                .or_insert(0usize) += 1;
        }
    }
    for object in &mut objects {
        object.duplicate_name = object
            .name
            .as_ref()
            .is_some_and(|n| names[&(object.kind.clone(), n.clone())] > 1);
    }
    let mut mappings = Vec::new();
    let mut tokens: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut token = |value: &str, key: &str| {
        tokens.entry(value.into()).or_default().insert(key.into());
    };
    let mapping_set_unchanged = settings.outputs.iter().all(|o| {
        assembly::fingerprint(&decoded.json, &o.source_locator).as_deref()
            == Some(&o.source_fingerprint)
    }) && settings.nodes.iter().all(|n| {
        assembly::fingerprint(&decoded.json, &n.source_locator).as_deref()
            == Some(&n.source_fingerprint)
    });
    for output in &settings.outputs {
        token(&output.asset_id, &output.key);
        token(&output.asset_guid, &output.key);
        let mut evidence = mapping(
            &objects,
            output.kind.as_str(),
            &output.key,
            &output.source_locator,
            &output.source_fingerprint,
            request.max_entries,
        );
        evidence.asset_id = Some(output.asset_id.clone());
        evidence.asset_guid = Some(output.asset_guid.clone());
        evidence.cooked_path = Some(match output.kind {
            ImportOutputKind::Texture => {
                format!("cooked/textures/{}.texture.json", output.asset_id)
            }
            ImportOutputKind::Prefab => format!("prefabs/{}.json", output.asset_id),
            _ => format!("cooked/{}.asset", output.asset_id),
        });
        let matched = evidence.status == "committed" && mapping_set_unchanged;
        mappings.push(evidence);
        for slot in &output.material_slots {
            let key = format!("{}/{}", output.key, slot.key);
            token(&slot.name, &key);
            token(&slot.key, &key);
            let mut evidence = mapping(
                &objects,
                "slot",
                &key,
                &slot.source_locator,
                "",
                request.max_entries,
            );
            evidence.slot_name = Some(slot.name.clone());
            evidence.asset_id = Some(output.asset_id.clone());
            evidence.asset_guid = Some(output.asset_guid.clone());
            evidence.cooked_path = Some(format!("cooked/{}.asset", output.asset_id));
            evidence.status = if matched
                && objects
                    .iter()
                    .any(|o| o.kind == "slot" && o.source_locator == slot.source_locator)
            {
                "committed"
            } else {
                "reviewRequired"
            }
            .into();
            // Old primitive content is not in the descriptor: show all sibling choices,
            // never pretend an index or a name proves correspondence after mesh change.
            if !matched {
                evidence.candidate_locators = objects
                    .iter()
                    .filter(|o| o.kind == "slot")
                    .take(request.max_entries + 1)
                    .map(|o| o.source_locator.clone())
                    .collect();
                evidence.ambiguous = evidence.candidate_locators.len() > 1;
            }
            mappings.push(evidence);
        }
    }
    for node in &settings.nodes {
        token(&node.entity_id, &node.key);
        let mut evidence = mapping(
            &objects,
            "node",
            &node.key,
            &node.source_locator,
            &node.source_fingerprint,
            request.max_entries,
        );
        evidence.entity_id = Some(node.entity_id.clone());
        if let Some(prefab) = settings
            .outputs
            .iter()
            .find(|o| o.kind == ImportOutputKind::Prefab)
        {
            evidence.asset_id = Some(prefab.asset_id.clone());
            evidence.asset_guid = Some(prefab.asset_guid.clone());
            evidence.cooked_path = Some(format!("prefabs/{}.json", prefab.asset_id));
        }
        mappings.push(evidence);
    }
    let mut impacts = Vec::new();
    let mut truncated = false;
    for other in source.paths().filter(|p| {
        *p != path
            && ["Assets/", "Scenes/", "Prefabs/", "AUI/", "Rules/"]
                .iter()
                .any(|prefix| p.starts_with(prefix))
    }) {
        if let Ok(value) = serde_json::from_slice::<Value>(source.bytes(other).unwrap()) {
            let start = impacts.len();
            references(
                &value,
                "",
                other,
                &tokens,
                &mut impacts,
                request.max_entries,
                &mut truncated,
            );
            for impact in &mut impacts[start..] {
                if let Some(index) = impact
                    .field_path
                    .strip_prefix("/entities/")
                    .and_then(|p| p.split('/').next())
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    let entity = &value["entities"][index];
                    if entity["components"].as_array().is_some_and(|items| {
                        items
                            .iter()
                            .any(|c| c["componentType"] == "engine.prefab_instance")
                    }) {
                        impact.source_instance_entity_id =
                            entity["id"].as_str().map(str::to_string);
                    }
                }
            }
        }
    }
    let source_path = settings.source.clone();
    let source_digest = decoded.report.source_digest.clone();
    let diagnostic = assembly::compile(settings,decoded).err().map(|e| serde_json::json!({"code":e.error.code,"sourcePath":e.source_path,"fieldPath":e.error.field_path,"message":e.error.message}));
    truncated |= objects.len() > request.max_entries || mappings.len() > request.max_entries;
    objects.truncate(request.max_entries);
    mappings.truncate(request.max_entries);
    let mut remaining_candidates = request.max_entries;
    for mapping in &mut mappings {
        truncated |= mapping.candidate_locators.len() > remaining_candidates;
        mapping.candidate_locators.truncate(remaining_candidates);
        remaining_candidates -= mapping.candidate_locators.len();
    }
    Ok(ModelImportInspectionReport { schema_version:"model-import-inspection.v1",expected_revision_id:revision.into(),descriptor_path:path.clone(),source_path,source_digest,buildable:diagnostic.is_none(),diagnostic,required_objects:objects,committed_mappings:mappings,reference_impacts:impacts,truncated,next_action:"Review candidates and all reference impacts; explicitly edit persistent outputs/nodes/slots, then submit the descriptor and affected references through engine_project_mutate with this expectedRevisionId. Candidates never assign identity. Reinspect if truncated or revision changed." })
}
fn mapping(
    objects: &[ModelImportSourceObject],
    kind: &str,
    key: &str,
    locator: &str,
    fingerprint: &str,
    limit: usize,
) -> ModelImportMappingEvidence {
    let matching: Vec<_> = objects
        .iter()
        .filter(|o| {
            o.kind == kind && (o.source_fingerprint == fingerprint || o.source_locator == locator)
        })
        .take(limit + 1)
        .collect();
    let committed = objects.iter().any(|o| {
        o.kind == kind && o.source_locator == locator && o.source_fingerprint == fingerprint
    });
    ModelImportMappingEvidence {
        kind: kind.into(),
        key: key.into(),
        asset_id: None,
        asset_guid: None,
        entity_id: None,
        slot_name: None,
        source_locator: locator.into(),
        cooked_path: None,
        status: if committed {
            "committed"
        } else if matching.is_empty() {
            "removedOrUnmatched"
        } else {
            "reviewRequired"
        }
        .into(),
        ambiguous: matching.len() > 1,
        candidate_locators: matching
            .into_iter()
            .map(|o| o.source_locator.clone())
            .collect(),
    }
}
fn references(
    value: &Value,
    field: &str,
    path: &str,
    tokens: &BTreeMap<String, BTreeSet<String>>,
    result: &mut Vec<ModelImportReferenceImpact>,
    limit: usize,
    truncated: &mut bool,
) {
    let mut record = |value: &str, at: &str| {
        let keys: BTreeSet<_> = tokens
            .iter()
            .filter(|(token, _)| {
                value == token.as_str()
                    || value.split(['/', '.']).any(|part| part == token.as_str())
            })
            .flat_map(|(_, keys)| keys.iter().cloned())
            .collect();
        if !keys.is_empty() {
            if result.len() == limit {
                *truncated = true;
            } else {
                result.push(ModelImportReferenceImpact {
                    source_path: path.into(),
                    field_path: at.into(),
                    value: value.into(),
                    mapping_keys: keys.into_iter().collect(),
                    source_instance_entity_id: None,
                });
            }
        }
    };
    match value {
        Value::String(value) => record(value, field),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                references(
                    item,
                    &format!("{field}/{i}"),
                    path,
                    tokens,
                    result,
                    limit,
                    truncated,
                );
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                let at = format!("{field}/{}", key.replace('~', "~0").replace('/', "~1"));
                if let Some(keys) = tokens.get(key) {
                    if result.len() == limit {
                        *truncated = true;
                    } else {
                        result.push(ModelImportReferenceImpact {
                            source_path: path.into(),
                            field_path: at.clone(),
                            value: key.clone(),
                            mapping_keys: keys.iter().cloned().collect(),
                            source_instance_entity_id: None,
                        });
                    }
                }
                references(item, &at, path, tokens, result, limit, truncated);
            }
        }
        _ => {}
    }
}
