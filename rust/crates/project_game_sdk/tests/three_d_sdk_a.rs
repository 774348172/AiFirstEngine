use project_game_sdk::{
    DeferredMutation, DeferredMutationWriter, EntityHandle, GameResult, SpatialAssetRef,
    Transform3D, Value, WorldRead,
};
use project_runtime_sdk::ProjectRuntimeSpatialIntent as Intent;
use std::collections::BTreeMap;

#[test]
fn spatial_helpers_preserve_full_runtime_identity_and_typed_payloads() {
    let token = (17_u64 << 32) | 9;
    let handle = EntityHandle::new("cube", token).unwrap();
    let mut writer = DeferredMutationWriter::default();
    writer
        .write_local_transform3d(&handle, Transform3D::identity())
        .unwrap();
    writer.set_mesh_visible(&handle, false).unwrap();
    writer
        .set_material(
            &handle,
            "surface",
            SpatialAssetRef {
                id: "red".into(),
                asset_type: "material".into(),
                guid: Some("guid-red".into()),
                sub_asset: None,
            },
        )
        .unwrap();
    writer.select_main_camera(&handle).unwrap();
    assert_eq!(writer.as_slice().len(), 4);
    for mutation in writer.as_slice() {
        let DeferredMutation::Spatial {
            entity_id,
            generation,
            ..
        } = mutation
        else {
            panic!("spatial wire")
        };
        assert_eq!(entity_id, "cube");
        assert_eq!(*generation, token);
    }
    assert!(matches!(
        &writer.as_slice()[1],
        DeferredMutation::Spatial {
            intent: Intent::SetMeshVisible { visible: false },
            ..
        }
    ));
    assert!(
        matches!(&writer.as_slice()[2],DeferredMutation::Spatial {intent:Intent::SetMaterial {slot,..},..} if slot=="surface")
    );
}

#[test]
fn spatial_helpers_reject_authoring_names_without_live_handles() {
    let handle = EntityHandle::from_stable_identity("cube").unwrap();
    let mut writer = DeferredMutationWriter::default();
    assert!(writer
        .write_local_transform3d(&handle, Transform3D::identity())
        .is_err());
    assert!(writer.set_mesh_visible(&handle, false).is_err());
    assert!(writer.select_main_camera(&handle).is_err());
    assert!(writer
        .set_material(
            &handle,
            "surface",
            SpatialAssetRef {
                id: "x".into(),
                asset_type: "material".into(),
                guid: Some("g".into()),
                sub_asset: None
            }
        )
        .is_err());
    assert!(writer.as_slice().is_empty());
}

struct Read;
impl WorldRead for Read {
    fn query(&self, _: &[String], _: &[String]) -> GameResult<Vec<EntityHandle>> {
        Ok(vec![EntityHandle::new("cube", 23).unwrap()])
    }
    fn read_component(&self, entity: &EntityHandle, component: &str) -> GameResult<Value> {
        assert_eq!(entity.generation(), 23);
        assert_eq!(component, "engine.transform3d");
        Ok(Value::Object(BTreeMap::from([
            ("translation".into(), Value::Vec3([1., 2., 3.])),
            ("scale".into(), Value::Vec3([2.; 3])),
            (
                "rotationXyzw".into(),
                Value::Array(vec![
                    Value::Number(0.),
                    Value::Number(0.),
                    Value::Number(0.),
                    Value::Number(1.),
                ]),
            ),
        ])))
    }
}
#[test]
fn typed_read_uses_runtime_component_name_and_resolved_generation() {
    let handle = Read.resolve_entity("cube").unwrap();
    let transform = Read.read_transform3d(&handle).unwrap();
    assert_eq!(transform.translation, [1., 2., 3.]);
    assert_eq!(transform.scale, [2.; 3]);
    assert_eq!(transform.rotation_xyzw, [0., 0., 0., 1.]);
    assert!(Read.resolve_entity("absent").is_err());
}

#[test]
fn s2b_light_writer_preserves_generation_and_rejects_invalid_values_before_staging() {
    let handle = EntityHandle::new("sun", (17_u64 << 32) | 9).unwrap();
    let stale = EntityHandle::from_stable_identity("sun").unwrap();
    let mut w = DeferredMutationWriter::default();
    assert!(w.set_light_enabled(&stale, true).is_err());
    assert!(w.set_light_color(&stale, [1.; 3]).is_err());
    assert!(w.set_light_intensity(&stale, 1.).is_err());
    for bad in [-1., f32::NAN, f32::INFINITY] {
        assert!(w.set_light_color(&handle, [bad, 0., 0.]).is_err());
        assert!(w.set_light_intensity(&handle, bad).is_err());
    }
    assert!(w.as_slice().is_empty());
    w.set_light_enabled(&handle, false).unwrap();
    w.set_light_color(&handle, [0.1, 0.2, 0.3]).unwrap();
    w.set_light_intensity(&handle, 12.).unwrap();
    let items = w.into_inner();
    for m in &items {
        let DeferredMutation::Spatial {
            entity_id,
            generation,
            ..
        } = m
        else {
            panic!("light wire")
        };
        assert_eq!(entity_id, "sun");
        assert_eq!(*generation, (17_u64 << 32) | 9);
    }
    assert!(matches!(
        &items[0],
        DeferredMutation::Spatial {
            intent: Intent::SetLightEnabled { enabled: false },
            ..
        }
    ));
    assert!(
        matches!(&items[1],DeferredMutation::Spatial{intent:Intent::SetLightColor{color},..} if *color==[0.1,0.2,0.3])
    );
    assert!(
        matches!(&items[2],DeferredMutation::Spatial{intent:Intent::SetLightIntensity{intensity},..} if *intensity==12.)
    );
}
