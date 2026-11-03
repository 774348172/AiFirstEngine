use super::{target_catalog, BenchmarkError, BenchmarkResult, TARGET_RECIPES};
use crate::{
    CheckProfile, ExecutionMode, GameProjectCompiler, PreparedRuntimePackage,
    ProjectAuthoringSession, ProjectRelativePath, RunOptions, TargetProfile,
};
use authoring_project_context::ProjectSnapshot;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::render_contract::{CookedRenderAsset, RenderAssetData, RenderContract};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticWindow {
    pub warmup_frames: u64,
    pub sample_frames: u64,
    pub repeats: u64,
    pub minimum_wall_seconds: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BenchmarkRequest {
    pub revision: String,
    pub run_id: String,
    pub output_parent: PathBuf,
    pub project_root: PathBuf,
    pub source_revision: String,
    pub target_recipe_sha256: String,
    pub workload: String,
    pub stage_recipe: String,
    pub mode: String,
    pub operation: String,
    pub target_profile: String,
    pub device_profile: String,
    pub backend: String,
    pub evidence_level: String,
    pub window: DiagnosticWindow,
    pub fault_schedule: Vec<String>,
    pub timeout_ms: u64,
}
impl BenchmarkRequest {
    pub fn validate(&self) -> BenchmarkResult<()> {
        self.validate_fields()?;
        self.output_root()?;
        Ok(())
    }
    fn validate_fields(&self) -> BenchmarkResult<()> {
        if self.revision != "371-request-v1"
            || self.target_recipe_sha256 != sha256_prefixed(TARGET_RECIPES)
        {
            return Err(BenchmarkError::new(
                "benchmark.request_revision",
                "request/catalog revision mismatch",
            ));
        }
        let world_recipe = self.stage_recipe == "world-v1"
            && matches!(self.workload.as_str(), "BM-08" | "BM-10" | "BM-13");
        let sim_recipe = self.stage_recipe == "sim-v1"
            && matches!(
                self.workload.as_str(),
                "BM-05" | "BM-06" | "BM-07" | "BM-09"
            );
        let static_recipe = self.stage_recipe == "static-source-v1" && self.workload == "BM-02";
        let delivery_recipe =
            self.stage_recipe == "delivery-source-v1" && self.workload == "DELIVERY";
        let cell_recipe = self.stage_recipe == "foundation-v1" && self.workload == "BM-08";
        let static_full_recipe = self.stage_recipe == "foundation-v1" && self.workload == "BM-14";
        let lit_recipe = matches!(self.stage_recipe.as_str(), "lit-v1" | "lit-mini-v1")
            && self.workload == "BM-04";
        let hud_recipe = self.stage_recipe == "foundation-v1" && self.workload == "BM-01";
        let projection_recipe = self.stage_recipe == "foundation-v1" && self.workload == "BM-09";
        let pressure_recipe = self.stage_recipe == "foundation-v1" && self.workload == "BM-12";
        let foundation_recipe = hud_recipe || projection_recipe;
        if self.mode != "diagnostic"
            || (!static_recipe
                && !delivery_recipe
                && !cell_recipe
                && !static_full_recipe
                && !foundation_recipe
                && !pressure_recipe
                && !lit_recipe
                && !sim_recipe
                && !world_recipe)
        {
            return Err(BenchmarkError::new("benchmark.recipe_unimplemented", "only BM-01/BM-09 foundation Windows diagnostics, BM-02/static-source-v1, BM-08/foundation-v1, BM-14/foundation-v1 static Cook, BM-04/lit-v1 or lit-mini-v1 and DELIVERY/delivery-source-v1 and BM-05/BM-06/BM-07/BM-09 sim-v1 full cooked loads plus their 660/720/1200-tick functional variants, and BM-08/BM-10/BM-13 world-v1 original Cook/build/40000, 360 or 120-tick functional scenarios have consumers; performance qualification is unavailable"));
        }
        if foundation_recipe
            && (!matches!(
                self.operation.as_str(),
                "cook" | "build" | "run-windowed" | "run-scenario"
            ) || !self.target_profile.starts_with("windows-"))
        {
            return Err(BenchmarkError::new(
                "benchmark.profile_unimplemented",
                "BM01/BM09 foundation supports Windows Cook/build/window/scenario only",
            ));
        }
        if (sim_recipe || world_recipe)
            && (!matches!(
                self.operation.as_str(),
                "cook" | "build" | "run-windowed" | "run-scenario"
            ) || !self.target_profile.starts_with("windows-"))
        {
            return Err(BenchmarkError::new("benchmark.profile_unimplemented", "BM05/BM06/BM07 sim-v1 supports actual Windows Cook/build/window/semantic variants only; no Android or headless drawing fallback"));
        }
        let scenario = self.operation == "run-scenario";
        if pressure_recipe
            && (!matches!(self.operation.as_str(), "cook" | "build" | "run-windowed")
                || !self.target_profile.starts_with("windows-"))
        {
            return Err(BenchmarkError::new("benchmark.profile_unimplemented", "BM12 static pressure supports Windows Cook/build/bounded windows only; full stress/fault matrix is unavailable"));
        }
        if scenario && !foundation_recipe && !sim_recipe && !world_recipe {
            return Err(BenchmarkError::new(
                "benchmark.profile_unimplemented",
                "scenario consumer supports BM01/BM09 foundation and BM05/BM06/BM07 functional variants",
            ));
        }
        if lit_recipe
            && (!matches!(self.operation.as_str(), "cook" | "build" | "run-windowed")
                || !self.target_profile.starts_with("windows-"))
        {
            return Err(BenchmarkError::new("benchmark.profile_unimplemented", "BM-04 lit recipes support Windows Cook/build/real window diagnostics; headless drawing and formal qualification are unavailable"));
        }
        if static_full_recipe
            && (self.operation != "cook" || !self.target_profile.starts_with("windows-"))
        {
            return Err(BenchmarkError::new("benchmark.profile_unimplemented", "BM-14 foundation supports Windows static Cook only; runtime scale and other domains remain unqualified"));
        }
        if cell_recipe
            && (self.target_profile == "android-release" || self.operation == "run-headless")
        {
            return Err(BenchmarkError::new("benchmark.profile_unimplemented", "cell diagnostic currently supports Windows Cook/build, not headless ACK or Android"));
        }
        let pc = matches!(
            self.target_profile.as_str(),
            "windows-dev" | "windows-release"
        ) && self.device_profile == "pc-rtx3070-v1"
            && ((matches!(self.operation.as_str(), "run-windowed" | "run-scenario")
                && (lit_recipe
                    || foundation_recipe
                    || pressure_recipe
                    || sim_recipe
                    || world_recipe)
                && self.backend == "wgpu-windowed")
                || (!matches!(self.operation.as_str(), "run-windowed" | "run-scenario")
                    && self.backend == "headless-cpu"));
        let android = self.target_profile == "android-release"
            && self.device_profile == "android-toolchain-unqualified"
            && self.backend == "native-build"
            && self.operation == "build";
        if !pc && !android {
            return Err(BenchmarkError::new("benchmark.profile_unimplemented", "PC Cook/build/headless requires headless-cpu; lit run-windowed requires wgpu-windowed; Android arm64 is build-only; no platform/backend fallback"));
        }
        if !matches!(
            self.operation.as_str(),
            "cook" | "build" | "run-headless" | "run-windowed" | "run-scenario"
        ) || !matches!(self.evidence_level.as_str(), "off" | "summary")
            || !self.fault_schedule.is_empty()
            || self.window.repeats != 1
            || self.window.warmup_frames != 0
            || (if scenario {
                self.window.sample_frames
                    != if world_recipe {
                        if self.workload == "BM-08" {
                            40000
                        } else if self.workload == "BM-10" {
                            360
                        } else {
                            120
                        }
                    } else if projection_recipe && !sim_recipe {
                        63
                    } else if sim_recipe {
                        if self.workload == "BM-09" {
                            660
                        } else if matches!(self.workload.as_str(), "BM-06" | "BM-07") {
                            1200
                        } else {
                            720
                        }
                    } else {
                        7201
                    }
            } else {
                !(1..=120).contains(&self.window.sample_frames)
            })
            || self.window.minimum_wall_seconds != 0
            || !(1000..=600_000).contains(&self.timeout_ms)
        {
            return Err(BenchmarkError::new(
                "benchmark.diagnostic_scope",
                "bounded one-shot diagnostic; no trajectory/fault/performance qualification",
            ));
        }
        if !engine_runtime::render_contract::is_sha256_digest(&self.source_revision)
            || !self.project_root.is_absolute()
        {
            return Err(BenchmarkError::new(
                "benchmark.source_identity",
                "absolute project root and exact source digest required",
            ));
        }
        Ok(())
    }
    pub fn output_root(&self) -> BenchmarkResult<PathBuf> {
        if self.run_id.is_empty()
            || self.run_id.len() > 64
            || !self
                .run_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !self.output_parent.is_absolute()
            || !self.output_parent.is_dir()
        {
            return Err(BenchmarkError::new(
                "benchmark.output_ownership",
                "existing absolute parent and one literal run-id required",
            ));
        }
        let parent = fs::canonicalize(&self.output_parent).map_err(io_error)?;
        let path = parent.join(&self.run_id);
        if path.exists() {
            return Err(BenchmarkError::new(
                "benchmark.output_exists",
                "run output must be fresh; never overwrite original failures",
            ));
        }
        Ok(path)
    }
}

pub fn source_digest(snapshot: &ProjectSnapshot) -> String {
    let entries = snapshot
        .files
        .iter()
        .map(|f| (f.relative_path.clone(), sha256_prefixed(&f.bytes)))
        .collect::<BTreeMap<_, _>>();
    sha256_prefixed(&serde_json::to_vec(&entries).expect("string map serializes"))
}
pub fn project_source_digest(root: &Path) -> BenchmarkResult<String> {
    let mut session = ProjectAuthoringSession::open(root).map_err(owner_error)?;
    let paths = session
        .source_inventory()
        .map_err(owner_error)?
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("371-source-identity", paths)
        .map_err(owner_error)?;
    Ok(source_digest(lease.snapshot()))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticCookEvidence {
    pub render_instances: usize,
    pub unique_meshes: usize,
    pub unique_materials: usize,
    pub unique_textures: usize,
    pub unique_geometry_triangles: u64,
    pub source_instance_triangle_sum: u64,
    pub vertex_bytes: u64,
    pub index_bytes: u64,
    pub base_color_decoded_bytes: u64,
    pub payload_digests: BTreeMap<String, String>,
}
/// Count actual decoded payloads and scene bindings, never declaration fields alone.
pub fn inspect_static_cook(
    prepared: &PreparedRuntimePackage,
) -> BenchmarkResult<StaticCookEvidence> {
    inspect_cook(prepared, true)
}
fn inspect_cook(
    prepared: &PreparedRuntimePackage,
    static_recipe: bool,
) -> BenchmarkResult<StaticCookEvidence> {
    inspect_cook_for_counts(
        prepared,
        static_recipe,
        static_recipe.then_some((10000, 100, 32, 8, 10000000)),
    )
}

fn inspect_cook_for_counts(
    prepared: &PreparedRuntimePackage,
    static_recipe: bool,
    expected_counts: Option<(usize, usize, usize, usize, u64)>,
) -> BenchmarkResult<StaticCookEvidence> {
    let input = prepared.runtime_package_build_input();
    let mut meshes = BTreeMap::new();
    let mut result = StaticCookEvidence {
        render_instances: 0,
        unique_meshes: 0,
        unique_materials: 0,
        unique_textures: input.texture_payloads.len(),
        unique_geometry_triangles: 0,
        source_instance_triangle_sum: 0,
        vertex_bytes: 0,
        index_bytes: 0,
        base_color_decoded_bytes: 0,
        payload_digests: BTreeMap::new(),
    };
    for asset in &input.assets {
        if !matches!(asset.asset_type.as_str(), "mesh" | "material") {
            continue;
        }
        let payload = asset
            .runtime_payload
            .as_ref()
            .ok_or_else(|| BenchmarkError::new("benchmark.cook_missing", asset.asset_id.clone()))?;
        let cooked = RenderContract::<CookedRenderAsset>::decode(
            std::str::from_utf8(payload).map_err(owner_error)?,
        )
        .map_err(owner_error)?
        .data;
        result
            .payload_digests
            .insert(asset.asset_id.clone(), sha256_prefixed(payload));
        match cooked.asset {
            RenderAssetData::Mesh(g) => {
                if static_recipe && (g.vertex_count != 600 || g.index_count != 3000) {
                    return Err(BenchmarkError::new(
                        "benchmark.geometry_count",
                        asset.asset_id.clone(),
                    ));
                }
                let triangles = g.index_bytes.len() as u64 / g.index_format.byte_len() as u64 / 3;
                meshes.insert(asset.asset_id.clone(), triangles);
                result.unique_geometry_triangles += triangles;
                result.vertex_bytes += g.vertex_bytes.len() as u64;
                result.index_bytes += g.index_bytes.len() as u64;
                result.unique_meshes += 1;
            }
            RenderAssetData::Material(_) => result.unique_materials += 1,
        }
    }
    for scene in &input.scenes {
        for entity in &scene.entities {
            if let Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D {
                mesh_renderer: Some(renderer),
                ..
            }) = &entity.spatial
            {
                result.render_instances += 1;
                result.source_instance_triangle_sum +=
                    meshes.get(&renderer.mesh.0.id).ok_or_else(|| {
                        BenchmarkError::new("benchmark.mesh_missing", entity.id.clone())
                    })?;
            }
        }
    }
    for texture in &input.texture_payloads {
        result.base_color_decoded_bytes += texture.rgba8.len() as u64;
        if static_recipe
            && (texture.rgba8.len() != 2048 * 2048 * 4
                || texture
                    .rgba8
                    .chunks_exact(4)
                    .all(|p| p == &texture.rgba8[..4]))
        {
            return Err(BenchmarkError::new(
                "benchmark.texture_count",
                "nonconstant 2048-square baseColor required",
            ));
        }
    }
    if expected_counts.is_some_and(|expected| {
        (
            result.render_instances,
            result.unique_meshes,
            result.unique_materials,
            result.unique_textures,
            result.source_instance_triangle_sum,
        ) != expected
    }) {
        return Err(BenchmarkError::new(
            "benchmark.target_count",
            format!("actual static cook differs from frozen BM-02 target: {result:?}"),
        ));
    }
    if !static_recipe && (result.render_instances == 0 || input.aui_documents.is_empty()) {
        return Err(BenchmarkError::new(
            "benchmark.delivery_input_ineligible",
            "delivery smoke requires actual 3D instance bindings and AUI documents",
        ));
    }
    Ok(result)
}

type PressureCounts = (usize, usize, usize, usize, u64);
fn pressure_counts(descriptor: &serde_json::Value) -> BenchmarkResult<PressureCounts> {
    let fail = |m: &str| BenchmarkError::new("benchmark.pressure_payload_ineligible", m);
    let multiplier = descriptor["multiplier"]
        .as_f64()
        .ok_or_else(|| fail("missing multiplier"))?;
    if descriptor["workload"] != "BM-12"
        || !matches!(descriptor["baseWorkload"].as_str(), Some("BM-02" | "BM-03"))
        || ![1., 1.5, 2.].contains(&multiplier)
        || !matches!(descriptor["track"].as_str(), Some("K0" | "K1"))
    {
        return Err(fail(
            "only frozen BM02/BM03 static multipliers and K0/K1 are implemented",
        ));
    }
    let catalog = target_catalog()?;
    let base_recipe = catalog["workloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == descriptor["baseWorkload"])
        .ok_or_else(|| fail("unknown base workload"))?;
    let base = &base_recipe["pc"];
    if descriptor["seed"] != base_recipe["seed"] {
        return Err(fail("base source seed mismatch"));
    }
    let scaled =
        |key: &str| (base[key].as_u64().expect("frozen count") as f64 * multiplier) as usize;
    Ok((
        scaled("instances"),
        scaled("meshes"),
        scaled("materials"),
        scaled("textureSets"),
        scaled("instances") as u64 * 1000,
    ))
}

fn inspect_pressure_cook(prepared: &PreparedRuntimePackage) -> BenchmarkResult<serde_json::Value> {
    let fail = |m: &str| BenchmarkError::new("benchmark.pressure_payload_ineligible", m);
    let descriptor: serde_json::Value = serde_json::from_slice(
        prepared
            .source()
            .bytes("Benchmark/foundation-manifest.json")
            .ok_or_else(|| fail("missing static pressure descriptor"))?,
    )
    .map_err(owner_error)?;
    let expected = pressure_counts(&descriptor)?;
    let multiplier = descriptor["multiplier"].as_f64().expect("checked");
    let input = prepared.runtime_package_build_input();
    if input.scenes.len() != 1
        || input.scenes[0].entities.len() != expected.0 + 1
        || input.scenes[0].main_camera.as_deref() != Some("camera")
        || !input.aui_documents.is_empty()
    {
        return Err(fail(
            "static pressure requires exact live roots, selected camera, no extra HUD",
        ));
    }
    for entity in &input.scenes[0].entities {
        if !entity.enabled || entity.parent_id.is_some() {
            return Err(fail("disabled/child pressure instances do not count"));
        }
        match &entity.spatial {
            Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D {
                mesh_renderer: Some(mesh),
                camera: None,
                light: None,
                ..
            }) if entity.id != "camera" && mesh.visible => (),
            Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D {
                mesh_renderer: None,
                camera: Some(camera),
                light: None,
                ..
            }) if entity.id == "camera" && camera.enabled => {
                let expected_projection = if descriptor["track"] == "K0" {
                    engine_runtime::render_contract::Projection::Orthographic {
                        size: 220.,
                        near: 0.1,
                        far: 2000.,
                    }
                } else {
                    engine_runtime::render_contract::Projection::Perspective {
                        fov_y_degrees: 60.,
                        near: 0.1,
                        far: 2000.,
                    }
                };
                if camera.projection != expected_projection {
                    return Err(fail("pressure projection mismatch"));
                }
            }
            _ => return Err(fail("unexpected pressure scene content")),
        }
    }
    let cooked = inspect_cook_for_counts(prepared, true, Some(expected))?;
    Ok(
        serde_json::json!({"scope":"BM12-static-pressure-bounded-window","baseWorkload":descriptor["baseWorkload"],"multiplier":multiplier,"track":descriptor["track"],"actual":cooked,"qualification":"unqualified","notExecuted":["all-visible-full-K0/K1","other-domain-pressure","exhaustion-injection"]}),
    )
}

