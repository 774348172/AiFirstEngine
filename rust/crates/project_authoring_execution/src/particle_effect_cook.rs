//! Snapshot-only particle authoring compiler. It emits ordinary RuntimePackage assets.
use crate::game_project_compiler::{CompilerSourceView, GameProjectCompilerError, SourceLocation};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::particle_effect::program::{self, CONTRACT_CALLS, DEFAULT_BEHAVIOR};
use engine_runtime::particle_effect::*;
use engine_runtime::runtime_package_builder::RuntimePackageSourceAsset;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn error(path: &str, field: &str, message: impl std::fmt::Display) -> GameProjectCompilerError {
    GameProjectCompilerError::new_for_assembly(
        "particle_effect.invalid",
        format!("{path} [{field}]: {message}"),
    )
    .with_source_location(SourceLocation {
        source_path: path.into(),
        field_path: Some(field.into()),
        line: None,
        column: None,
        generated: false,
    })
}

pub(crate) fn cook(
    source: &CompilerSourceView,
) -> Result<Vec<RuntimePackageSourceAsset>, GameProjectCompilerError> {
    let paths: Vec<_> = source
        .paths()
        .filter(|p| p.starts_with("Assets/") && p.ends_with(".particle-effect.json"))
        .collect();
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut assets = BTreeMap::new();
    let mut guids = BTreeSet::new();
    for path in source.paths().filter(|p| {
        p.starts_with("Assets/") && (p.ends_with(".asset") || p.ends_with(".particle-effect.json"))
    }) {
        let value: Value = serde_json::from_slice(source.bytes(path).unwrap())
            .map_err(|e| error(path, "description", e))?;
        // Match the existing assembler fallback for unrelated legacy assets. Particle
        // descriptions themselves still require an explicit stable ID below.
        let id = value.get("assetId").and_then(Value::as_str).unwrap_or(path);
        if assets
            .insert(id.to_string(), (path, value.clone()))
            .is_some()
        {
            return Err(error(path, "assetId", "duplicate asset identity"));
        }
        if let Some(guid) = value
            .get("assetGuid")
            .or_else(|| value.get("asset_guid"))
            .and_then(Value::as_str)
        {
            if !guids.insert(guid.to_string()) {
                return Err(error(path, "assetGuid", "duplicate asset GUID"));
            }
        }
    }
    paths
        .into_iter()
        .map(|path| cook_one(path, source, &assets))
        .collect()
}

fn cook_one(
    path: &str,
    source: &CompilerSourceView,
    assets: &BTreeMap<String, (&str, Value)>,
) -> Result<RuntimePackageSourceAsset, GameProjectCompilerError> {
    let bytes = source.bytes(path).unwrap();
    let description: ParticleEffectDescription =
        serde_json::from_slice(bytes).map_err(|cause| {
            GameProjectCompilerError::new_for_assembly(
                "particle_effect.description_invalid",
                format!("{path}: {cause}"),
            )
            .with_source_location(SourceLocation {
                source_path: path.into(),
                field_path: Some("description".into()),
                line: Some(cause.line() as u64),
                column: Some(cause.column() as u64),
                generated: false,
            })
        })?;
    description
        .validate()
        .map_err(|e| error(path, &e.field, &e.message))?;
    // RuntimeAssetRef is shared and permissive for older consumers. This authoring surface is strict.
    let raw: Value = serde_json::from_slice(bytes).map_err(|e| error(path, "description", e))?;
    validate_reference_fields(path, &raw)?;
    let mut digests = BTreeMap::from([(path.to_string(), sha256_prefixed(bytes))]);
    let mut dependencies = BTreeSet::new();
    for (reference, expected_type, field) in description.asset_refs() {
        let (asset_path, value) = assets
            .get(&reference.id)
            .ok_or_else(|| error(path, &field, format!("missing asset '{}'", reference.id)))?;
        let expected_schema = format!("{expected_type}-asset.v1");
        if reference.asset_type != expected_type
            || value.get("schemaVersion").and_then(Value::as_str) != Some(expected_schema.as_str())
        {
            return Err(error(
                path,
                &field,
                format!("asset '{}' must have type {expected_type}", reference.id),
            ));
        }
        let guid = value
            .get("assetGuid")
            .or_else(|| value.get("asset_guid"))
            .and_then(Value::as_str)
            .unwrap_or(&reference.id);
        if reference.guid.as_deref().is_some_and(|g| g != guid) || reference.sub_asset.is_some() {
            return Err(error(
                path,
                &field,
                format!(
                    "GUID mismatch or unsupported subAsset for '{}'",
                    reference.id
                ),
            ));
        }
        dependencies.insert(guid.to_string());
        digests.insert(
            asset_path.to_string(),
            sha256_prefixed(source.bytes(asset_path).unwrap()),
        );
        // Existing source-backed image assets must include their actual source bytes in invalidation.
        if let Some(image) = value.get("sourceImage").and_then(Value::as_str) {
            let image_bytes = referenced_source(path, &field, image, source)?;
            digests.insert(image.into(), sha256_prefixed(image_bytes));
        }
    }
    let mut programs = Vec::new();
    for emitter in &description.emitters {
        let author = if let Some(author_path) = &emitter.behavior_source {
            if !author_path.ends_with(".particle.wgsl") {
                return Err(error(
                    path,
                    &format!("emitters.{}.behaviorSource", emitter.name),
                    "expected project-relative *.particle.wgsl",
                ));
            }
            let body = referenced_source(
                path,
                &format!("emitters.{}.behaviorSource", emitter.name),
                author_path,
                source,
            )?;
            digests.insert(author_path.clone(), sha256_prefixed(body));
            Some(std::str::from_utf8(body).map_err(|e| error(author_path, "wgsl", e))?)
        } else {
            None
        };
        programs.push(compile_program(path, &description, emitter, author)?);
    }
    let cooked = CookedParticleEffect {
        program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
        description,
        programs,
        source_digests: digests,
    };
    let payload = serde_json::to_vec(&cooked).map_err(|e| error(path, "cooked", e))?;
    let mut asset = RuntimePackageSourceAsset::new(
        &cooked.description.asset_id,
        &cooked.description.asset_id,
        "particle-effect",
        path,
        format!("cooked/particles/{}.json", cooked.description.asset_id),
    );
    asset.asset_guid = Some(cooked.description.asset_guid.clone());
    asset.dependencies = dependencies.into_iter().collect();
    asset.hash = Some(sha256_prefixed(&payload));
    asset.runtime_payload = Some(payload);
    Ok(asset)
}

