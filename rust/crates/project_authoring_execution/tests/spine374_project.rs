use engine_runtime::runtime_package::*;
use engine_runtime::runtime_package_builder::*;
use project_authoring_execution::{
    CheckProfile, GameProjectCompiler, ProjectAuthoringSession, TargetProfile,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const JSON: &str = include_str!("../../engine_spine_runtime/tests/fixtures/probe.json");
const ATLAS: &str = include_str!("../../engine_spine_runtime/tests/fixtures/probe.atlas");
#[cfg(feature = "spine")]
#[path = "spine374_project/gate_d.rs"]
mod gate_d;
#[cfg(feature = "spine")]
#[path = "spine374_project/gate_e.rs"]
mod gate_e;
#[cfg(feature = "spine")]
#[path = "spine374_project/gate_f.rs"]
mod gate_f;

#[cfg(feature = "spine")]
#[test]
#[ignore = "explicit Gate F WindowsRelease frozen recipes; requires formal measurement eligibility"]
fn spine374_f_release_recipes() {
    gate_f::release_recipes();
}
struct Fixture {
    root: PathBuf,
    source: PathBuf,
}
impl Fixture {
    #[cfg(feature = "spine")]
    fn authored(&self) -> Value {
        self.write("Scenes/main.scene.json",&json!({"schemaVersion":"scene.v1","id":"main","name":"Main","entities":[{"id":"robot","name":"Robot","transform":{"localPosition":{"x":0,"y":0,"z":0},"localRotation":{"x":0,"y":0,"z":0},"localScale":{"x":1,"y":1,"z":1}},"components":[]}]}));
        let d = json!({"schema":"animator-description-2d.v2","assetId":"hero-controller","entity":"robot","backend":"spine","skeleton":{"assetId":"hero"},"default":"idle",
            "parameters":{"moving":{"kind":"bool","default":false},"fire":{"kind":"trigger"}},
            "layers":[{"name":"base","blend":"replace"},{"name":"upper","blend":"replace","mask":"upper"}],"masks":{"upper":["upper"]},
            "animations":{"idle":{"clip":"motion","layer":"base","playback":"loop","speedPermille":1000,"mixInSeconds":0},
                "run":{"clip":"attack","layer":"base","playback":"loop","speedPermille":1000,"mixInSeconds":0.1},
                "wave":{"clip":"attack","layer":"upper","playback":"once","speedPermille":1000,"mixInSeconds":0.0001,"onComplete":"fadeOutLayer","fadeOutSeconds":0.2}},
            "rules":[{"all":[{"triggered":"fire"}],"play":"wave"},{"all":[{"parameter":"moving","equals":true}],"play":"run"}],
            "sockets":{"hand":{"bone":"upper","offset":[1,0,0]}}});
        self.write("Animations/hero.animator-description-2d.json", &d);
        d
    }
    #[cfg(feature = "spine")]
    fn check(
        &self,
    ) -> Result<
        project_authoring_execution::CheckReport,
        Box<project_authoring_execution::GameProjectCompilerError>,
    > {
        let mut session = ProjectAuthoringSession::open(&self.source).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("spine374-d-check", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        Ok(compiler.check(&lease, CheckProfile::new(TargetProfile::WindowsDev))?)
    }
    fn new(name: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::var_os("SPINE374_EVIDENCE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../.artifacts/374/gate-b-integration")
            });
        let root = parent.join(format!(
            "{name}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let f = Self {
            source: root.join("source"),
            root,
        };
        f.write("project.aife.json",&json!({"schemaVersion":"aife-project.v2","projectId":"spine374-project","defaultScene":"Scenes/main.scene.json"}));
        f.write(
            "Scenes/main.scene.json",
            &json!({"schemaVersion":"scene.v1","sceneId":"main","name":"Main","entities":[]}),
        );
        f.write(
            "Input/default.json",
            &serde_json::to_value(
                engine_runtime::input_mapping::InputMappingAsset::gameplay_default(),
            )
            .unwrap(),
        );
        f.write("Assets/hero.spine.json",&json!({"schemaVersion":"spine-skeleton-asset.v1","assetId":"hero","skeletonSource":"Assets/hero.skeleton.json",
            "atlasSource":"Assets/hero.atlas","pages":{"probe.png":{"assetId":"texture-probe"}},"format":"json","exportVersion":"4.2.00","pixelsPerUnit":100}));
        f.write("Assets/page.asset",&json!({"schemaVersion":"texture-asset.v1","assetId":"texture-probe","assetGuid":"texture-probe","sourceImage":"Assets/page.png"}));
        fs::write(f.source.join("Assets/hero.skeleton.json"), JSON).unwrap();
        fs::write(f.source.join("Assets/hero.atlas"), ATLAS).unwrap();
        f.pixels([128, 64, 32, 128]);
        f
    }
    fn pixels(&self, pixel: [u8; 4]) {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixel.repeat(4))
                .unwrap();
        }
        fs::write(self.source.join("Assets/page.png"), bytes).unwrap();
    }
    fn write(&self, path: &str, v: &Value) {
        let p = self.source.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, serde_json::to_vec(v).unwrap()).unwrap();
    }
    fn prepare(
        &self,
    ) -> Result<RuntimePackageBuildInput, Box<project_authoring_execution::GameProjectCompilerError>>
    {
        self.prepare_reported().map(|(input, _)| input)
    }
    fn prepare_reported(
        &self,
    ) -> Result<
        (
            RuntimePackageBuildInput,
            Vec<project_authoring_execution::ProjectAssemblyProducerReport>,
        ),
        Box<project_authoring_execution::GameProjectCompilerError>,
    > {
        let mut s = ProjectAuthoringSession::open(&self.source).unwrap();
        let paths = s
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = s.acquire_snapshot_lease("spine374-b", paths).unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        compiler.check(&lease, CheckProfile::new(TargetProfile::WindowsDev))?;
        let prepared = compiler.prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsDev,
            &self.root.join("cache"),
        )?;
        Ok((
            prepared.runtime_package_build_input().clone(),
            prepared.producer_reports().to_vec(),
        ))
    }
    #[cfg(feature = "spine")]
    fn package(&self, input: &RuntimePackageBuildInput) -> PathBuf {
        let root = self.root.join("package");
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&root, &input.scenes[0].id),
            input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:#?}"
        );
        root
    }
}

