//! Spine import inside the existing neutral Assembler; all reads use its leased SourceView.
use super::*;
use std::collections::BTreeSet;

// Keep the existing structured Compiler diagnostic intact without copying its
// large representation through every new import helper's success path.
type ImportResult<T> = Result<T, Box<GameProjectCompilerError>>;

#[cfg(feature = "spine")]
const CACHE_RECIPE: &str = "spine-4.2-linear-pma.v1";
#[cfg(feature = "spine")]
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedSpine {
    asset: engine_runtime::spine_animation::CookedSpineAsset,
    skeleton: String,
    atlas: String,
    textures: Vec<(CookedTextureAsset, Vec<String>)>,
}
#[cfg(feature = "spine")]
fn cache_dependency(
    source: &CompilerSourceView,
    path: &str,
    descriptor: &serde_json::Value,
    input: &RuntimePackageBuildInput,
    target: &str,
) -> ImportResult<String> {
    use engine_runtime::{canonical_digest::sha256_prefixed, spine_animation::*};
    let mut dependencies = vec![sha256_prefixed(source.bytes(path).unwrap())];
    for field in ["skeletonSource", "atlasSource"] {
        let bytes = descriptor[field]
            .as_str()
            .and_then(|p| source.bytes(p))
            .ok_or_else(|| {
                assembly_error("spine.import.source_missing", format!("{path}: {field}"))
            })?;
        dependencies.push(sha256_prefixed(bytes));
    }
    let pages = descriptor["pages"]
        .as_object()
        .ok_or_else(|| assembly_error("spine.import.pages_missing", path))?;
    for page in pages.values() {
        let id = page["assetId"]
            .as_str()
            .ok_or_else(|| assembly_error("spine.import.page_missing", path))?;
        let texture = input
            .texture_payloads
            .iter()
            .find(|t| t.metadata.asset_id == id)
            .ok_or_else(|| assembly_error("spine.import.page_missing", id))?;
        dependencies.push(sha256_prefixed(
            &serde_json::to_vec(&texture.metadata).unwrap(),
        ));
        dependencies.push(sha256_prefixed(&texture.rgba8));
        let guids = input
            .assets
            .iter()
            .filter(|a| a.asset_id == id)
            .map(|a| (&a.asset_guid, &a.dependencies))
            .collect::<Vec<_>>();
        dependencies.push(sha256_prefixed(&serde_json::to_vec(&guids).unwrap()));
    }
    Ok(sha256_prefixed(
        &serde_json::to_vec(&(
            CACHE_RECIPE,
            SPINE_BACKEND_COMMIT,
            SPINE_CAPABILITY,
            target,
            path,
            dependencies,
        ))
        .unwrap(),
    ))
}
#[cfg(feature = "spine")]
fn encode_cache(cached: &CachedSpine, pixels: &[&[u8]]) -> Vec<u8> {
    let header = serde_json::to_vec(cached).unwrap();
    let mut bytes =
        Vec::with_capacity(8 + header.len() + pixels.iter().map(|p| p.len()).sum::<usize>());
    bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&header);
    for pixel in pixels {
        bytes.extend_from_slice(pixel);
    }
    bytes
}
#[cfg(feature = "spine")]
fn decode_cache(bytes: &[u8]) -> Option<(CachedSpine, Vec<Vec<u8>>)> {
    use engine_runtime::canonical_digest::sha256_prefixed;
    let len = usize::try_from(u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?)).ok()?;
    let end = 8usize.checked_add(len)?;
    let cached: CachedSpine = serde_json::from_slice(bytes.get(8..end)?).ok()?;
    if cached.textures.len() != cached.asset.pages.len()
        || cached.textures.len() > 8
        || sha256_prefixed(cached.skeleton.as_bytes()) != cached.asset.payload_digest
        || sha256_prefixed(cached.atlas.as_bytes()) != cached.asset.atlas_digest
    {
        return None;
    }
    let mut offset = end;
    let mut pixels = Vec::new();
    for (meta, _) in &cached.textures {
        if meta.width > 4096
            || meta.height > 4096
            || meta.byte_length > 64 * 1024 * 1024
            || meta.format != "rgba8Unorm"
            || meta.color_space != "linear"
        {
            return None;
        }
        let end = offset.checked_add(meta.byte_length)?;
        pixels.push(bytes.get(offset..end)?.to_vec());
        offset = end;
    }
    (offset == bytes.len()).then_some((cached, pixels))
}
#[cfg(feature = "spine")]
fn append_cached(
    path: &str,
    cached: (CachedSpine, Vec<Vec<u8>>),
    input: &mut RuntimePackageBuildInput,
    registry: &mut engine_runtime::spine_animation::CookedSpineRegistry,
    payloads: &mut BTreeMap<String, Vec<u8>>,
) {
    let (cached, pixels) = cached;
    for ((metadata, dependencies), rgba8) in cached.textures.into_iter().zip(pixels) {
        if input.assets.iter().any(|a| a.asset_id == metadata.asset_id) {
            continue;
        }
        let id = &metadata.asset_id;
        let mut asset = RuntimePackageSourceAsset::new(
            id,
            id,
            "texture",
            path,
            format!("cooked/textures/{id}.texture.json"),
        );
        asset.asset_guid = Some(id.clone());
        asset.sub_asset_id = Some("spine-linear-pma".into());
        asset.hash = Some(metadata.source_hash.clone());
        asset.dependencies = dependencies;
        input.assets.push(asset);
        input.texture_payloads.push(RuntimePackageSourceTexture {
            metadata,
            rgba8: rgba8.into(),
        });
    }
    let a = &cached.asset;
    let mut asset = RuntimePackageSourceAsset::new(
        &a.asset_id,
        &a.asset_id,
        "spine-skeleton",
        path,
        &a.payload_path,
    );
    asset.asset_guid = Some(a.asset_id.clone());
    asset.dependencies = a.pages.iter().map(|p| p.texture_asset_id.clone()).collect();
    asset.hash = Some(a.payload_digest.clone());
    asset.runtime_payload = Some(cached.skeleton.into_bytes());
    input.assets.push(asset);
    payloads.insert(a.atlas_path.clone(), cached.atlas.into_bytes());
    registry.assets.push(cached.asset);
}

