//! Import producers share the existing typed artifact cache. Source is never written.
use super::*;
use crate::{
    ProjectAssemblyArtifactCache, ProjectAssemblyArtifactCacheStatus as Status,
    ProjectAssemblyArtifactPublishStatus, ProjectAssemblyProducerReport,
    ProjectAssemblyProducerSubstageReport as Substage,
};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Instant;

const RECIPE: &str = "gltf-character.import-cache.v3";
const INDEX: &str = "model-import-dependencies";
const MODEL: &str = "model-import";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_cache_ignores_other_buffer_views_but_rebuilds_consumed_positions() {
        let root = std::env::temp_dir().join(format!(
            "aife-import-view-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = ProjectAssemblyArtifactCache::open(&root).unwrap();
        let json = serde_json::json!({"asset":{"version":"2.0"},"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"buffers":[{"uri":"data.bin","byteLength":48}],"bufferViews":[{"buffer":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":12}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[-1,0,0],"max":[1,1,0]}]});
        let gltf = gltf::Gltf::from_slice(&serde_json::to_vec(&json).unwrap()).unwrap();
        let primitive = gltf
            .document
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .next()
            .unwrap();
        let mut bytes = [-1.0f32, 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 0.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let mut context = ImportCache {
            cache: Some(&cache),
            scope: "view-regression".into(),
            cancelled: &|| false,
            reports: Vec::new(),
        };
        let mut cook = |bytes: &[u8]| {
            let buffers = BTreeMap::from([(0, Arc::<[u8]>::from(bytes))]);
            let geometry = context
                .geometry(
                    &json,
                    &primitive,
                    &buffers,
                    "Models/data.gltf",
                    "/meshes/0/primitives/0",
                )
                .unwrap();
            (
                serde_json::to_value(geometry).unwrap(),
                context.reports.last().unwrap().cache_status,
            )
        };
        let (first, status) = cook(&bytes);
        assert_eq!(status, Status::Produced);
        // An animation bufferView shares the same .bin, outside the mesh views.
        bytes[36..40].copy_from_slice(&0.5f32.to_le_bytes());
        let (clip_only, status) = cook(&bytes);
        assert_eq!(first, clip_only);
        assert_eq!(
            status,
            Status::Hit,
            "unconsumed animation view must not decode geometry again"
        );
        bytes[0..4].copy_from_slice(&(-0.5f32).to_le_bytes());
        let (mesh_changed, status) = cook(&bytes);
        assert_eq!(status, Status::Produced);
        assert_ne!(first, mesh_changed);
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn digest(value: &impl Serialize) -> String {
    sha256_prefixed(&serde_json::to_vec(value).expect("cache identity"))
}
fn millis(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

pub(super) struct ImportCache<'a> {
    cache: Option<&'a ProjectAssemblyArtifactCache>,
    scope: String,
    cancelled: &'a dyn Fn() -> bool,
    reports: Vec<ProjectAssemblyProducerReport>,
}
impl ImportCache<'_> {
    pub(super) fn disabled() -> Self {
        Self {
            cache: None,
            scope: String::new(),
            cancelled: &|| false,
            reports: Vec::new(),
        }
    }
    pub(super) fn check_cancelled(&self, path: &str) -> Result<()> {
        if (self.cancelled)() {
            Err(fail(
                path,
                "",
                "import_cancelled",
                "Import cancelled before publishing a complete prepared package.",
            ))
        } else {
            Ok(())
        }
    }
    fn key(&self, input: &impl Serialize) -> String {
        digest(&(RECIPE, &self.scope, input))
    }
    fn read<T: DeserializeOwned>(
        &self,
        producer: &str,
        key: &str,
        report: &mut ProjectAssemblyProducerReport,
    ) -> Option<T> {
        let start = Instant::now();
        report.producer_recipe_version = RECIPE.into();
        report.recipe_key = Some(key.into());
        let result = self.cache.and_then(|cache| {
            let lookup = cache.lookup_json::<T>(producer, key, RECIPE);
            report.cache_status = lookup.status;
            report.miss_reason = lookup.reason;
            report.artifact_path = lookup.artifact_path.map(|p| p.display().to_string());
            report.output_digest = lookup.envelope.map(|e| e.output_digest);
            lookup.artifact
        });
        report.lookup_duration_ms = millis(start);
        result
    }
    fn publish<T: Serialize>(
        &self,
        producer: &str,
        key: &str,
        value: &T,
        report: &mut ProjectAssemblyProducerReport,
        path: &str,
    ) -> Result<()> {
        let start = Instant::now();
        let output = digest(value);
        report.output_digest = Some(output.clone());
        if let Some(cache) = self.cache {
            let result = cache
                .publish_json(producer, RECIPE, key, key, &output, value)
                .map_err(|e| fail(path, "", "import_cache_failed", e.to_string()))?;
            report.cache_status = match result.status {
                ProjectAssemblyArtifactPublishStatus::Produced => Status::Produced,
                ProjectAssemblyArtifactPublishStatus::PublishRaceReused => {
                    Status::PublishRaceReused
                }
            };
            report.artifact_path = Some(result.artifact_path.display().to_string());
        }
        report.publish_duration_ms = millis(start);
        Ok(())
    }
    fn product<T: Serialize + DeserializeOwned>(
        &mut self,
        producer: &str,
        input: &impl Serialize,
        path: &str,
        produce: impl FnOnce() -> Result<T>,
    ) -> Result<(T, bool)> {
        self.check_cancelled(path)?;
        let start = Instant::now();
        let key = self.key(input);
        let mut report = ProjectAssemblyProducerReport::uncached(producer, 0);
        if let Some(value) = self.read(producer, &key, &mut report) {
            report.substages.push(Substage::skipped("decode"));
            report.duration_ms = millis(start);
            self.reports.push(report);
            return Ok((value, true));
        }
        let production = Instant::now();
        let value = produce()?;
        report.produce_duration_ms = millis(production);
        report
            .substages
            .push(Substage::completed("decode", report.produce_duration_ms));
        self.check_cancelled(path)?;
        self.publish(producer, &key, &value, &mut report, path)?;
        report.duration_ms = millis(start);
        self.reports.push(report);
        Ok((value, false))
    }
    pub(super) fn geometry(
        &mut self,
        json: &Value,
        primitive: &gltf::Primitive<'_>,
        buffers: &BTreeMap<usize, Arc<[u8]>>,
        path: &str,
        locator: &str,
    ) -> Result<CookedGeometry> {
        if self.cache.is_none() {
            return geometry::decode_primitive(primitive, buffers, path, locator);
        }
        let mut accessors = Vec::new();
        let mut views = BTreeMap::new();
        let mut data = BTreeMap::new();
        for (_, a) in primitive
            .attributes()
            .chain(primitive.indices().map(|a| (gltf::Semantic::Positions, a)))
        {
            accessors.push((a.index(), &json["accessors"][a.index()]));
            let view = a.view().expect("validated accessor");
            views.insert(view.index(), &json["bufferViews"][view.index()]);
            // The validated primitive consumes these views, not unrelated
            // animation/image views that happen to share the same .bin file.
            data.entry(view.index()).or_insert_with(|| {
                sha256_prefixed(
                    &buffers[&view.buffer().index()][view.offset()..view.offset() + view.length()],
                )
            });
        }
        let semantics: Vec<_> = primitive
            .attributes()
            .map(|(semantic, a)| (format!("{semantic:?}"), a.index()))
            .collect();
        let input = (
            format!("{:?}", primitive.mode()),
            semantics,
            primitive.indices().map(|a| a.index()),
            accessors,
            views,
            data,
        );
        self.product("model-import-geometry", &input, path, || {
            geometry::decode_primitive(primitive, buffers, path, locator)
        })
        .map(|(v, _)| v)
    }
    pub(super) fn image(
        &mut self,
        bytes: &[u8],
        input: gltf::image::Source<'_>,
        path: &str,
        budget: &mut Budget<'_>,
    ) -> Result<DecodedImage> {
        if self.cache.is_none() {
            return source::decode_image(bytes, input, path, budget);
        }
        #[derive(Serialize, Deserialize)]
        struct ImageProduct {
            image: DecodedImage,
            decoder_bytes: u64,
        }
        let hint = match input {
            gltf::image::Source::View { mime_type, .. } => vec![Some(mime_type.to_string())],
            gltf::image::Source::Uri { uri, mime_type } => vec![
                mime_type.map(str::to_string),
                uri.strip_prefix("data:")
                    .and_then(|s| s.split_once(';'))
                    .map(|(mime, _)| mime.into()),
            ],
        };
        let identity = (sha256_prefixed(bytes), hint, budget.limits.clone());
        let source_path = budget.source.to_string();
        let (product, reused) =
            self.product("model-import-image", &identity, &source_path, || {
                let before = budget.image_bytes;
                let image = source::decode_image(bytes, input, path, budget)?;
                Ok(ImageProduct {
                    image,
                    decoder_bytes: budget.image_bytes - before,
                })
            })?;
        if reused {
            charge(
                &mut budget.image_bytes,
                product.decoder_bytes,
                budget.limits.decoded_image_bytes,
                budget.source,
                path,
                "import_image_budget_exceeded",
            )?;
            charge(
                &mut budget.cooked_bytes,
                product.image.rgba8.len() as u64,
                budget.limits.cooked_bytes,
                budget.source,
                path,
                "import_cooked_budget_exceeded",
            )?;
        }
        Ok(product.image)
    }
}