#[cfg(not(feature = "spine"))]
#[test]
fn spine374_b_default_compiler_and_player_reject_required_backend() {
    let f = Fixture::new("default-capability");
    let e = f.prepare().unwrap_err();
    assert!(e.code().contains("backend_not_compiled"), "{e:?}");
    let package = f.root.join("package");
    fs::create_dir_all(&package).unwrap();
    // A well-formed old manifest with a newly required capability must fail before
    // attempting to use any of its scene/assets paths.
    let manifest = json!({"schemaVersion":"runtime-package.v2","packageMode":"debug-readable",
        "project":{"projectId":"x","name":"x","version":"1","runtimeModule":{"moduleId":"engine.empty.runtime","interfaceVersion":"project-runtime-module.v2","aotDigest":"x"}},
        "activeSceneId":"main","scenes":[],"assets":{"path":"none","assetCount":0},"rules":{"path":"none","mode":"rust-aot"},
        "input":{"path":"none","defaultMappingId":"none","mappingCount":0},"spineAnimator":{"path":"spine/registry.json","registryDigest":"x","requiredCapability":"animator.spine.4.2.v1"}});
    // Construct module identity with the production serde spelling.
    let mut manifest = manifest;
    manifest["project"] =
        serde_json::to_value(RuntimeProjectInfo::explicit_empty("x", "x", "1")).unwrap();
    fs::write(
        package.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let loaded = load_runtime_package(&package);
    assert!(loaded.value.is_none());
    assert!(
        loaded
            .diagnostics
            .issues
            .iter()
            .any(|i| i.message.contains("backend_not_compiled")),
        "{:?}",
        loaded.diagnostics
    );
    let player = runtime_player_winit::run_headless_native_player_from_package(
        runtime_player_winit::NativePlayerWindowRunRequest::headless_surface_gate(&package),
    );
    assert_eq!(player.package_status, "error");
    assert!(
        player
            .diagnostics
            .iter()
            .any(|i| i.message.contains("backend_not_compiled")),
        "{player:#?}"
    );
}

#[cfg(feature = "spine")]
#[test]
fn spine374_b_original_assembler_package_loads_without_source_and_has_real_pose() {
    let f = Fixture::new("roundtrip");
    let input = f.prepare().unwrap();
    let spine = input
        .spine_animator
        .as_ref()
        .expect("Assembler omitted Spine");
    assert_eq!(spine.registry.assets.len(), 1);
    let a = &spine.registry.assets[0];
    assert_eq!(a.bones, ["root", "lower", "upper"]);
    assert_eq!(a.events.len(), 3);
    let original = input
        .texture_payloads
        .iter()
        .find(|t| t.metadata.asset_id == "texture-probe")
        .unwrap();
    assert_eq!(&original.rgba8[..4], &[128, 64, 32, 128]);
    let derived = input
        .texture_payloads
        .iter()
        .find(|t| t.metadata.asset_id == a.pages[0].texture_asset_id)
        .unwrap();
    assert_eq!(derived.metadata.format, "rgba8Unorm");
    assert_eq!(derived.metadata.color_space, "linear");
    assert_eq!(&derived.rgba8[..4], &[28, 7, 2, 128]);
    let package = f.package(&input);
    fs::rename(&f.source, f.root.join("source-offline")).unwrap();
    let loaded = load_runtime_package(&package);
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    let registry = loaded.value.unwrap().spine_animator_registry.unwrap();
    let asset = registry.load_asset(&package, "hero").unwrap();
    let mut instance = asset.instance().unwrap();
    instance.play(0, "motion", None, 0.0, false, false).unwrap();
    instance.step(0.5).unwrap();
    let g = instance.geometry(100.0).unwrap();
    assert_eq!(g.indices, [0, 1, 2]);
    assert!((g.vertices[0].x - 0.03).abs() < 1e-4);
    let player = runtime_player_winit::run_headless_native_player_from_package(
        runtime_player_winit::NativePlayerWindowRunRequest::headless_surface_gate(&package),
    );
    assert_eq!(player.package_status, "ok", "{player:#?}");
    assert!(
        player
            .diagnostics
            .iter()
            .all(|i| i.severity != runtime_player_winit::NativeWindowHostDiagnosticSeverity::Error),
        "{player:#?}"
    );
    fs::write(
        f.root.join("source-absent-player.json"),
        serde_json::to_vec_pretty(&player).unwrap(),
    )
    .unwrap();
}

#[cfg(feature = "spine")]
#[test]
fn spine374_b_check_rejects_missing_pages_and_unsafe_or_unsupported_sources() {
    for case in [
        "missing-texture",
        "missing-source",
        "escape",
        "binary",
        "version",
        "duplicate-key",
    ] {
        let f = Fixture::new(case);
        let p = f.source.join("Assets/hero.spine.json");
        let mut d: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        match case {
            "missing-texture" => d["pages"]["probe.png"]["assetId"] = json!("missing"),
            "missing-source" => d["skeletonSource"] = json!("Assets/missing.json"),
            "escape" => d["skeletonSource"] = json!("../outside.json"),
            "binary" => d["format"] = json!("binary"),
            "version" => d["exportVersion"] = json!("4.3.00"),
            _ => {}
        }
        f.write("Assets/hero.spine.json", &d);
        if case == "duplicate-key" {
            let s = fs::read_to_string(&p).unwrap();
            fs::write(&p, s.replacen("{", "{\"assetId\":\"duplicate\",", 1)).unwrap();
        }
        assert!(f.prepare().is_err(), "admitted {case}");
    }
}

#[cfg(feature = "spine")]
#[test]
fn spine374_b_texture_dependency_changes_cook_identity_and_pma_variant() {
    let f = Fixture::new("incremental");
    let first = f.prepare().unwrap();
    let same = f.prepare().unwrap();
    assert_eq!(
        first.assembly_input_digest().unwrap(),
        same.assembly_input_digest().unwrap()
    );
    let old = &first.spine_animator.as_ref().unwrap().registry.assets[0];
    f.pixels([200, 64, 32, 128]);
    let changed = f.prepare().unwrap();
    let new = &changed.spine_animator.as_ref().unwrap().registry.assets[0];
    assert_ne!(old.source_digest, new.source_digest);
    assert_ne!(old.pages, new.pages);
    assert_eq!(old.payload_digest, new.payload_digest);
    assert_ne!(
        first.assembly_input_digest().unwrap(),
        changed.assembly_input_digest().unwrap()
    );
}

#[cfg(feature = "spine")]
#[test]
fn spine374_b_package_rejects_registry_digest_payload_and_path_tampering() {
    for case in [
        "registry",
        "payload",
        "path",
        "backend",
        "missing-page",
        "page-pixels",
        "page-format",
        "dependency",
    ] {
        let f = Fixture::new(case);
        let input = f.prepare().unwrap();
        let package = f.package(&input);
        let p = package.join("spine/registry.json");
        let mut r: engine_runtime::spine_animation::CookedSpineRegistry =
            serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        match case {
            "registry" => r.registry_digest = "sha256:bad".into(),
            "payload" => {
                fs::write(package.join(&r.assets[0].payload_path), b"{}").unwrap();
            }
            "path" => {
                r.assets[0].payload_path = "../outside.json".into();
            }
            "backend" => r.backend.commit = "unsupported".into(),
            _ => {
                let manifest_path = package.join("assets/asset-manifest.json");
                let mut manifest: Value =
                    serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
                let page = &r.assets[0].pages[0].texture_asset_id;
                let meta = package.join(format!("cooked/textures/{page}.texture.json"));
                let mut m: Value = serde_json::from_slice(&fs::read(&meta).unwrap()).unwrap();
                match case {
                    "missing-page" => {
                        fs::remove_file(package.join(m["pixelDataPath"].as_str().unwrap()))
                            .unwrap();
                    }
                    "page-pixels" => {
                        fs::write(
                            package.join(m["pixelDataPath"].as_str().unwrap()),
                            [0u8; 16],
                        )
                        .unwrap();
                    }
                    "page-format" => {
                        m["colorSpace"] = json!("srgb");
                        fs::write(meta, serde_json::to_vec(&m).unwrap()).unwrap();
                    }
                    _ => {
                        for d in manifest["dependencyTable"].as_array_mut().unwrap() {
                            if d["assetGuid"] == "hero" {
                                d["dependencies"] = json!([]);
                            }
                        }
                        fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
                    }
                }
            }
        }
        fs::write(&p, serde_json::to_vec(&r).unwrap()).unwrap();
        let loaded = load_runtime_package(&package);
        assert!(loaded.value.is_none(), "accepted {case}");
        assert!(loaded.diagnostics.has_errors());
    }
}

#[cfg(feature = "spine")]
#[test]
fn spine374_b_cook_cache_reuses_unchanged_and_invalidates_only_texture_closure() {
    use project_authoring_execution::ProjectAssemblyArtifactCacheStatus as Status;
    let f = Fixture::new("closure-cache");
    f.write("Assets/other.page.asset",&json!({"schemaVersion":"texture-asset.v1","assetId":"other-texture","assetGuid":"other-texture","sourceImage":"Assets/other.png"}));
    fs::copy(
        f.source.join("Assets/page.png"),
        f.source.join("Assets/other.png"),
    )
    .unwrap();
    let mut other: Value =
        serde_json::from_slice(&fs::read(f.source.join("Assets/hero.spine.json")).unwrap())
            .unwrap();
    other["assetId"] = json!("other");
    other["pages"]["probe.png"]["assetId"] = json!("other-texture");
    f.write("Assets/other.spine.json", &other);
    let (cold, cold_reports) = f.prepare_reported().unwrap();
    assert_eq!(
        cold_reports
            .iter()
            .filter(|r| r.producer_id == "spine-cook" && r.cache_status == Status::Produced)
            .count(),
        2
    );
    let (warm, warm_reports) = f.prepare_reported().unwrap();
    assert_eq!(
        cold.assembly_input_digest().unwrap(),
        warm.assembly_input_digest().unwrap()
    );
    let hits = warm_reports
        .iter()
        .filter(|r| r.producer_id == "spine-cook")
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 2);
    assert!(hits
        .iter()
        .all(|r| r.cache_status == Status::Hit && r.substages.iter().all(|s| s.skipped)));
    f.pixels([200, 64, 32, 128]);
    let (changed, reports) = f.prepare_reported().unwrap();
    assert_eq!(
        reports
            .iter()
            .filter(|r| r.producer_id == "spine-cook" && r.cache_status == Status::Hit)
            .count(),
        1
    );
    assert_eq!(
        reports
            .iter()
            .filter(|r| r.producer_id == "spine-cook" && r.cache_status == Status::Produced)
            .count(),
        1
    );
    let asset = |input: &RuntimePackageBuildInput, id: &str| {
        input
            .spine_animator
            .as_ref()
            .unwrap()
            .registry
            .assets
            .iter()
            .find(|a| a.asset_id == id)
            .unwrap()
            .clone()
    };
    assert_eq!(asset(&cold, "other"), asset(&changed, "other"));
    assert_ne!(asset(&cold, "hero"), asset(&changed, "hero"));
    fs::write(
        f.root.join("cache-evidence.json"),
        serde_json::to_vec_pretty(
            &json!({"cold":cold_reports,"warm":warm_reports,"changed":reports}),
        )
        .unwrap(),
    )
    .unwrap();
}

#[cfg(feature = "spine")]
#[test]
fn spine374_b_legacy_v1_animator_registry_is_unchanged_when_spine_is_added() {
    let f = Fixture::new("legacy341");
    f.write("Animations/robot.animator-description-2d.json",&json!({"schema":"animator-description-2d.v1","assetId":"robot","default":"idle","parameters":{"moving":false},
        "animations":{"idle":{"frames":["texture-probe"],"loop":true},"walk":{"frames":["texture-probe","texture-probe"],"fps":10,"loop":true}},"rules":[{"when":"moving","play":"walk"}]}));
    let with = f.prepare().unwrap();
    assert_eq!(with.animator2d_registry.controllers[0].id, "robot");
    let loaded = load_runtime_package(f.package(&with));
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    assert_eq!(
        loaded.value.unwrap().animator2d_registry,
        with.animator2d_registry
    );
    for file in [
        "Assets/hero.spine.json",
        "Assets/hero.skeleton.json",
        "Assets/hero.atlas",
    ] {
        fs::remove_file(f.source.join(file)).unwrap();
    }
    let without = f.prepare().unwrap();
    assert!(without.spine_animator.is_none());
    assert_eq!(without.animator2d_registry, with.animator2d_registry);
}