fn owner_error(error: impl std::fmt::Display) -> BenchmarkError {
    BenchmarkError::new("benchmark.owner_failure", error.to_string())
}
fn compiler_error(error: crate::GameProjectCompilerError) -> BenchmarkError {
    let mut result = owner_error(&error);
    result.owner_details = Some(serde_json::json!({
        "code":error.code(), "stage":format!("{:?}",error.stage()), "message":error.message(),
        "sourceLocation":error.source_location(), "diagnostics":error.diagnostics(), "nextAction":error.next_action()
    }));
    result
}
fn io_error(error: std::io::Error) -> BenchmarkError {
    BenchmarkError::new("benchmark.io", error.to_string())
}
fn write_json(path: &Path, value: &impl Serialize) -> BenchmarkResult<()> {
    fs::write(path, serde_json::to_vec_pretty(value).map_err(owner_error)?).map_err(io_error)
}

pub fn run_request(path: &Path) -> BenchmarkResult<PathBuf> {
    let bytes = fs::read(path).map_err(io_error)?;
    if bytes.len() > 1024 * 1024 {
        return Err(BenchmarkError::new(
            "benchmark.request_size",
            "request exceeds 1MiB",
        ));
    }
    let request: BenchmarkRequest = serde_json::from_slice(&bytes).map_err(owner_error)?;
    let output = request.output_root()?;
    // Exclusive creation preserves the first failing attempt. No cleanup of existing state.
    fs::create_dir(&output).map_err(io_error)?;
    fs::write(output.join("request.json"), &bytes).map_err(io_error)?;
    let result = execute(&request, &output);
    if let Err(error) = &result {
        write_json(
            &output.join("failure.json"),
            &serde_json::json!({"code":error.code,"message":error.message,"ownerDetails":error.owner_details,"status":"failed","qualification":"unqualified"}),
        )?;
    }
    result.map(|_| output)
}