fn declarations(source: &CompilerSourceView) -> Vec<(&str, serde_json::Value)> {
    source
        .paths()
        .filter(|p| p.starts_with("Assets/") && (p.ends_with(".json") || p.ends_with(".asset")))
        .filter_map(|p| {
            serde_json::from_slice::<serde_json::Value>(source.bytes(p)?)
                .ok()
                .map(|v| (p, v))
        })
        .filter(|(_, v)| v["schemaVersion"] == "spine-skeleton-asset.v1")
        .collect()
}
pub(super) fn owned_paths(source: &CompilerSourceView) -> BTreeSet<String> {
    declarations(source)
        .into_iter()
        .flat_map(|(path, v)| {
            [
                Some(path.to_string()),
                v["skeletonSource"].as_str().map(str::to_string),
                v["atlasSource"].as_str().map(str::to_string),
            ]
        })
        .flatten()
        .collect()
}
pub(crate) fn check(source: &CompilerSourceView) -> ImportResult<()> {
    super::spine_description::validate_headers(source).map_err(Box::new)?;
    let declarations = declarations(source);
    if declarations.is_empty() {
        if super::spine_description::descriptions(source)
            .next()
            .is_some()
        {
            return Err(Box::new(assembly_error(
                "animator.binding.unknown_skeleton",
                "Spine author description requires a skeleton declaration.",
            )));
        }
        return Ok(());
    }
    #[cfg(not(feature = "spine"))]
    {
        Err(Box::new(assembly_error(
            "spine.package.backend_not_compiled",
            "Compiler has no Spine backend",
        )))
    }
    #[cfg(feature = "spine")]
    {
        let error = |path: &str, message: String| {
            Box::new(crate::game_project_compiler::check::error(
                "spine.import.invalid_source",
                message,
                Some(crate::SourceLocation {
                    source_path: path.into(),
                    field_path: None,
                    line: None,
                    column: None,
                    generated: false,
                }),
            ))
        };
        let mut ids = BTreeSet::new();
        let mut authored = Vec::new();
        for (path, v) in declarations {
            let text = |field: &str| {
                v[field]
                    .as_str()
                    .and_then(|p| source.bytes(p))
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .ok_or_else(|| {
                        error(path, format!("{field}: missing source in leased project"))
                    })
            };
            let d = engine_spine_runtime::import::validate_declared(
                std::str::from_utf8(source.bytes(path).unwrap())
                    .map_err(|e| error(path, e.to_string()))?,
                text("skeletonSource")?,
                text("atlasSource")?,
            )
            .map_err(|e| error(path, format!("{} {} {}", e.code, e.path, e.message)))?;
            if !ids.insert(d.asset_id.clone()) {
                return Err(error(path, "duplicate Spine asset ID".into()));
            }
            for page in &d.source.pages {
                let references = source
                    .paths()
                    .filter(|p| {
                        p.starts_with("Assets/") && (p.ends_with(".json") || p.ends_with(".asset"))
                    })
                    .filter_map(|p| {
                        serde_json::from_slice::<serde_json::Value>(source.bytes(p)?).ok()
                    })
                    .filter(|v| v["assetId"].as_str() == Some(&page.texture_asset_id))
                    .collect::<Vec<_>>();
                if references.len() != 1
                    || references[0]["schemaVersion"] != "texture-asset.v1"
                    || references[0]["sourceImage"]
                        .as_str()
                        .and_then(|p| source.bytes(p))
                        .is_none()
                {
                    return Err(error(
                        path,
                        format!(
                            "texture dependency must resolve uniquely to source pixels: {}",
                            page.texture_asset_id
                        ),
                    ));
                }
            }
            authored.push(d);
        }
        super::spine_description::check(source, authored).map_err(Box::new)?;
        Ok(())
    }
}
pub(super) fn append(
    source: &CompilerSourceView,
    input: &mut RuntimePackageBuildInput,
    cache: Option<&ProjectAssemblyArtifactCache>,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
    target: &str,
) -> ImportResult<()> {
    let declarations = declarations(source);
    if declarations.is_empty() {
        return Ok(());
    }
    #[cfg(not(feature = "spine"))]
    {
        let _ = (input, cache, reports, target);
        Err(Box::new(assembly_error(
            "spine.package.backend_not_compiled",
            "Rebuild the Compiler with the spine feature.",
        )))
    }
    #[cfg(feature = "spine")]
    {
        use engine_runtime::canonical_digest::sha256_prefixed;
        use engine_runtime::spine_animation::*;
        let error =
            |message: String| Box::new(assembly_error("spine.import.dependency_invalid", message));
        let mut registry = CookedSpineRegistry::empty(60);
        let mut payloads = BTreeMap::new();
        let mut asset_ids = BTreeSet::new();
        for (path, descriptor) in declarations {
            let started = std::time::Instant::now();
            let dependency = cache_dependency(source, path, &descriptor, input, target)?;
            let key = sha256_prefixed(format!("{CACHE_RECIPE}|{dependency}").as_bytes());
            let mut report = ProjectAssemblyProducerReport::uncached("spine-cook", 0);
            report.producer_recipe_version = CACHE_RECIPE.into();
            report.recipe_key = Some(key.clone());
            if let Some(cache) = cache {
                let lookup = cache.lookup_bytes("spine-cook", &key, CACHE_RECIPE);
                report.cache_status = lookup.status;
                report.miss_reason = lookup.reason;
                report.artifact_path = lookup.artifact_path.map(|p| p.display().to_string());
                if let (Some(bytes), Some(envelope)) = (lookup.artifact, lookup.envelope) {
                    if envelope.dependency_digest == dependency
                        && envelope.output_digest == envelope.payload_digest
                    {
                        if let Some(cached) = decode_cache(&bytes) {
                            if !asset_ids.insert(cached.0.asset.asset_id.clone())
                                || input
                                    .assets
                                    .iter()
                                    .any(|a| a.asset_id == cached.0.asset.asset_id)
                            {
                                return Err(error(format!(
                                    "duplicate Spine asset {}",
                                    cached.0.asset.asset_id
                                )));
                            }
                            append_cached(path, cached, input, &mut registry, &mut payloads);
                            report.output_digest = Some(envelope.output_digest);
                            report.substages.push(
                                crate::ProjectAssemblyProducerSubstageReport::skipped(
                                    "validate-normalize-and-cook",
                                ),
                            );
                            report.duration_ms = started.elapsed().as_millis() as u64;
                            reports.push(report);
                            continue;
                        }
                    }
                    cache
                        .quarantine("spine-cook", &key)
                        .map_err(|e| error(e.to_string()))?;
                    report.cache_status = crate::ProjectAssemblyArtifactCacheStatus::Invalid;
                }
            }
            let produce_started = std::time::Instant::now();
            let bytes = |field: &str| -> ImportResult<&str> {
                let p = descriptor[field]
                    .as_str()
                    .ok_or_else(|| error(format!("{path}/{field}: expected path")))?;
                let b = source
                    .bytes(p)
                    .ok_or_else(|| error(format!("{path}/{field}: source missing: {p}")))?;
                std::str::from_utf8(b).map_err(|e| error(e.to_string()))
            };
            let declared = engine_spine_runtime::import::validate_declared(
                std::str::from_utf8(source.bytes(path).unwrap())
                    .map_err(|e| error(e.to_string()))?,
                bytes("skeletonSource")?,
                bytes("atlasSource")?,
            )
            .map_err(|e| error(format!("{} {} {}", e.code, e.path, e.message)))?;
            if !asset_ids.insert(declared.asset_id.clone())
                || input.assets.iter().any(|a| a.asset_id == declared.asset_id)
            {
                return Err(error(format!(
                    "duplicate Spine asset {}",
                    declared.asset_id
                )));
            }
            let mut texture_digests = BTreeMap::new();
            let mut pages = Vec::new();
            let mut dependencies = Vec::new();
            for page in &declared.source.pages {
                let textures = input
                    .texture_payloads
                    .iter()
                    .filter(|t| t.metadata.asset_id == page.texture_asset_id)
                    .collect::<Vec<_>>();
                if textures.len() != 1 {
                    return Err(error(format!(
                        "{path}: texture AssetRef must resolve once: {}",
                        page.texture_asset_id
                    )));
                }
                let texture = textures[0];
                if texture.metadata.mip_count != 1
                    || !matches!(
                        texture.metadata.format.as_str(),
                        "rgba8Unorm" | "rgba8UnormSrgb"
                    )
                {
                    return Err(error(format!(
                        "{path}: Spine normalization requires the source RGBA8 texture, not compressed/mipped data"
                    )));
                }
                texture_digests.insert(
                    page.texture_asset_id.clone(),
                    texture
                        .metadata
                        .source_hash
                        .trim_start_matches("sha256:")
                        .into(),
                );
                let pixels = page
                    .linear_premultiplied_rgba8(
                        texture.metadata.width,
                        texture.metadata.height,
                        &texture.rgba8,
                    )
                    .map_err(|e| error(format!("{} {} {}", e.code, e.path, e.message)))?;
                // One content-derived subasset in the existing texture owner. A shared
                // source texture keeps its original straight/sRGB semantics for other consumers.
                let variant = sha256_prefixed(
                    &serde_json::to_vec(&(
                        "spine-linear-pma.v1",
                        &page.texture_asset_id,
                        page.pma,
                        &texture.metadata.source_hash,
                        &pixels,
                    ))
                    .unwrap(),
                );
                let id = format!("spine-page-{}", &variant[7..]);
                if !input.assets.iter().any(|a| a.asset_id == id) {
                    let mut derived = texture.clone();
                    derived.metadata.asset_id = id.clone();
                    derived.metadata.cooked_asset_id = format!("cooked:{id}");
                    derived.metadata.source_hash = variant;
                    derived.metadata.color_space = "linear".into();
                    derived.metadata.format = "rgba8Unorm".into();
                    derived.metadata.mip_count = 1;
                    derived.metadata.byte_length = pixels.len();
                    derived.metadata.pixel_data_path = format!("cooked/textures/{id}.rgba");
                    derived.rgba8 = pixels.into();
                    let mut asset = RuntimePackageSourceAsset::new(
                        &id,
                        &id,
                        "texture",
                        path,
                        format!("cooked/textures/{id}.texture.json"),
                    );
                    asset.asset_guid = Some(id.clone());
                    asset.sub_asset_id = Some("spine-linear-pma".into());
                    asset.dependencies = input
                        .assets
                        .iter()
                        .filter(|a| a.asset_id == page.texture_asset_id)
                        .map(|a| a.asset_guid.clone().unwrap_or_else(|| a.asset_id.clone()))
                        .collect();
                    asset.hash = Some(derived.metadata.source_hash.clone());
                    input.assets.push(asset);
                    input.texture_payloads.push(derived);
                }
                dependencies.push(id.clone());
                pages.push(SpinePage {
                    name: page.name.clone(),
                    texture_asset_id: id,
                    pma: page.pma,
                });
            }
            let cook_key = declared
                .cook_key(&texture_digests, SPINE_CAPABILITY)
                .map_err(|e| error(e.message))?;
            // Rebind atlas page IDs to the normalized package subassets. No source paths
            // or PNG decoder are available at runtime.
            let payload = declared.source.cooked_json().as_bytes().to_vec();
            let atlas = declared.source.cooked_atlas().as_bytes().to_vec();
            let payload_path = format!("spine/{}/skeleton.json", declared.asset_id);
            let atlas_path = format!("spine/{}/skeleton.atlas", declared.asset_id);
            let events=declared.source.animation_data.iter().flat_map(|a|a.events.iter().map(|e| serde_json::json!({
                "sourceEventId":declared.event_id(&a.name,e.source_ordinal),"animation":a.name,"timeUs":e.time_us,
                "sourceOrdinal":e.source_ordinal,"name":e.name,"intValue":e.int_value,"floatValue":e.float_value,
                "stringValue":e.string_value,"volume":e.volume,"balance":e.balance
            }))).collect();
            registry.assets.push(CookedSpineAsset {
                asset_id: declared.asset_id.clone(),
                source_digest: format!("sha256:{cook_key}"),
                payload_path: payload_path.clone(),
                payload_digest: sha256_prefixed(&payload),
                atlas_path: atlas_path.clone(),
                atlas_digest: sha256_prefixed(&atlas),
                pages,
                animations: declared.source.animations.clone(),
                skins: declared.source.skins.clone(),
                bones: declared.source.bones.clone(),
                slots: declared.source.slots.clone(),
                events,
                limits: BTreeMap::from([("vertices".into(), 32768), ("indices".into(), 98304)]),
                pixels_per_unit: declared.pixels_per_unit,
            });
            let mut asset = RuntimePackageSourceAsset::new(
                &declared.asset_id,
                &declared.asset_id,
                "spine-skeleton",
                path,
                &payload_path,
            );
            asset.asset_guid = Some(declared.asset_id);
            asset.dependencies = dependencies;
            asset.hash = Some(sha256_prefixed(&payload));
            asset.runtime_payload = Some(payload);
            input.assets.push(asset);
            payloads.insert(atlas_path, atlas);
            let asset = registry.assets.last().unwrap();
            let textures = asset
                .pages
                .iter()
                .map(|page| {
                    let texture = input
                        .texture_payloads
                        .iter()
                        .find(|t| t.metadata.asset_id == page.texture_asset_id)
                        .unwrap();
                    let source = input
                        .assets
                        .iter()
                        .find(|a| a.asset_id == page.texture_asset_id)
                        .unwrap();
                    (texture.clone(), source.dependencies.clone())
                })
                .collect::<Vec<_>>();
            let cached = CachedSpine {
                asset: asset.clone(),
                skeleton: String::from_utf8(
                    input
                        .assets
                        .last()
                        .unwrap()
                        .runtime_payload
                        .clone()
                        .unwrap(),
                )
                .unwrap(),
                atlas: String::from_utf8(payloads[&asset.atlas_path].clone()).unwrap(),
                textures: textures
                    .iter()
                    .map(|(t, d)| (t.metadata.clone(), d.clone()))
                    .collect(),
            };
            let bytes = encode_cache(
                &cached,
                &textures
                    .iter()
                    .map(|(t, _)| t.rgba8.as_slice())
                    .collect::<Vec<_>>(),
            );
            let output = sha256_prefixed(&bytes);
            report.output_digest = Some(output.clone());
            report.produce_duration_ms = produce_started.elapsed().as_millis() as u64;
            report
                .substages
                .push(crate::ProjectAssemblyProducerSubstageReport::completed(
                    "validate-normalize-and-cook",
                    report.produce_duration_ms,
                ));
            if let Some(cache) = cache {
                let published = cache
                    .publish_bytes(
                        "spine-cook",
                        CACHE_RECIPE,
                        &key,
                        &dependency,
                        &output,
                        &bytes,
                    )
                    .map_err(|e| error(e.to_string()))?;
                report.cache_status = match published.status {
                    crate::ProjectAssemblyArtifactPublishStatus::Produced => {
                        crate::ProjectAssemblyArtifactCacheStatus::Produced
                    }
                    crate::ProjectAssemblyArtifactPublishStatus::PublishRaceReused => {
                        crate::ProjectAssemblyArtifactCacheStatus::PublishRaceReused
                    }
                };
                report.artifact_path = Some(published.artifact_path.display().to_string());
            }
            report.duration_ms = started.elapsed().as_millis() as u64;
            reports.push(report);
        }
        registry.assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        registry.seal().map_err(error)?;
        input.spine_animator = Some(SpinePackageSource { registry, payloads });
        Ok(())
    }
}
