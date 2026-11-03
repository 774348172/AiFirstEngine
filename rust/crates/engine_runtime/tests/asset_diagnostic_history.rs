//! A long-running loader must retain its first failure without an unbounded log.
use engine_runtime::runtime_asset_diagnostics::{
    AssetLoadDiagnostic, AssetLoadDiagnostics, AssetLoadErrorCode, AssetLoadStage, AssetLoadState,
};

fn event(request: u64, failed: bool) -> AssetLoadDiagnostic {
    AssetLoadDiagnostic {
        request_id: request,
        asset_ref_id: format!("resource-{request}"),
        asset_guid: None,
        asset_type: "mesh".into(),
        stage: AssetLoadStage::Decode,
        state: if failed {
            AssetLoadState::Failed
        } else {
            AssetLoadState::Ok
        },
        error_code: failed.then_some(AssetLoadErrorCode::DecodeFailed),
        bundle_id: None,
        cooked_asset_id: None,
        loader_kind: None,
        dependency_chain: vec![],
        handle_id: Some(request),
        generation: Some(1),
        sync_load: false,
        recommended_action: failed.then(|| "Check the source payload.".into()),
    }
}

#[test]
fn diagnostic_history_is_bounded_and_preserves_first_failure_and_recent_context() {
    // The first failure may arrive before or after the success history fills.
    for first_failure in [1, 4097] {
        let mut history = AssetLoadDiagnostics::new();
        for request in 1..=16384 {
            history.push(event(request, request == first_failure || request == 16383));
            assert!(
                history.entries().len() <= 1024,
                "unbounded history at {request}"
            );
        }
        let entries = history.entries();
        assert_eq!(history.omitted_count() + entries.len() as u64, 16384);
        assert_eq!(entries.last().unwrap().request_id, 16384);
        assert!(entries
            .iter()
            .any(|entry| entry.request_id == first_failure));
        assert!(entries.iter().any(|entry| entry.request_id == 16383));
        assert!(entries
            .windows(2)
            .all(|pair| pair[0].request_id < pair[1].request_id));
        assert!(history.has_error(AssetLoadErrorCode::DecodeFailed));
    }
}

#[test]
fn ordinary_loader_failure_history_reports_omissions_and_keeps_first_cause() {
    use engine_runtime::{
        resource_memory::{memory_totals, ResourceMemorySample},
        runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable},
        runtime_asset_loader::RuntimeAssetLoader,
    };
    let mut loader = RuntimeAssetLoader::new(
        "unused",
        RuntimeAssetIndex::new(vec![], vec![]),
        RuntimePackageMountTable::new(vec![]),
    );
    for request in 0..20000 {
        assert!(loader
            .load_by_id(format!("missing-{request}"), "mesh")
            .is_err());
    }
    let history = loader.diagnostics();
    assert!(history.entries().len() <= 1024);
    assert_eq!(history.entries().first().unwrap().asset_ref_id, "missing-0");
    assert_eq!(
        history.entries().last().unwrap().asset_ref_id,
        "missing-19999"
    );
    assert_eq!(
        history.omitted_count() + history.entries().len() as u64,
        20000
    );
    assert!(history.has_error(AssetLoadErrorCode::MissingAssetRef));
    let mut sample = ResourceMemorySample::new(1, 1, 0);
    loader.observe_memory(&mut sample);
    assert_eq!(
        sample.capacities["loader.diagnostics.retained"],
        history.entries().len() as u64
    );
    assert_eq!(
        sample.capacities["loader.diagnostics.omitted"],
        history.omitted_count()
    );
    assert!(memory_totals(&sample.allocations).unwrap().cpu_allocated < 1024 * 1024);
}