fn execute(request: &BenchmarkRequest, output: &Path) -> BenchmarkResult<()> {
    let started = Instant::now();
    request.validate_fields()?;
    target_catalog()?;
    let mut session = ProjectAuthoringSession::open(&request.project_root).map_err(owner_error)?;
    let paths = session
        .source_inventory()
        .map_err(owner_error)?
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease(format!("371-{}", request.run_id), paths)
        .map_err(owner_error)?;
    if source_digest(lease.snapshot()) != request.source_revision {
        return Err(BenchmarkError::new(
            "benchmark.source_changed",
            "immutable source closure differs from frozen request",
        ));
    }
    let snapshot_ms = started.elapsed().as_secs_f64() * 1000.;
    let target = match request.target_profile.as_str() {
        "windows-release" => TargetProfile::WindowsRelease,
        "android-release" => TargetProfile::AndroidRelease,
        _ => TargetProfile::WindowsDev,
    };
    let compiler = GameProjectCompiler::bind(&lease).map_err(compiler_error)?;
    let sim_recipe = request.stage_recipe == "sim-v1";
    let world_recipe = request.stage_recipe == "world-v1";
    let lit_recipe = request.workload == "BM-04";
    let hud_recipe = request.workload == "BM-01";
    let projection_recipe = request.workload == "BM-09";
    let pressure_recipe = request.workload == "BM-12";
    let module_recipe = lit_recipe
        || hud_recipe
        || projection_recipe
        || pressure_recipe
        || sim_recipe
        || world_recipe;
    // Lit Cook does not claim a Rust module check. Build compiles the exact
    // leased module itself; do not duplicate that process with Compiler.check.
    let check_started = Instant::now();
    if !module_recipe {
        let checked = compiler
            .check(
                &lease,
                // Explicit offline driver invocation authorizes the existing project check process.
                CheckProfile::new(target)
                    .with_process_approval(true)
                    .with_timeout_ms(request.timeout_ms),
            )
            .map_err(compiler_error)?;
        if request.evidence_level == "summary" {
            write_json(&output.join("check.json"), &checked)?;
        }
    }
    let check_ms = if module_recipe {
        0.0
    } else {
        check_started.elapsed().as_secs_f64() * 1000.
    };
    let cook_started = Instant::now();
    // Evidence is fresh per run, while the existing Compiler cache is reusable
    // across lit requests under this explicitly owned parent. The dot prefix
    // cannot collide with any validated literal run-id.
    let cook_cache_root = if module_recipe {
        request.output_parent.join(".lit-cook-cache")
    } else {
        output.join("cook-cache")
    };
    let prepared = if matches!(
        request.workload.as_str(),
        "BM-01"
            | "BM-04"
            | "BM-05"
            | "BM-06"
            | "BM-07"
            | "BM-08"
            | "BM-09"
            | "BM-10"
            | "BM-12"
            | "BM-13"
            | "BM-14"
    ) {
        compiler.prepare_with_artifact_cache(&lease, target, &cook_cache_root)
    } else {
        compiler.prepare(&lease, target)
    }
    .map_err(compiler_error)?;
    let cook_ms = cook_started.elapsed().as_secs_f64() * 1000.;
    // Prepared owns its validated source; the original lease is no longer an
    // input to inspection/export. Do not retain a second source through build.
    drop(lease.release());
    drop(session);
    if pressure_recipe {
        write_json(
            &output.join("cook.json"),
            &inspect_pressure_cook(&prepared)?,
        )?;
    } else if world_recipe {
        write_json(
            &output.join("cook.json"),
            &super::world::inspect_world_cook(&prepared, &request.workload)?,
        )?;
    } else if sim_recipe {
        write_json(
            &output.join("cook.json"),
            &super::inspect_simulation_cook(&prepared, &request.workload)?,
        )?;
    } else if projection_recipe {
        write_json(
            &output.join("cook.json"),
            &super::inspect_foundation_projection(&prepared)?,
        )?;
    } else if hud_recipe {
        write_json(
            &output.join("cook.json"),
            &super::inspect_foundation_hud(&prepared)?,
        )?;
    } else if lit_recipe {
        let evidence = if request.stage_recipe == "lit-mini-v1" {
            super::inspect_lit_diagnostic(&prepared)?
        } else {
            super::inspect_lit_cook(&prepared)?
        };
        write_json(&output.join("cook.json"), &evidence)?;
    } else if request.workload == "BM-14" {
        let evidence = super::inspect_foundation_static_cook(&prepared)?;
        write_json(&output.join("cook.json"), &evidence)?;
    } else if request.workload == "BM-08" {
        let evidence = super::inspect_foundation_cells(&prepared)?;
        write_json(&output.join("cook.json"), &evidence)?;
    } else {
        let evidence = inspect_cook(&prepared, request.workload == "BM-02")?;
        write_json(&output.join("cook.json"), &evidence)?;
    }
    if request.evidence_level == "summary" {
        write_json(&output.join("producers.json"), &prepared.producer_reports())?;
        write_json(&output.join("prepare.json"), &prepared.prepare_summary())?;
    }
    let mut build_ms = 0.0;
    let mut player_ms = 0.0;
    let scenario = if request.operation == "run-scenario" && sim_recipe {
        Some(super::simulation::simulation_scenario(
            &prepared,
            &request.workload,
            request.window.sample_frames,
            request.timeout_ms,
        )?)
    } else if request.operation == "run-scenario" && world_recipe {
        Some(super::world::world_scenario(
            &prepared,
            &request.workload,
            request.window.sample_frames,
            request.timeout_ms,
        )?)
    } else if request.operation == "run-scenario" {
        Some(super::foundation::foundation_scenario(
            &prepared,
            &request.workload,
            request.window.sample_frames,
            request.timeout_ms,
        )?)
    } else {
        None
    };
    if matches!(
        request.operation.as_str(),
        "build" | "run-windowed" | "run-scenario"
    ) {
        let build_started = Instant::now();
        let built = compiler.build(
            &prepared,
            crate::BuildRequest::for_project(
                target,
                &request.project_root,
                ProjectRelativePath::parse(format!("Build/{}", request.run_id))
                    .map_err(owner_error)?,
                crate::default_project_runtime_player_build_root(),
            )
            .with_frame_limit(request.window.sample_frames)
            .with_timeout_ms(request.timeout_ms)
            .with_player_verification(false),
        );
        build_ms = build_started.elapsed().as_secs_f64() * 1000.;
        write_json(
            &output.join("build-timing.json"),
            &serde_json::json!({
                "elapsedMs":build_ms,"succeeded":built.is_ok(),"includesPlayerExecution":false
            }),
        )?;
        let built = built.map_err(compiler_error)?;
        write_json(
            &output.join("delivery.json"),
            &serde_json::json!({"targetProfile":target.as_str(),"deliveryIdentity":built.delivery().delivery_identity(),"artifactIdentity":built.delivery().artifact_identity(),"packageDir":built.delivery().package_dir(),"desktop":built.desktop_export(),"android":built.android_export()}),
        )?;
        // Delivery and the owned scenario suffice from here. Release cooked
        // CPU payloads before starting a Player that loads the published package.
        drop(prepared);
        if let Some(scenario) = scenario {
            let player_started = Instant::now();
            let result = runtime_cli::run_bounded_semantic_playtest_unpaced(
                runtime_cli::SemanticPlaytestProcessRequest {
                    player_executable: built.delivery().package_dir().join("Game.exe"),
                    runtime_package: built.delivery().package_dir().join("data/runtime_package"),
                    scenario,
                    output_dir: output.join("cycle"),
                },
            );
            player_ms = player_started.elapsed().as_secs_f64() * 1000.;
            write_json(
                &output.join("player-timing.json"),
                &serde_json::json!({"elapsedMs":player_ms,"succeeded":result.is_ok(),"includesBuild":false}),
            )?;
            let result = result.map_err(owner_error)?;
            write_json(&output.join("player.json"), &result)?;
            if serde_json::to_value(&result).map_err(owner_error)?["overall"] != "passed" {
                return Err(BenchmarkError::new(
                    "benchmark.scenario_failed",
                    "full leased benchmark scenario failed; original player/cycle evidence retained",
                ));
            }
        }
        if request.operation == "run-windowed" {
            let player_started = Instant::now();
            let verified = compiler.verify(
                built.delivery(),
                crate::VerifyRequest::new(
                    ExecutionMode::Windowed,
                    request.window.sample_frames,
                    request.timeout_ms,
                    true,
                ),
            );
            player_ms = player_started.elapsed().as_secs_f64() * 1000.;
            write_json(
                &output.join("player-timing.json"),
                &serde_json::json!({
                    "elapsedMs":player_ms,"succeeded":verified.is_ok(),"includesBuild":false
                }),
            )?;
            let verified = verified.map_err(compiler_error)?;
            let receipt = verified.process_verification();
            // Successful process exit alone is not proof of the requested window.
            // Retain the actual receipt even when an incomplete consumer rejects.
            write_json(&output.join("player.json"), receipt)?;
            if receipt.mode != "windowed"
                || receipt.child_frames_completed != Some(request.window.sample_frames)
                || receipt.child_present_status.as_deref() != Some("presented")
                || receipt.screenshot_status.as_deref() != Some("captured")
            {
                return Err(BenchmarkError::new("benchmark.window_incomplete", "window must complete the requested frames, native present and screenshot; see player.json"));
            }
        }
    } else if request.operation == "run-headless" {
        let player_started = Instant::now();
        let executed = compiler
            .run(
                &prepared,
                RunOptions::new(
                    ExecutionMode::Headless,
                    request.window.sample_frames,
                    request.timeout_ms,
                )
                .with_project_delivery(
                    &request.project_root,
                    ProjectRelativePath::parse(format!("Build/{}", request.run_id))
                        .map_err(owner_error)?,
                    crate::default_project_runtime_player_build_root(),
                ),
            )
            .map_err(compiler_error)?;
        // Compiler.run includes build; keep this combined legacy interval
        // separate rather than pretending it is pure Player time.
        player_ms = player_started.elapsed().as_secs_f64() * 1000.;
        write_json(&output.join("player.json"), executed.desktop_export())?;
    }
    let clock_started = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(Instant::now());
    }
    let probe_ns = clock_started.elapsed().as_nanos();
    write_json(
        &output.join("result.json"),
        &serde_json::json!({
            "status":"diagnostic_completed","mode":"diagnostic","targetProfile":target.as_str(),"driverProfile":if cfg!(debug_assertions) {"debug"} else {"release"},
            "workload":request.workload,"stageRecipe":request.stage_recipe,"sourceRevision":request.source_revision,"scenarioExecuted":request.operation=="run-scenario",
            "targetRecipeSha256":request.target_recipe_sha256,"sourceCook":"passed","player":match request.operation.as_str() {"run-headless"=>"executed-headless","run-windowed"=>"executed-windowed","run-scenario"=>"executed-semantic-windowed",_=>"not_executed"},
            "measurement":{"clock":"std::time::Instant-monotonic","snapshotMs":snapshot_ms,"checkMs":check_ms,"cookMs":cook_ms,"cookCacheRoot":if matches!(request.workload.as_str(), "BM-01"|"BM-04"|"BM-05"|"BM-06"|"BM-07"|"BM-08"|"BM-09"|"BM-12"|"BM-13"|"BM-14") {Some(&cook_cache_root)} else {None},"buildMs":build_ms,"playerMs":player_ms,"playerIntervalIncludesBuild":request.operation=="run-headless","driverWallMs":started.elapsed().as_secs_f64()*1000.,"clockProbe1000CallsNs":probe_ns,"evidenceLevel":request.evidence_level},
            "qualification":{"capacity":"unqualified","performance":"unqualified","visual":"unqualified","platform":"unqualified","installed":"unqualified"},
            "contentInspection":if world_recipe {if request.workload=="BM-08" {"original-BM08-PC-256-cells-8GiB-actual-unique-cooked-textures; runtime claims separate"} else if request.workload=="BM-10" {"original-BM10-PC-same-Cook-combined-domain-bindings; runtime claims separate"} else {"original-BM13-PC-cooked-terrain-collision-nav-vegetation-audio-particles; runtime claims separate"}} else if sim_recipe {if request.workload=="BM-09" {"BM09-PC-1000-rendered-roots-10-real-characters-100-enabled-colliders-4-native-path-agents; precision requires actual zero-anchor comparison"} else if request.workload=="BM-07" {"full-BM07-PC-real-native-tiles-agents-motors-obstacles"} else if request.workload=="BM-06" {"full-BM06-PC-cooked-bodies-shapes-joints-and-motors"} else {"full-BM05-PC-cooked-rigs-skins-geometry-and-live-joint-bindings"}} else if pressure_recipe {"declared-static-base-multiplied-actual-payloads-bounded-window-only"} else if projection_recipe {"1000-mesh-roots-selected-camera-no-HUD"} else if hud_recipe {"10000-pure-spatial-roots-selected-camera-20-HUD"} else if lit_recipe {if request.operation=="run-windowed" {"PBR-normal-ORM-full-mip-cooked-windowed-no-pixel-qualification"} else {"PBR-normal-ORM-full-mip-cooked-not-drawn"}} else {"static"},
            "rustModuleCheck":if module_recipe {if request.operation=="cook" {"not-executed-prepare-only"} else {"compiled-by-build"}} else {"executed"},
            "recipeExecution":if world_recipe && request.operation=="run-scenario" {if request.workload=="BM-08" {"canonical-K2-keys-40000-tick-functional-route; SDK-command-and-wait-ticks-explicit; no formal read/upload/residency/performance qualification"} else if request.workload=="BM-10" {"original-BM10-combined-360-tick-functional; not K3/K2/visibility/particles/mixing/performance qualification"} else {"original-BM13-payload-120-tick-smoke; not K2/LOD/residency/performance qualification"}} else if sim_recipe && request.operation=="run-scenario" {if request.workload=="BM-09" {"BM09-660-tick-one-anchor-twenty-rebases-cross-domain-functional; other anchors and measured precision separate"} else if request.workload=="BM-07" {"full-BM07-load-1200-tick-functional-variants; not full-K3/performance qualification"} else if request.workload=="BM-06" {"full-BM06-load-1200-tick-functional-variants; not full-K3/performance qualification"} else {"full-BM05-load-720-tick-functional-variants; not full-K3/performance qualification"}} else if hud_recipe && request.operation=="run-scenario" {"one-complete-7200-tick-BM01-variant-plus-init; see committed scenario assertions"} else if projection_recipe && request.operation=="run-scenario" {"three-anchor-BM09-63-tick-relocation; see committed assertions and captures"} else {"bounded-diagnostic-only"},
            "notExecuted":["complete-BM-matrix","independent-pixel-qualification","actual-OS-display-event-association","GPU-entire-frame-qualification","runtime-Off-Summary-comparison","Android-device","Release-performance"]
        }),
    )
}

