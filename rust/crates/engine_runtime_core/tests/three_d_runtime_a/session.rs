use super::*;
use engine_runtime::frame_loop::{FrameLoop, ProjectRuntimeFrameSession, RuntimeFrameContext};
use engine_runtime::project_runtime_session::*;
use project_runtime_sdk::{ProjectRuntimeSpatialAssetRef, ProjectRuntimeSpatialIntent as Intent};
use std::collections::BTreeMap;

#[derive(Default)]
struct Session {
    mutations: Option<ProjectRuntimeMutationBuffer>,
    receipts: BTreeMap<u64, (EntityId, u64)>,
    status: Option<ProjectRuntimeSessionStatus>,
}
impl ProjectRuntimeSession for Session {
    fn session_id(&self) -> &str {
        "spatial-test"
    }
    fn accept_spawn_receipts(&mut self, receipts: BTreeMap<u64, (EntityId, u64)>) {
        self.receipts = receipts;
    }
    fn handle_aui_actions(
        &mut self,
        _: ProjectRuntimeSessionContext<'_>,
        _: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }
    fn fixed_update(&mut self, _: ProjectRuntimeSessionContext<'_>) -> ProjectRuntimeSessionOutput {
        let mut output = self
            .mutations
            .take()
            .map(ProjectRuntimeSessionOutput::applied)
            .unwrap_or_else(ProjectRuntimeSessionOutput::no_op);
        if let Some(status) = self.status {
            output.status = status;
        }
        output
    }
}
fn fixture() -> Fixture {
    let mut camera = base()["entities"][1].clone();
    camera["id"] = json!("prefabCamera");
    Fixture::new(
        base(),
        Some(
            json!({"schemaVersion":"runtime-prefab.v2","id":"p","name":"P","rootEntityId":"prefabCamera","entities":[camera]}),
        ),
    )
}
fn prefab(fixture: &Fixture) -> ProjectRuntimeSpatialAssetRef {
    let reference = RuntimeAssetRef {
        id: "p".into(),
        asset_type: "prefab".into(),
        guid: None,
        sub_asset: None,
    };
    let record = fixture
        .package
        .runtime_asset_index
        .resolve(&reference)
        .unwrap();
    ProjectRuntimeSpatialAssetRef {
        id: "p".into(),
        asset_type: "prefab".into(),
        guid: Some(record.asset_guid.clone()),
        sub_asset: None,
    }
}
fn tick(
    fixture: &Fixture,
    hydrator: &mut RuntimeSceneHydrator,
    world: &mut World,
    session: &mut Session,
) -> Result<(), String> {
    FrameLoop::new("main")
        .tick_runtime_frame_with_project_session_delta_and_fixed_steps(
            world,
            &mut RenderSceneState::new(),
            &mut RenderExtractContext::new(),
            None,
            None,
            1.0 / 60.0,
            Some(RuntimeFrameContext::new(
                &fixture.package,
                hydrator.instance_loader_mut(),
            )),
            ProjectRuntimeFrameSession {
                session,
                actions: &[],
                report_level: ProjectRuntimeSessionReportLevel::Summary,
                observation_contract: None,
            },
            1,
        )
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn spatial_prefab_tick_returns_live_root_without_selecting_camera_and_despawn_releases_it() {
    let fixture = fixture();
    let (mut world, mut hydrator) = fixture.hydrate();
    let before = world.entity_count();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.instantiate_spatial_prefab(7, prefab(&fixture));
    let mut session = Session {
        mutations: Some(mutations),
        ..Default::default()
    };
    tick(&fixture, &mut hydrator, &mut world, &mut session).unwrap();
    assert_eq!(world.entity_count(), before + 1);
    assert_eq!(world.main_camera(), Some(&id("camera")));
    let (root, generation) = session.receipts[&7].clone();
    assert_eq!(world.entity_generation(&root), Some(generation));
    assert!(world.camera3d(&root).is_some());
    assert_ne!(root, id("prefabCamera"));
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.spatial_command(root.clone(), generation, Intent::SelectMainCamera {});
    session.mutations = Some(mutations);
    tick(&fixture, &mut hydrator, &mut world, &mut session).unwrap();
    assert_eq!(world.main_camera(), Some(&root));
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.despawn_spatial_prefab(root.clone(), generation);
    session.mutations = Some(mutations);
    tick(&fixture, &mut hydrator, &mut world, &mut session).unwrap();
    assert_eq!(world.entity_count(), before);
    assert!(world.entity_generation(&root).is_none());
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.despawn_spatial_prefab(root, generation);
    session.mutations = Some(mutations);
    assert!(tick(&fixture, &mut hydrator, &mut world, &mut session)
        .unwrap_err()
        .contains("three_d.stale_entity"));
}

#[test]
fn spatial_prefab_invalid_tail_rolls_back_prepared_assets_and_all_world_writes() {
    let fixture = fixture();
    let (mut world, mut hydrator) = fixture.hydrate();
    let before = world.entity_count();
    let cache = hydrator
        .instance_loader_mut()
        .asset_loader()
        .decoded_cache_len();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.spatial_command(
        id("cube"),
        world.entity_generation(&id("cube")).unwrap(),
        Intent::SetMeshVisible { visible: false },
    );
    mutations.instantiate_spatial_prefab(1, prefab(&fixture));
    let mut missing = prefab(&fixture);
    missing.guid = Some("missing".into());
    missing.id = "missing".into();
    mutations.instantiate_spatial_prefab(2, missing);
    let mut session = Session {
        mutations: Some(mutations),
        ..Default::default()
    };
    world.take_dirty_records();
    assert!(tick(&fixture, &mut hydrator, &mut world, &mut session)
        .unwrap_err()
        .contains("three_d.prefab_prepare_failed"));
    assert_eq!(world.entity_count(), before);
    assert!(world.mesh_renderer3d(&id("cube")).unwrap().visible);
    assert!(world.dirty_records().is_empty());
    assert!(session.receipts.is_empty());
    assert_eq!(
        hydrator
            .instance_loader_mut()
            .asset_loader()
            .decoded_cache_len(),
        cache
    );
}

#[test]
fn spatial_prefab_noop_and_duplicate_request_never_spawn() {
    let fixture = fixture();
    for status in [
        ProjectRuntimeSessionStatus::NoOp,
        ProjectRuntimeSessionStatus::Applied,
    ] {
        let (mut world, mut hydrator) = fixture.hydrate();
        let before = world.entity_count();
        let mut mutations = ProjectRuntimeMutationBuffer::new();
        mutations.instantiate_spatial_prefab(1, prefab(&fixture));
        mutations.instantiate_spatial_prefab(1, prefab(&fixture));
        let mut session = Session {
            mutations: Some(mutations),
            status: Some(status),
            ..Default::default()
        };
        let result = tick(&fixture, &mut hydrator, &mut world, &mut session);
        assert_eq!(result.is_ok(), status == ProjectRuntimeSessionStatus::NoOp);
        assert_eq!(world.entity_count(), before);
        assert!(session.receipts.is_empty());
    }
}

#[test]
fn spatial_material_missing_package_identity_rejects_prior_write() {
    let fixture = fixture();
    let (mut world, mut hydrator) = fixture.hydrate();
    let generation = world.entity_generation(&id("cube")).unwrap();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.spatial_command(
        id("cube"),
        generation,
        Intent::SetMeshVisible { visible: false },
    );
    mutations.spatial_command(
        id("cube"),
        generation,
        Intent::SetMaterial {
            slot: "surface".into(),
            material: ProjectRuntimeSpatialAssetRef {
                id: "missing".into(),
                asset_type: "material".into(),
                guid: Some("missing".into()),
                sub_asset: None,
            },
        },
    );
    let mut session = Session {
        mutations: Some(mutations),
        ..Default::default()
    };
    assert!(tick(&fixture, &mut hydrator, &mut world, &mut session)
        .unwrap_err()
        .contains("three_d.material_unresolved"));
    assert!(world.mesh_renderer3d(&id("cube")).unwrap().visible);
}
