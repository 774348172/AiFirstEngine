//! One ordinary combined Compiler output; each required domain must survive Cook.
use super::*;
#[test]
#[ignore = "explicit full BM10 Cook; construction-owned memory/time monitor required"]
fn actual_bm10_combined_cook_and_missing_domains_reject() {
    let root = PathBuf::from(std::env::var_os("AIFE_BM10_PROJECT").unwrap());
    let evidence = PathBuf::from(std::env::var_os("AIFE_BM10_EVIDENCE").unwrap());
    fs::create_dir(&evidence).unwrap();
    let mut session = ProjectAuthoringSession::open(&root).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("bm10-full-combined-cook", paths)
        .unwrap();
    let digest = source_digest(lease.snapshot());
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let prepared = compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &evidence.parent().unwrap().join(".lit-cook-cache"),
        )
        .unwrap();
    let actual = inspect_bm10_input(prepared.runtime_package_build_input()).unwrap();
    let mut bad = prepared.runtime_package_build_input().clone();
    let mut rejected = Vec::new();
    for id in ["actor-099", "dynamic-0999", "agent-0999", "speaker-63"] {
        let index = bad.scenes[0]
            .entities
            .iter()
            .position(|e| e.id == id)
            .unwrap();
        let removed = bad.scenes[0].entities.remove(index);
        let error = inspect_bm10_input(&bad).expect_err(id);
        rejected.push(json!({"missingEntity":id,"code":error.code,"message":error.message}));
        bad.scenes[0].entities.insert(index, removed);
    }
    for id in ["bm10-prop", "bm13-particles", "bm13-tone"] {
        let index = bad.assets.iter().position(|a| a.asset_id == id).unwrap();
        let removed = bad.assets.remove(index);
        let error = inspect_bm10_input(&bad).expect_err(id);
        rejected.push(json!({"missingAsset":id,"code":error.code,"message":error.message}));
        bad.assets.insert(index, removed);
    }
    let node = bad.aui_documents[0].document["nodes"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();
    let error = inspect_bm10_input(&bad).unwrap_err();
    rejected.push(json!({"missing":"HUD node","code":error.code,"message":error.message}));
    bad.aui_documents[0].document["nodes"]
        .as_array_mut()
        .unwrap()
        .push(node);
    fs::write(evidence.join("terminal.json"),serde_json::to_vec_pretty(&json!({"status":"passed","scope":"actual full BM10 combined Cook and required-domain rejections only; runtime/visibility/particles/mixing/K3K2/performance remain separate","sourceDigest":digest,"actual":actual,"rejected":rejected,"producers":prepared.producer_reports()})).unwrap()).unwrap();
}
