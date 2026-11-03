//! Local full static Cook evidence; no runtime scale or display qualification.
use project_authoring_execution::{
    platform_3d_benchmark::*, GameProjectCompiler, ProjectAssemblyArtifactCacheStatus as Status,
    ProjectAuthoringSession, TargetProfile,
};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Instant};

#[path = "support/bm14_final_pbr.rs"]
mod final_pbr;

#[test]
#[ignore = "requires freshly generated run-owned BM14 source; five full Cook rounds"]
fn full_static_foundation_incremental_cook_has_exact_bound_payloads_and_invalidation() {
    let project = PathBuf::from(std::env::var_os("AIFE_BM14_PROJECT").unwrap());
    let evidence = PathBuf::from(std::env::var_os("AIFE_BM14_EVIDENCE").unwrap());
    assert!(
        !evidence.exists(),
        "preserve first evidence; fresh output required"
    );
    fs::create_dir_all(&evidence).unwrap();
    let mut previous = BTreeMap::new();
    let mut previous_digest = String::new();
    for round in ["cold", "unchanged", "material", "mesh", "shared-dependency"] {
        match round {
            "material" | "mesh" => {
                let relative = if round == "material" {
                    "Assets/material-000.json"
                } else {
                    "Assets/mesh-000.json"
                };
                let path = project.join(relative);
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                if round == "material" {
                    value["baseColor"][0] = serde_json::json!(0.85);
                } else {
                    let byte = value["renderData"]["data"]["payload"]["vertexBytes"][8]
                        .as_u64()
                        .unwrap();
                    value["renderData"]["data"]["payload"]["vertexBytes"][8] =
                        serde_json::json!(byte ^ 1);
                }
                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            "shared-dependency" => {
                // Four of the 256 materials reference this real PNG. The texture
                // must be rebuilt; unchanged material descriptors remain Hits.
                fs::copy(
                    project.join("Assets/texture-a-0.png"),
                    project.join("Assets/texture-000.png"),
                )
                .unwrap();
            }
            _ => {}
        }
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease(format!("371-bm14-{round}"), paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let started = Instant::now();
        eprintln!("BM14 {round}: prepare starting");
        let prepared = compiler.prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &evidence.join("cache"),
        );
        let wall_ms = started.elapsed().as_secs_f64() * 1000.;
        if let Err(error) = &prepared {
            fs::write(evidence.join(format!("{round}-failure.json")),serde_json::to_vec_pretty(&serde_json::json!({
                "code":error.code(),"message":error.message(),"diagnostics":error.diagnostics(),"wallMs":wall_ms
            })).unwrap()).unwrap();
        }
        let prepared = prepared.unwrap();
        eprintln!("BM14 {round}: prepare {wall_ms:.2}ms; inspecting actual payloads");
        let inspected = inspect_foundation_static_cook(&prepared);
        let input = prepared.runtime_package_build_input();
        let assets: BTreeMap<_, _> = input
            .assets
            .iter()
            .map(|a| (a.asset_id.clone(), a.hash.clone()))
            .collect();
        let changed: Vec<_> = assets
            .iter()
            .filter(|(id, hash)| previous.get(*id) != Some(*hash))
            .map(|(id, _)| id.clone())
            .collect();
        let producers: Vec<_> = prepared
            .producer_reports()
            .iter()
            .filter(|r| matches!(r.producer_id.as_str(), "render-asset-cook" | "texture-cook"))
            .collect();
        let outputs: Vec<_> = input
            .assets
            .iter()
            .map(|a| {
                serde_json::json!({"assetId":a.asset_id,"dependencies":a.dependencies,
            "digest":a.hash,"payloadBytes":a.runtime_payload.as_ref().map_or(0,Vec::len)})
            })
            .collect();
        fs::write(evidence.join(format!("{round}.json")),serde_json::to_vec_pretty(&serde_json::json!({
            "scope":"BM-14/foundation-v1 static Cook; not runtime scale or complete BM14",
            "round":round,"target":"windows-release","sourceDigest":source_digest(lease.snapshot()),
            "assemblyDigest":prepared.assembly_digest(),"wallMs":wall_ms,"inspected":inspected.as_ref().ok(),
            "inspectionError":inspected.as_ref().err().map(|e|serde_json::json!({"code":e.code,"message":e.message})),
            "producerCalls":producers,"changedAssets":changed,"outputs":outputs,
            "cache":"fresh only cold; OS IO cache unknown",
            "notExecuted":["skin","clip","PBR","mips","world dataset","actual OS display"]
        })).unwrap()).unwrap();
        inspected.unwrap();
        assert_eq!(producers.len(), 10003 + 260 + 68);
        match round {
            "cold" => assert!(producers.iter().all(|p| p.cache_status == Status::Produced)),
            "unchanged" => {
                assert!(producers.iter().all(|p| p.cache_status == Status::Hit));
                assert!(changed.is_empty());
                assert_eq!(prepared.assembly_digest(), previous_digest);
            }
            _ => {
                let id = match round {
                    "material" => "material-000",
                    "mesh" => "mesh-000",
                    _ => "texture-000",
                };
                assert_eq!(changed, vec![id.to_owned()]);
                assert_eq!(
                    producers
                        .iter()
                        .filter(|p| p.cache_status == Status::Produced)
                        .count(),
                    1
                );
                assert_eq!(
                    producers
                        .iter()
                        .filter(|p| p.cache_status == Status::Hit)
                        .count(),
                    producers.len() - 1
                );
                assert_ne!(prepared.assembly_digest(), previous_digest);
                if round == "shared-dependency" {
                    assert_eq!(
                        input
                            .assets
                            .iter()
                            .filter(|a| a.asset_id.starts_with("material-")
                                && a.dependencies.contains(&"371-texture-000".into()))
                            .count(),
                        4
                    );
                }
            }
        }
        previous = assets;
        previous_digest = prepared.assembly_digest().to_owned();
        eprintln!("BM14 {round}: complete");
    }
}