#[cfg(test)]
mod pressure_tests {
    use super::*;
    #[test]
    fn pressure_recipe_counts_bind_the_declared_static_base_and_seed() {
        let catalog = target_catalog().unwrap();
        for base in ["BM-02", "BM-03"] {
            let recipe = catalog["workloads"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["id"] == base)
                .unwrap();
            for scale in [1., 1.5, 2.] {
                for track in ["K0", "K1"] {
                    let descriptor = serde_json::json!({"workload":"BM-12","baseWorkload":base,"multiplier":scale,"track":track,"seed":recipe["seed"]});
                    let counts = pressure_counts(&descriptor).unwrap();
                    let scaled =
                        |key: &str| (recipe["pc"][key].as_u64().unwrap() as f64 * scale) as usize;
                    assert_eq!(
                        counts,
                        (
                            scaled("instances"),
                            scaled("meshes"),
                            scaled("materials"),
                            scaled("textureSets"),
                            scaled("instances") as u64 * 1000
                        )
                    );
                    for (key, value) in [
                        ("seed", serde_json::json!(0)),
                        ("baseWorkload", serde_json::json!("BM-04")),
                        ("track", serde_json::json!("K2")),
                        ("multiplier", serde_json::json!(0.5)),
                    ] {
                        let mut invalid = descriptor.clone();
                        invalid[key] = value;
                        assert!(
                            pressure_counts(&invalid).is_err(),
                            "{base}/{scale}/{track}/{key}"
                        );
                    }
                }
            }
        }
    }
}