fn referenced_source<'a>(
    origin: &str,
    field: &str,
    path: &str,
    source: &'a CompilerSourceView,
) -> Result<&'a [u8], GameProjectCompilerError> {
    crate::ProjectRelativePath::parse(path).map_err(|e| error(origin, field, e))?;
    source.bytes(path).ok_or_else(|| {
        error(
            origin,
            field,
            format!("'{path}' is missing from the immutable snapshot"),
        )
    })
}

fn validate_reference_fields(path: &str, raw: &Value) -> Result<(), GameProjectCompilerError> {
    for (i, emitter) in raw["emitters"].as_array().into_iter().flatten().enumerate() {
        for (field, reference) in [
            ("texture", &emitter["draw"]["texture"]),
            ("material", &emitter["draw"]["material"]),
            ("geometry.asset", &emitter["draw"]["geometry"]["asset"]),
        ] {
            if let Some(reference) = reference.as_object() {
                for key in reference.keys() {
                    if !["id", "type", "guid", "subAsset"].contains(&key.as_str()) {
                        return Err(error(
                            path,
                            &format!("emitters[{i}].draw.{field}.{key}"),
                            "unknown asset reference field",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn compile_program(
    path: &str,
    effect: &ParticleEffectDescription,
    emitter: &ParticleEmitter,
    author: Option<&str>,
) -> Result<CookedParticleProgram, GameProjectCompilerError> {
    let mut wgsl = program::prefix(effect, emitter);
    let first_line = wgsl.lines().count() as u32 + 1;
    let body = author.unwrap_or(DEFAULT_BEHAVIOR);
    let line_count = body.lines().count() as u32;
    wgsl.push_str(body);
    // Parse the author library before adding engine calls so project entry points/bindings
    // cannot hide among engine-owned declarations. This is structural, not a keyword filter.
    let fail = |location: Option<naga::SourceLocation>, message: String| {
        program_error(
            path, emitter, &wgsl, first_line, line_count, location, message,
        )
    };
    let module = naga::front::wgsl::parse_str(&wgsl)
        .map_err(|e| fail(e.location(&wgsl), e.message().to_string()))?;
    if !module.entry_points.is_empty() {
        return Err(error(
            emitter.behavior_source.as_deref().unwrap_or(path),
            "wgsl",
            "project particle functions cannot declare entry points",
        ));
    }
    if let Some((handle, _)) = module.global_variables.iter().next() {
        return Err(fail(Some(module.global_variables.get_span(handle).location(&wgsl)),"project particle functions cannot declare global variables or resource bindings; use parameters, inputs and returned Particle state".into()));
    }
    for (handle, function) in module.functions.iter() {
        if function
            .name
            .as_deref()
            .is_some_and(|name| name.starts_with("engine_"))
        {
            return Err(fail(
                Some(module.functions.get_span(handle).location(&wgsl)),
                "engine_ function names are reserved".into(),
            ));
        }
    }
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| {
        let loc = e
            .spans()
            .map(|(span, _)| span.location(&wgsl))
            .filter(|loc| {
                loc.line_number >= first_line && loc.line_number < first_line + line_count
            })
            .max_by_key(|loc| loc.offset)
            .or_else(|| e.location(&wgsl));
        fail(loc, format!("{e}: {:?}", e.as_inner()))
    })?;
    wgsl.push_str(CONTRACT_CALLS);
    let module = naga::front::wgsl::parse_str(&wgsl).map_err(|e| {
        program_error(
            path,
            emitter,
            &wgsl,
            first_line,
            line_count,
            e.location(&wgsl),
            e.message().to_string(),
        )
    })?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(),naga::valid::Capabilities::empty()).validate(&module)
        .map_err(|e| program_error(path,emitter,&wgsl,first_line,line_count,e.location(&wgsl),format!("particle_init/particle_update must accept (Particle, ParticleContext, EffectParams, ParticleInputs) and return Particle: {e}: {:?}",e.as_inner())))?;
    wgsl = program::compute_source(effect, emitter, &wgsl);
    let module = naga::front::wgsl::parse_str(&wgsl).map_err(|e| {
        program_error(
            path,
            emitter,
            &wgsl,
            first_line,
            line_count,
            e.location(&wgsl),
            e.message().into(),
        )
    })?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| {
        program_error(
            path,
            emitter,
            &wgsl,
            first_line,
            line_count,
            e.location(&wgsl),
            format!("GPU compute validation: {e}: {:?}", e.as_inner()),
        )
    })?;
    Ok(CookedParticleProgram {
        emitter: emitter.name.clone(),
        wgsl,
        author_path: emitter.behavior_source.clone(),
        author_first_line: first_line,
        author_line_count: line_count,
        particle_stride: program::particle_layout(emitter).2,
    })
}

fn program_error(
    path: &str,
    emitter: &ParticleEmitter,
    wgsl: &str,
    first: u32,
    count: u32,
    location: Option<naga::SourceLocation>,
    message: String,
) -> GameProjectCompilerError {
    let author_location =
        location.filter(|loc| loc.line_number >= first && loc.line_number < first + count);
    let authored = emitter.behavior_source.is_some() && author_location.is_some();
    let source_path = if authored {
        emitter.behavior_source.as_deref().unwrap()
    } else {
        path
    };
    let line = author_location
        .filter(|_| authored)
        .map(|loc| u64::from(loc.line_number - first + 1));
    let column = author_location
        .filter(|_| authored)
        .map(|loc| u64::from(loc.line_position));
    let generated_field = location
        .filter(|loc| loc.line_number < first)
        .and_then(|loc| {
            let lines: Vec<_> = wgsl.lines().take(loc.line_number as usize).collect();
            let field = lines.last()?.trim().split_once(':')?.0;
            let structure = lines
                .iter()
                .rev()
                .find(|line| line.starts_with("struct "))?;
            if structure.starts_with("struct EffectParams ") {
                Some(format!("parameters.{field}.name"))
            } else if structure.starts_with("struct ParticleState ") {
                Some(format!(
                    "emitters.{}.customState.{field}.name",
                    emitter.name
                ))
            } else if structure.starts_with("struct ParticleInputs ") {
                Some(format!("emitters.{}.inputs.{field}.name", emitter.name))
            } else {
                None
            }
        });
    let generated = !authored && generated_field.is_none();
    let source_fragment = author_location
        .and_then(|loc| wgsl.lines().nth(loc.line_number.saturating_sub(1) as usize))
        .unwrap_or_default();
    GameProjectCompilerError::new_for_assembly(
        "particle_effect.wgsl_invalid",
        format!(
            "{source_path}{} emitter '{}': {message}\n{source_fragment}",
            line.map(|n| format!(":{n}")).unwrap_or_default(),
            emitter.name
        ),
    )
    .with_source_location(SourceLocation {
        source_path: source_path.into(),
        field_path: Some(
            generated_field.unwrap_or_else(|| format!("emitters.{}.behaviorSource", emitter.name)),
        ),
        line,
        column,
        generated,
    })
}

#[cfg(test)]
mod tests;