pub(crate) fn compile_cached(
    source: &CompilerSourceView,
    cache: Option<&ProjectAssemblyArtifactCache>,
    scope: &str,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<assembly::CompiledImport>> {
    let mut context = ImportCache {
        cache,
        scope: scope.into(),
        cancelled,
        reports: Vec::new(),
    };
    let mut imports = Vec::new();
    for path in source
        .paths()
        .filter(|p| p.starts_with("Assets/") && (p.ends_with(".json") || p.ends_with(".asset")))
    {
        context.check_cancelled(path)?;
        let value: Value = serde_json::from_slice(source.bytes(path).unwrap())
            .map_err(|e| fail(path, "", "schema_invalid", e.to_string()))?;
        if value["schemaVersion"] != "model-import.v1" {
            continue;
        }
        let settings: ModelImportV1 = decode(value, "").map_err(|error| ImportFailure {
            source_path: path.into(),
            error,
        })?;
        let start = Instant::now();
        let base_key = context.key(&(
            path,
            &settings,
            source.bytes(&settings.source).map(sha256_prefixed),
        ));
        let mut report = ProjectAssemblyProducerReport::uncached(MODEL, 0);
        let mut index_report = ProjectAssemblyProducerReport::uncached(INDEX, 0);
        let paths: Option<Vec<String>> = context.read(INDEX, &base_key, &mut index_report);
        let full_key = |paths: &[String]| {
            context.key(&(
                &base_key,
                paths
                    .iter()
                    .map(|p| (p, source.bytes(p).map(sha256_prefixed)))
                    .collect::<Vec<_>>(),
            ))
        };
        if let Some(paths) = &paths {
            let key = full_key(paths);
            if let Some(imported) =
                context.read::<assembly::CompiledImport>(MODEL, &key, &mut report)
            {
                report
                    .substages
                    .push(Substage::skipped("decode_and_assemble"));
                report.duration_ms = millis(start);
                context.reports.push(report);
                imports.push(imported);
                continue;
            }
        }
        let production = Instant::now();
        let decoded = decode_model_cached(source, path, &settings, &mut context)?;
        let imported = assembly::compile(settings, decoded)?;
        let paths: Vec<String> = imported
            .report
            .dependencies
            .iter()
            .map(|d| d.source_path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let key = context.key(&(
            &base_key,
            paths
                .iter()
                .map(|p| (p, source.bytes(p).map(sha256_prefixed)))
                .collect::<Vec<_>>(),
        ));
        report.recipe_key = Some(key.clone());
        report.producer_recipe_version = RECIPE.into();
        report.produce_duration_ms = millis(production);
        report.substages.push(Substage::completed(
            "decode_and_assemble",
            report.produce_duration_ms,
        ));
        context.check_cancelled(path)?;
        context.publish(MODEL, &key, &imported, &mut report, path)?;
        context.publish(INDEX, &base_key, &paths, &mut index_report, path)?;
        report.duration_ms = millis(start);
        context.reports.push(report);
        imports.push(imported);
    }
    reports.extend(context.reports);
    super::bindings::validate(source, &imports)?;
    Ok(imports)
}
