//! Original cell/loader owners under bounded failure, churn and disk save reload.
use super::s4a::Fixture;
use engine_runtime::{
    archetype::ComponentValue,
    component_value::RuntimeValue,
    components::ComponentTypeId,
    field_path::FieldPath,
    ids::EntityId,
    project_runtime_session::ProjectRuntimeMutationBuffer,
    runtime_instance_loader::{CellError, CellStatus},
    world_persistence::*,
};
use serde_json::json;

fn drained(f: &Fixture) {
    assert_eq!(f.loader.cell_lifetime_counts(), (0, 0));
    assert_eq!(f.loader.asset_loader().decoded_cache_len(), 0);
    let s = f.loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (
            s.pending_tickets,
            s.pending_operations,
            s.reserved_bytes,
            s.cpu_resident_bytes
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(f.manager.report(f.frame).resident_bytes, 0);
}
#[test]
fn s4c_twenty_original_cell_load_unload_cycles_return_to_residency_baseline() {
    let mut f = Fixture::new(None);
    drained(&f);
    for cycle in 0..20 {
        let ticket = f.load("repeat");
        let source = f.node(ticket);
        let runtime = f.world.runtime_id_for_source(&source).unwrap();
        assert_eq!(f.loader.cell_status(ticket), Ok(CellStatus::Active));
        f.release(ticket);
        assert!(f.world.entity(&source).is_none());
        assert!(f.world.try_resolve_runtime_entity(runtime).is_err());
        f.frame += 1;
        f.manager.end_frame(f.frame);
        drained(&f);
        assert_eq!(
            f.loader.cell_status(ticket),
            Ok(CellStatus::Released),
            "cycle {cycle}"
        );
    }
}
#[test]
fn s4c_missing_cooked_cell_file_fails_within_bound_and_releases_original_requests() {
    let mut f = Fixture::new(None);
    let package = &f._files.package;
    let record = package
        .runtime_asset_index
        .record_by_guid("cell-a")
        .unwrap();
    let cooked = package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    std::fs::remove_file(package.package_dir.join(cooked.path.as_ref().unwrap())).unwrap();
    let request =
        super::streamed_cells::request("a", f.loader.asset_loader().asset_revision_epoch(), false);
    let ticket = f.loader.request_cell(request, &f.world).unwrap();
    let started = std::time::Instant::now();
    loop {
        match f.loader.prepare_cell_with_simulation(
            ticket,
            &f.world,
            f.frame,
            &mut f.manager,
            &mut f.physics,
            &mut f.nav,
        ) {
            Err(CellError::NotReady) => {
                assert!(
                    started.elapsed().as_secs() < 10,
                    "missing payload must produce a terminal failure"
                );
                std::thread::yield_now();
            }
            Err(error) => {
                assert!(matches!(error, CellError::Asset(_)), "{error:?}");
                break;
            }
            Ok(status) => panic!("missing payload reached {status:?}"),
        }
    }
    assert_eq!(f.loader.cell_status(ticket), Ok(CellStatus::Failed));
    assert!(f.loader.cell_error(ticket).is_some());
    drained(&f);
}
fn health(f: &Fixture, id: &EntityId) -> RuntimeValue {
    let ComponentValue::Dynamic {
        value: RuntimeValue::Object(v),
        ..
    } = f
        .world
        .component_value(id, &ComponentTypeId::new("project.stats"))
        .unwrap()
    else {
        panic!()
    };
    v["health"].clone()
}
fn set_health(f: &mut Fixture, id: &EntityId, n: i64) {
    let mut batch = ProjectRuntimeMutationBuffer::new();
    batch.write_component_field(
        id.clone(),
        ComponentTypeId::new("project.stats"),
        FieldPath::parse("health").unwrap(),
        RuntimeValue::I64(n),
    );
    batch
        .prepare(&f.world)
        .unwrap()
        .commit(&mut f.world)
        .unwrap();
}
#[test]
fn s4c_logical_snapshot_disk_bytes_restore_new_incarnations_and_corruption_is_atomic() {
    let mut f = Fixture::new(Some(
        json!({"componentType":"project.stats","data":{"health":10}}),
    ));
    let left = f.load("left");
    let right = f.load("right");
    let li = f.node(left);
    let ri = f.node(right);
    set_health(&mut f, &li, 31);
    set_health(&mut f, &ri, 79);
    let old = f.world.runtime_id_for_source(&li).unwrap();
    let declaration = WorldLogicalDeclaration3D {
        world_id: "disk-world".into(),
        revision: 1,
        placements: vec!["left".into(), "right".into()],
        fields: vec![WorldLogicalField3D {
            component: "project.stats".into(),
            field: "health".into(),
        }],
    };
    let snapshot = f
        .loader
        .capture_partition_logical(&declaration, &f.world)
        .unwrap();
    let path = f._files.root.join("world-save.json");
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    f.release(left);
    f.release(right);
    let left = f.load("left");
    let right = f.load("right");
    let li = f.node(left);
    let ri = f.node(right);
    assert!(f.world.try_resolve_runtime_entity(old).is_err());
    assert_eq!(health(&f, &li), RuntimeValue::I64(10));
    f.loader
        .restore_partition_logical(&declaration, &std::fs::read(&path).unwrap(), &mut f.world)
        .unwrap();
    assert_eq!(health(&f, &li), RuntimeValue::I64(31));
    assert_eq!(health(&f, &ri), RuntimeValue::I64(79));
    let mut corrupt = snapshot;
    corrupt.records[0].value = project_runtime_sdk::ProjectRuntimeValue::Integer(999);
    corrupt.records[1].source_entity = "missing-source".into();
    std::fs::write(&path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
    assert!(f
        .loader
        .restore_partition_logical(&declaration, &std::fs::read(&path).unwrap(), &mut f.world)
        .is_err());
    assert_eq!(health(&f, &li), RuntimeValue::I64(31));
    assert_eq!(health(&f, &ri), RuntimeValue::I64(79));
    f.release(left);
    f.release(right);
    drained(&f);
}
